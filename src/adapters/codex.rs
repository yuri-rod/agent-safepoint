use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use serde_json::Value;

use super::AgentMetadata;

pub fn extract_metadata(workspace: &Path) -> Option<AgentMetadata> {
    let mut candidate_dirs = Vec::new();

    // 1. Workspace local .codex
    candidate_dirs.push(workspace.join(".codex"));

    // 2. User home .codex sessions
    if let Some(home) = dirs_home() {
        candidate_dirs.push(home.join(".codex").join("sessions"));
        candidate_dirs.push(home.join(".codex"));
    }

    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }
        if let Some(file_path) = find_newest_transcript(&dir) {
            if let Some(meta) = parse_transcript(&file_path) {
                return Some(meta);
            }
        }
    }

    None
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

fn find_newest_transcript(dir: &Path) -> Option<PathBuf> {
    let mut newest_file = None;
    let mut newest_time = std::time::UNIX_EPOCH;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && (path.extension().map_or(false, |ext| ext == "jsonl" || ext == "json")) {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified > newest_time {
                            newest_time = modified;
                            newest_file = Some(path);
                        }
                    }
                }
            } else if path.is_dir() {
                if let Some(nested) = find_newest_transcript(&path) {
                    if let Ok(meta) = nested.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if modified > newest_time {
                                newest_time = modified;
                                newest_file = Some(nested);
                            }
                        }
                    }
                }
            }
        }
    }

    newest_file
}

fn parse_transcript(path: &Path) -> Option<AgentMetadata> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut prompt = None;
    let mut tool_calls = Vec::new();

    for line in reader.lines().flatten() {
        if let Ok(val) = serde_json::from_str::<Value>(&line) {
            if let Some(t) = val.get("type").and_then(|s| s.as_str()) {
                if t == "USER_INPUT" || t == "user" {
                    if let Some(content) = val.get("content").and_then(|s| s.as_str()) {
                        prompt = Some(content.trim().to_string());
                    }
                }
            }

            if let Some(tools) = val.get("tool_calls").and_then(|a| a.as_array()) {
                for tool in tools {
                    if let Some(name) = tool.get("name").and_then(|s| s.as_str()) {
                        let name_str = name.to_string();
                        if !tool_calls.contains(&name_str) {
                            tool_calls.push(name_str);
                        }
                    }
                }
            }
        }
    }

    Some(AgentMetadata {
        agent_name: Some("Codex".to_string()),
        prompt,
        tool_calls,
    })
}
