use std::collections::HashSet;
use colored::*;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use super::cas::ObjectStore;
use super::tree::{FileEntry, TreeManifest};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileDelta {
    Added(FileEntry),
    Deleted(FileEntry),
    Modified { from: FileEntry, to: FileEntry },
    ModeChanged { from: FileEntry, to: FileEntry },
    Renamed { from: FileEntry, to: FileEntry },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeDiff {
    pub added: Vec<FileEntry>,
    pub deleted: Vec<FileEntry>,
    pub modified: Vec<(FileEntry, FileEntry)>,
    pub mode_changed: Vec<(FileEntry, FileEntry)>,
    pub renamed: Vec<(FileEntry, FileEntry)>,
}

impl TreeDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.deleted.is_empty()
            && self.modified.is_empty()
            && self.mode_changed.is_empty()
            && self.renamed.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.added.len()
            + self.deleted.len()
            + self.modified.len()
            + self.mode_changed.len()
            + self.renamed.len()
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.added.is_empty() {
            parts.push(format!("+{} added", self.added.len()));
        }
        if !self.modified.is_empty() {
            parts.push(format!("~{} modified", self.modified.len()));
        }
        if !self.deleted.is_empty() {
            parts.push(format!("-{} deleted", self.deleted.len()));
        }
        if !self.renamed.is_empty() {
            parts.push(format!("→{} renamed", self.renamed.len()));
        }
        if !self.mode_changed.is_empty() {
            parts.push(format!("*{} chmod", self.mode_changed.len()));
        }
        if parts.is_empty() {
            "No changes".to_string()
        } else {
            parts.join(", ")
        }
    }

    pub fn compute(from: &TreeManifest, to: &TreeManifest) -> Self {
        let mut added = Vec::new();
        let mut deleted = Vec::new();
        let mut modified = Vec::new();
        let mut mode_changed = Vec::new();
        let mut renamed = Vec::new();

        let mut matched_added_paths: HashSet<String> = HashSet::new();

        // Check modified, mode_changed, and deleted
        for (path, from_entry) in &from.entries {
            match to.entries.get(path) {
                Some(to_entry) => {
                    if from_entry.hash != to_entry.hash
                        || from_entry.symlink_target != to_entry.symlink_target
                    {
                        modified.push((from_entry.clone(), to_entry.clone()));
                    } else if from_entry.mode != to_entry.mode {
                        mode_changed.push((from_entry.clone(), to_entry.clone()));
                    }
                }
                None => {
                    deleted.push(from_entry.clone());
                }
            }
        }

        // Check added
        for (path, to_entry) in &to.entries {
            if !from.entries.contains_key(path) {
                added.push(to_entry.clone());
            }
        }

        // Heuristic: Match exact renames (same content hash across deleted and added)
        let mut remaining_deleted = Vec::new();
        for del in deleted {
            if let Some(pos) = added.iter().position(|add| add.hash == del.hash && !matched_added_paths.contains(&add.path)) {
                let add = added.remove(pos);
                matched_added_paths.insert(add.path.clone());
                renamed.push((del, add));
            } else {
                remaining_deleted.push(del);
            }
        }

        Self {
            added,
            deleted: remaining_deleted,
            modified,
            mode_changed,
            renamed,
        }
    }

    pub fn print_summary(&self) {
        for (from, to) in &self.renamed {
            println!("  {} {} -> {}", "R".magenta().bold(), from.path, to.path);
        }
        for entry in &self.added {
            println!("  {} {}", "+".green().bold(), entry.path);
        }
        for (from, _to) in &self.modified {
            println!("  {} {}", "~".yellow().bold(), from.path);
        }
        for (from, _to) in &self.mode_changed {
            println!("  {} {} (mode changed)", "*".blue().bold(), from.path);
        }
        for entry in &self.deleted {
            println!("  {} {}", "-".red().bold(), entry.path);
        }
    }

    pub fn render_diff(&self, store: &ObjectStore) {
        self.print_summary();
        println!();

        for (from_entry, to_entry) in &self.modified {
            println!("{}", format!("--- a/{}", from_entry.path).red());
            println!("{}", format!("+++ b/{}", to_entry.path).green());

            let from_bytes = store.read_blob(&from_entry.hash).unwrap_or_default();
            let to_bytes = store.read_blob(&to_entry.hash).unwrap_or_default();

            let from_str = String::from_utf8_lossy(&from_bytes);
            let to_str = String::from_utf8_lossy(&to_bytes);

            let is_binary = from_bytes.contains(&0) || to_bytes.contains(&0);
            if is_binary {
                println!("  [Binary file differs ({} bytes -> {} bytes)]\n", from_bytes.len(), to_bytes.len());
                continue;
            }

            let diff = TextDiff::from_lines(&from_str, &to_str);
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-".red(),
                    ChangeTag::Insert => "+".green(),
                    ChangeTag::Equal => " ".normal(),
                };
                print!("{}{}", sign, change);
            }
            println!();
        }

        for entry in &self.added {
            println!("{}", format!("--- /dev/null").red());
            println!("{}", format!("+++ b/{}", entry.path).green());

            let bytes = store.read_blob(&entry.hash).unwrap_or_default();
            if bytes.contains(&0) {
                println!("  [New binary file ({} bytes)]\n", bytes.len());
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines() {
                println!("{}{}", "+".green(), line);
            }
            println!();
        }

        for entry in &self.deleted {
            println!("{}", format!("--- a/{}", entry.path).red());
            println!("{}", format!("+++ /dev/null").green());

            let bytes = store.read_blob(&entry.hash).unwrap_or_default();
            if bytes.contains(&0) {
                println!("  [Deleted binary file ({} bytes)]\n", bytes.len());
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines() {
                println!("{}{}", "-".red(), line);
            }
            println!();
        }
    }
}
