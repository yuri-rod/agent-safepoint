use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ignore::WalkBuilder;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::cas::ObjectStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub mode: u32,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeManifest {
    pub tree_hash: String,
    pub total_files: usize,
    pub total_bytes: u64,
    pub entries: BTreeMap<String, FileEntry>,
}

impl TreeManifest {
    pub fn new() -> Self {
        Self {
            tree_hash: String::new(),
            total_files: 0,
            total_bytes: 0,
            entries: BTreeMap::new(),
        }
    }

    pub fn compute_tree_hash(&mut self) {
        let mut hasher = Sha256::new();
        for (path, entry) in &self.entries {
            hasher.update(path.as_bytes());
            hasher.update(entry.hash.as_bytes());
            hasher.update(&entry.size.to_le_bytes());
            hasher.update(&entry.mode.to_le_bytes());
            if let Some(target) = &entry.symlink_target {
                hasher.update(target.as_bytes());
            }
        }
        self.tree_hash = format!("{:x}", hasher.finalize());
        self.total_files = self.entries.len();
        self.total_bytes = self.entries.values().map(|e| e.size).sum();
    }

    pub fn scan(workspace: &Path, store: &ObjectStore) -> io::Result<Self> {
        let mut manifest = Self::new();

        let walker = WalkBuilder::new(workspace)
            .hidden(false)
            .parents(true)
            .ignore(true)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(true)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                name != ".safepoint" && name != ".git"
            })
            .build();

        for result in walker {
            let entry = match result {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("Warning: skipping unreadable entry: {}", err);
                    continue;
                }
            };

            let path = entry.path();
            if path == workspace {
                continue;
            }

            let symlink_meta = match fs::symlink_metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if symlink_meta.is_dir() {
                continue;
            }

            let rel_path = match path.strip_prefix(workspace) {
                Ok(p) => p.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };

            #[cfg(unix)]
            let mode = symlink_meta.permissions().mode();
            #[cfg(not(unix))]
            let mode = 0o644;

            let is_symlink = symlink_meta.file_type().is_symlink();

            if is_symlink {
                let target = match fs::read_link(path) {
                    Ok(t) => t.to_string_lossy().replace('\\', "/"),
                    Err(_) => continue,
                };
                let hash = format!("{:x}", Sha256::digest(target.as_bytes()));
                manifest.entries.insert(
                    rel_path.clone(),
                    FileEntry {
                        path: rel_path,
                        hash,
                        size: target.len() as u64,
                        mode,
                        is_symlink: true,
                        symlink_target: Some(target),
                    },
                );
            } else if symlink_meta.is_file() {
                let (hash, size) = store.write_file(path)?;
                manifest.entries.insert(
                    rel_path.clone(),
                    FileEntry {
                        path: rel_path,
                        hash,
                        size,
                        mode,
                        is_symlink: false,
                        symlink_target: None,
                    },
                );
            }
        }

        manifest.compute_tree_hash();
        Ok(manifest)
    }

    pub fn save(&self, store: &ObjectStore) -> io::Result<String> {
        let json_bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        store.write_blob(&json_bytes)
    }

    pub fn load(hash: &str, store: &ObjectStore) -> io::Result<Self> {
        let bytes = store.read_blob(hash)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
