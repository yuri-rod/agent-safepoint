use std::env;
use std::io;
use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use colored::*;

use safepoint::core::cas::ObjectStore;
use safepoint::core::diff::TreeDiff;
use safepoint::core::journal::Journal;
use safepoint::core::restore::RestorePlan;
use safepoint::core::tree::TreeManifest;
use safepoint::process::runner::execute_wrapped;

#[derive(Parser)]
#[command(name = "safepoint", version, about = "Universal, local-first undo and recovery layer for coding agents")]
struct Cli {
    #[arg(short = 'C', long = "directory", global = true, help = "Run as if safepoint was started in <directory>")]
    directory: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Wrap and execute an agent or command with automatic pre/post checkpoints", trailing_var_arg = true)]
    Run {
        #[arg(short = 't', long = "tag", help = "Optional tag label for this checkpoint")]
        tag: Option<String>,

        #[arg(required = true, help = "Command and arguments to execute (e.g. codex, claude, python script.py)")]
        command: Vec<String>,
    },

    #[command(about = "Create a manual checkpoint of the current workspace state")]
    Checkpoint {
        #[arg(short = 'm', long = "message", help = "Checkpoint message description")]
        message: Option<String>,

        #[arg(short = 't', long = "tag", help = "Optional tag label")]
        tag: Option<String>,
    },

    #[command(about = "Show the timeline of recorded checkpoints")]
    Timeline,

    #[command(about = "Show current workspace modifications relative to the latest checkpoint")]
    Status,

    #[command(about = "Show unified line diff between workspace and a checkpoint, or between two checkpoints")]
    Diff {
        #[arg(default_value = "latest", help = "Checkpoint selector (e.g. @1, @2, latest, or tag)")]
        target: String,

        #[arg(help = "Optional second checkpoint to compare against")]
        base: Option<String>,
    },

    #[command(about = "Restore the workspace to a prior checkpoint, reverting all agent/shell mutations")]
    Undo {
        #[arg(default_value = "latest", help = "Checkpoint selector to revert to (e.g. @1, @2, latest)")]
        target: String,

        #[arg(long = "dry-run", help = "Show what would be restored without modifying disk")]
        dry_run: bool,
    },

    #[command(about = "List all tracked files in a specific checkpoint")]
    ListFiles {
        #[arg(default_value = "latest", help = "Checkpoint selector (e.g. @1, @2, latest)")]
        target: String,
    },
}

