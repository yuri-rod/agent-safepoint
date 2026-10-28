use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use super::AgentMetadata;

pub fn extract_metadata(workspace: &Path) -> Option<AgentMetadata> {
    let mut candidate_dirs = Vec::new();

    // 1. Workspace local .claude
    candidate_dirs.push(workspace.join(".claude"));

    // 2. User home .claude projects
    if let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) {
        candidate_dirs.push(home.join(".claude").join("projects"));
        candidate_dirs.push(home.join(".claude"));
    }

    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }
        if let Some(file_path) = find_newest_json(&dir) {
            if let Some(meta) = parse_claude_session(&file_path) {
                return Some(meta);
            }
        }
    }

    None
}

fn find_newest_json(dir: &Path) -> Option<PathBuf> {
    let mut newest_file = None;
    let mut newest_time = std::time::UNIX_EPOCH;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && (path
                    .extension()
                    .is_some_and(|ext| ext == "json" || ext == "jsonl"))
            {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if modified > newest_time {
                            newest_time = modified;
                            newest_file = Some(path);
                        }
                    }
                }
            } else if path.is_dir() {
                if let Some(nested) = find_newest_json(&path) {
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

fn parse_claude_session(path: &Path) -> Option<AgentMetadata> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut prompt = None;
    let mut tool_calls = Vec::new();

    for line in reader.lines() {
        let line = line.ok()?;
        if let Ok(val) = serde_json::from_str::<Value>(&line) {
            if let Some(role) = val.get("role").and_then(|s| s.as_str()) {
                if role == "user" {
                    if let Some(text) = val.get("content").and_then(|s| s.as_str()) {
                        prompt = Some(text.trim().to_string());
                    }
                }
            }

            if let Some(tool_name) = val.get("tool").and_then(|s| s.as_str()) {
                let name_str = tool_name.to_string();
                if !tool_calls.contains(&name_str) {
                    tool_calls.push(name_str);
                }
            }
        }
    }

    Some(AgentMetadata {
        agent_name: Some("Claude Code".to_string()),
        prompt,
        tool_calls,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_claude_session;

    #[test]
    fn rejects_transcript_read_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.jsonl");
        let valid = br#"{"role":"user","content":"synthetic prompt","tool":"read"}
"#;
        std::fs::write(&path, valid).unwrap();
        let metadata = parse_claude_session(&path).unwrap();
        assert_eq!(metadata.prompt.as_deref(), Some("synthetic prompt"));
        assert_eq!(metadata.tool_calls, vec!["read"]);

        let corrupt = [valid.as_slice(), b"\xff\n", valid.as_slice()].concat();
        std::fs::write(&path, corrupt).unwrap();
        assert!(parse_claude_session(&path).is_none());
    }
}
