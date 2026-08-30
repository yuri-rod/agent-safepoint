use std::fs;
use std::io;
use std::path::Path;
use colored::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::cas::ObjectStore;
use super::diff::TreeDiff;
use super::tree::{FileEntry, TreeManifest};

pub struct RestorePlan {
    pub diff: TreeDiff,
    pub files_to_delete: Vec<String>,
    pub files_to_restore: Vec<FileEntry>,
    pub files_to_modify: Vec<(FileEntry, FileEntry)>,
    pub modes_to_change: Vec<(FileEntry, FileEntry)>,
}

impl RestorePlan {
    pub fn build(target: &TreeManifest, current: &TreeManifest) -> Self {
        let diff = TreeDiff::compute(target, current);

        let mut files_to_delete = Vec::new();
        for added in &diff.added {
            files_to_delete.push(added.path.clone());
        }
        for (_del, add) in &diff.renamed {
            files_to_delete.push(add.path.clone());
        }

        let mut files_to_restore = Vec::new();
        for deleted in &diff.deleted {
            files_to_restore.push(deleted.clone());
        }
        for (del, _add) in &diff.renamed {
            files_to_restore.push(del.clone());
        }

        let mut files_to_modify = Vec::new();
        for (target_entry, current_entry) in &diff.modified {
            files_to_modify.push((target_entry.clone(), current_entry.clone()));
        }

        let mut modes_to_change = Vec::new();
        for (target_entry, current_entry) in &diff.mode_changed {
            modes_to_change.push((target_entry.clone(), current_entry.clone()));
        }

        Self {
            diff,
            files_to_delete,
            files_to_restore,
            files_to_modify,
            modes_to_change,
        }
    }

    pub fn print_plan(&self) {
        println!("\n{}", "=== Safepoint Restore Plan ===".cyan().bold());
        if self.files_to_delete.is_empty()
            && self.files_to_restore.is_empty()
            && self.files_to_modify.is_empty()
            && self.modes_to_change.is_empty()
        {
            println!("Workspace is already identical to target checkpoint.");
            return;
        }

        for path in &self.files_to_delete {
            println!("  {} {} (will remove)", "-".red().bold(), path);
        }
        for entry in &self.files_to_restore {
            println!("  {} {} (will recreate)", "+".green().bold(), entry.path);
        }
        for (target_entry, _) in &self.files_to_modify {
            println!("  {} {} (will revert)", "~".yellow().bold(), target_entry.path);
        }
        for (target_entry, _) in &self.modes_to_change {
            println!("  {} {} (will chmod to {:o})", "*".blue().bold(), target_entry.path, target_entry.mode);
        }
        println!();
    }

    pub fn execute(&self, workspace: &Path, store: &ObjectStore) -> io::Result<()> {
        // 1. Delete newly created files
        for rel_path in &self.files_to_delete {
            let full_path = workspace.join(rel_path);
            if full_path.exists() || fs::symlink_metadata(&full_path).is_ok() {
                fs::remove_file(&full_path)?;
                clean_empty_parents(&full_path, workspace)?;
            }
        }

        // 2. Restore deleted / renamed files
        for entry in &self.files_to_restore {
            let full_path = workspace.join(&entry.path);
            restore_single_entry(entry, &full_path, store)?;
        }

        // 3. Revert modified files
        for (target_entry, _) in &self.files_to_modify {
            let full_path = workspace.join(&target_entry.path);
            restore_single_entry(target_entry, &full_path, store)?;
        }

        // 4. Update file permissions
        for (target_entry, _) in &self.modes_to_change {
            let full_path = workspace.join(&target_entry.path);
            #[cfg(unix)]
            if full_path.exists() {
                fs::set_permissions(&full_path, fs::Permissions::from_mode(target_entry.mode))?;
            }
        }

        Ok(())
    }
}

fn restore_single_entry(entry: &FileEntry, full_path: &Path, store: &ObjectStore) -> io::Result<()> {
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if full_path.exists() || fs::symlink_metadata(full_path).is_ok() {
        let _ = fs::remove_file(full_path);
    }

    if entry.is_symlink {
        if let Some(target) = &entry.symlink_target {
            #[cfg(unix)]
            std::os::unix::fs::symlink(target, full_path)?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_file(target, full_path)?;
        }
    } else {
        store.copy_to_file(&entry.hash, full_path)?;
        #[cfg(unix)]
        {
            fs::set_permissions(full_path, fs::Permissions::from_mode(entry.mode))?;
        }
    }

    Ok(())
}

fn clean_empty_parents(path: &Path, root: &Path) -> io::Result<()> {
    let mut current = path.parent();
    while let Some(parent) = current {
        if parent == root || !parent.starts_with(root) {
            break;
        }
        if fs::read_dir(parent)?.next().is_none() {
            let _ = fs::remove_dir(parent);
            current = parent.parent();
        } else {
            break;
        }
    }
    Ok(())
}
