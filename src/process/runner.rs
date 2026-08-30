use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use colored::*;

use crate::adapters;
use crate::core::cas::ObjectStore;
use crate::core::diff::TreeDiff;
use crate::core::journal::Journal;
use crate::core::tree::TreeManifest;

pub fn execute_wrapped(
    workspace: &Path,
    cmd_args: &[String],
    tag: Option<String>,
) -> io::Result<i32> {
    let safepoint_dir = workspace.join(".safepoint");
    let store = ObjectStore::new(&safepoint_dir)?;
    let mut journal = Journal::load(&safepoint_dir)?;

    eprintln!("{}", "[safepoint] Taking pre-execution baseline snapshot...".cyan().dimmed());
    let baseline_tree = TreeManifest::scan(workspace, &store)?;
    let parent_hash = baseline_tree.save(&store)?;

    let command_str = cmd_args.join(" ");
    eprintln!("{} `{}`\n", "[safepoint] Running:".cyan().bold(), command_str);

    let start = Instant::now();
    let mut child = Command::new(&cmd_args[0])
        .args(&cmd_args[1..])
        .current_dir(workspace)
        .spawn()?;

    let status = child.wait()?;
    let duration_ms = start.elapsed().as_millis() as u64;
    let exit_code = status.code().unwrap_or(-1);

    eprintln!("\n{}", "[safepoint] Command finished. Scanning for workspace mutations...".cyan().dimmed());
    let post_tree = TreeManifest::scan(workspace, &store)?;
    let post_hash = post_tree.save(&store)?;

    let diff = TreeDiff::compute(&baseline_tree, &post_tree);
    let summary = diff.summary();

    let agent_meta = adapters::detect_and_extract(workspace, &command_str);

    let cp = journal.add(
        post_hash,
        Some(parent_hash),
        summary.clone(),
        post_tree.total_files,
        post_tree.total_bytes,
        Some(command_str),
        Some(exit_code),
        Some(duration_ms),
        tag,
        agent_meta.agent_name,
        agent_meta.prompt,
        agent_meta.tool_calls,
    );
    journal.save(&safepoint_dir)?;

    if diff.is_empty() {
        eprintln!(
            "{} Checkpoint {} created (No workspace changes detected).",
            "[safepoint]".green().bold(),
            format!("@{}", cp.id).yellow().bold()
        );
    } else {
        eprintln!(
            "{} Checkpoint {} created: {}",
            "[safepoint]".green().bold(),
            format!("@{}", cp.id).yellow().bold(),
            summary.bold()
        );
        diff.print_summary();
    }

    Ok(exit_code)
}