fn find_workspace_root(start_dir: &Path) -> PathBuf {
    let mut curr = start_dir.to_path_buf();
    loop {
        if curr.join(".safepoint").exists() || curr.join(".git").exists() {
            return curr;
        }
        if !curr.pop() {
            break;
        }
    }
    start_dir.to_path_buf()
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let current_dir = env::current_dir()?;
    let workspace = cli.directory.unwrap_or_else(|| find_workspace_root(&current_dir));
    let safepoint_dir = workspace.join(".safepoint");

    match cli.command {
        Commands::Run { tag, command } => {
            let code = execute_wrapped(&workspace, &command, tag)?;
            std::process::exit(code);
        }

        Commands::Checkpoint { message, tag } => {
            let store = ObjectStore::new(&safepoint_dir)?;
            let mut journal = Journal::load(&safepoint_dir)?;

            let prev_hash = journal.latest().map(|cp| cp.tree_hash.clone());
            let tree = TreeManifest::scan(&workspace, &store)?;
            let tree_hash = tree.save(&store)?;

            let summary = if let Some(prev) = &prev_hash {
                let prev_tree = TreeManifest::load(prev, &store)?;
                let diff = TreeDiff::compute(&prev_tree, &tree);
                diff.summary()
            } else {
                format!("Initial baseline ({} files)", tree.total_files)
            };

            let final_summary = if let Some(msg) = message {
                format!("{}: {}", msg, summary)
            } else {
                summary
            };

            let cp = journal.add(
                tree_hash,
                prev_hash,
                final_summary,
                tree.total_files,
                tree.total_bytes,
                None,
                None,
                None,
                tag,
            );
            journal.save(&safepoint_dir)?;

            println!(
                "{} Created checkpoint {} ({})",
                "✓".green().bold(),
                format!("@{}", cp.id).yellow().bold(),
                cp.summary
            );
        }

        Commands::Timeline => {
            let journal = Journal::load(&safepoint_dir)?;
            journal.print_timeline();
        }

        Commands::Status => {
            let store = ObjectStore::new(&safepoint_dir)?;
            let journal = Journal::load(&safepoint_dir)?;

            let latest_cp = match journal.latest() {
                Some(cp) => cp,
                None => {
                    println!("No checkpoints recorded yet. Run `safepoint checkpoint` to create a baseline.");
                    return Ok(());
                }
            };

            let base_tree = TreeManifest::load(&latest_cp.tree_hash, &store)?;
            let current_tree = TreeManifest::scan(&workspace, &store)?;
            let diff = TreeDiff::compute(&base_tree, &current_tree);

            println!("\n{}", format!("=== Workspace Status (relative to @{}) ===", latest_cp.id).cyan().bold());
            if diff.is_empty() {
                println!("Workspace clean, no changes relative to latest checkpoint.");
            } else {
                println!("Modifications: {}\n", diff.summary().bold());
                diff.print_summary();
            }
            println!();
        }

        Commands::Diff { target, base } => {
            let store = ObjectStore::new(&safepoint_dir)?;
            let journal = Journal::load(&safepoint_dir)?;

            let target_cp = match journal.find_by_selector(&target) {
                Some(cp) => cp,
                None => {
                    eprintln!("Error: Checkpoint '{}' not found.", target);
                    std::process::exit(1);
                }
            };

            let (from_tree, to_tree) = if let Some(base_sel) = base {
                let base_cp = match journal.find_by_selector(&base_sel) {
                    Some(cp) => cp,
                    None => {
                        eprintln!("Error: Base checkpoint '{}' not found.", base_sel);
                        std::process::exit(1);
                    }
                };
                (
                    TreeManifest::load(&base_cp.tree_hash, &store)?,
                    TreeManifest::load(&target_cp.tree_hash, &store)?,
                )
            } else {
                (
                    TreeManifest::load(&target_cp.tree_hash, &store)?,
                    TreeManifest::scan(&workspace, &store)?,
                )
            };

            let diff = TreeDiff::compute(&from_tree, &to_tree);
            println!("\n{}", format!("=== Diff relative to @{} ===", target_cp.id).cyan().bold());
            if diff.is_empty() {
                println!("No differences detected.");
            } else {
                diff.render_diff(&store);
            }
        }

        Commands::Undo { target, dry_run } => {
            let store = ObjectStore::new(&safepoint_dir)?;
            let journal = Journal::load(&safepoint_dir)?;

            let target_cp = match journal.find_by_selector(&target) {
                Some(cp) => cp,
                None => {
                    eprintln!("Error: Checkpoint '{}' not found.", target);
                    std::process::exit(1);
                }
            };

            let target_tree = TreeManifest::load(&target_cp.tree_hash, &store)?;
            let current_tree = TreeManifest::scan(&workspace, &store)?;

            let plan = RestorePlan::build(&target_tree, &current_tree);
            plan.print_plan();

            if dry_run {
                println!("{}", "[Dry run mode: No changes made to disk]".yellow());
                return Ok(());
            }

            if plan.files_to_delete.is_empty()
                && plan.files_to_restore.is_empty()
                && plan.files_to_modify.is_empty()
                && plan.modes_to_change.is_empty()
            {
                println!("{}", "Nothing to undo. Workspace is already at target state.".green());
                return Ok(());
            }

            plan.execute(&workspace, &store)?;
            println!(
                "{} Successfully rolled back workspace to checkpoint {}",
                "✓".green().bold(),
                format!("@{}", target_cp.id).yellow().bold()
            );
        }

        Commands::ListFiles { target } => {
            let store = ObjectStore::new(&safepoint_dir)?;
            let journal = Journal::load(&safepoint_dir)?;

            let target_cp = match journal.find_by_selector(&target) {
                Some(cp) => cp,
                None => {
                    eprintln!("Error: Checkpoint '{}' not found.", target);
                    std::process::exit(1);
                }
            };

            let tree = TreeManifest::load(&target_cp.tree_hash, &store)?;
            println!("\n{}", format!("=== Files in Checkpoint @{} ({} files, {:.2} MB) ===", target_cp.id, tree.total_files, tree.total_bytes as f64 / 1_048_576.0).cyan().bold());
            for (path, entry) in &tree.entries {
                let kind = if entry.is_symlink {
                    format!("-> {}", entry.symlink_target.as_deref().unwrap_or("")).magenta()
                } else {
                    format!("{:>8} bytes", entry.size).dimmed()
                };
                println!("  {:04o}  {}  {}", entry.mode & 0o777, kind, path);
            }
            println!();
        }
    }

    Ok(())
}
