use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use super::AgentMetadata;

pub fn extract_metadata(workspace: &Path) -> Option<AgentMetadata> {
    let history_file = workspace.join(".aider.chat.history.md");
    if history_file.is_file() {
        if let Some(meta) = parse_aider_history(&history_file) {
            return Some(meta);
        }
    }

    let input_file = workspace.join(".aider.input.history");
    if input_file.is_file() {
        if let Ok(file) = File::open(&input_file) {
            let mut last = None;
            for line in BufReader::new(file).lines() {
                last = Some(line.ok()?);
            }
            if let Some(last) = last {
                return Some(AgentMetadata {
                    agent_name: Some("Aider".to_string()),
                    prompt: Some(last),
                    tool_calls: Vec::new(),
                });
            }
        }
    }

    Some(AgentMetadata {
        agent_name: Some("Aider".to_string()),
        prompt: None,
        tool_calls: Vec::new(),
    })
}

fn parse_aider_history(path: &Path) -> Option<AgentMetadata> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last_prompt = None;
    let mut tool_calls = Vec::new();

    for line in reader.lines() {
        let line = line.ok()?;
        let trimmed = line.trim();
        if trimmed.starts_with("#### >") {
            last_prompt = Some(trimmed.trim_start_matches("#### >").trim().to_string());
        } else if trimmed.starts_with("Applied edit to") {
            let file_target = trimmed
                .trim_start_matches("Applied edit to")
                .trim()
                .to_string();
            if !tool_calls.contains(&file_target) {
                tool_calls.push(format!("edit:{}", file_target));
            }
        }
    }

    Some(AgentMetadata {
        agent_name: Some("Aider".to_string()),
        prompt: last_prompt,
        tool_calls,
    })
}

#[cfg(test)]
mod tests {
    use super::{extract_metadata, parse_aider_history};

    #[test]
    fn rejects_transcript_read_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("synthetic.md");
        let valid = b"#### > synthetic prompt\nApplied edit to fixture.txt\n";
        std::fs::write(&path, valid).unwrap();
        let metadata = parse_aider_history(&path).unwrap();
        assert_eq!(metadata.prompt.as_deref(), Some("synthetic prompt"));
        assert_eq!(metadata.tool_calls, vec!["edit:fixture.txt"]);

        let corrupt = [valid.as_slice(), b"\xff\n", valid.as_slice()].concat();
        std::fs::write(&path, corrupt).unwrap();
        assert!(parse_aider_history(&path).is_none());
    }

    #[test]
    fn rejects_input_history_read_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".aider.input.history");
        std::fs::write(&path, b"first prompt\nlast prompt\n").unwrap();
        let metadata = extract_metadata(dir.path()).unwrap();
        assert_eq!(metadata.prompt.as_deref(), Some("last prompt"));

        std::fs::write(&path, b"first prompt\n\xff\nlast prompt\n").unwrap();
        assert!(extract_metadata(dir.path()).is_none());
    }
}
