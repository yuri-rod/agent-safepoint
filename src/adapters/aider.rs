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
            let lines: Vec<String> = BufReader::new(file).lines().flatten().collect();
            if let Some(last) = lines.last() {
                return Some(AgentMetadata {
                    agent_name: Some("Aider".to_string()),
                    prompt: Some(last.clone()),
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

    for line in reader.lines().flatten() {
        let trimmed = line.trim();
        if trimmed.starts_with("#### >") {
            last_prompt = Some(trimmed.trim_start_matches("#### >").trim().to_string());
        } else if trimmed.starts_with("Applied edit to") {
            let file_target = trimmed.trim_start_matches("Applied edit to").trim().to_string();
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
