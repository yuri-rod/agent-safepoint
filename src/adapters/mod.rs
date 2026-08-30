pub mod codex;
pub mod claude;
pub mod aider;
pub mod gemini;

use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AgentMetadata {
    pub agent_name: Option<String>,
    pub prompt: Option<String>,
    pub tool_calls: Vec<String>,
}

pub fn detect_and_extract(workspace: &Path, command_str: &str) -> AgentMetadata {
    let lower_cmd = command_str.to_lowercase();
    if lower_cmd.contains("codex") {
        if let Some(meta) = codex::extract_metadata(workspace) {
            return meta;
        }
        return AgentMetadata {
            agent_name: Some("Codex".to_string()),
            prompt: None,
            tool_calls: Vec::new(),
        };
    } else if lower_cmd.contains("claude") {
        if let Some(meta) = claude::extract_metadata(workspace) {
            return meta;
        }
        return AgentMetadata {
            agent_name: Some("Claude Code".to_string()),
            prompt: None,
            tool_calls: Vec::new(),
        };
    } else if lower_cmd.contains("gemini") || lower_cmd.contains("agy") || lower_cmd.contains("antigravity") {
        if let Some(meta) = gemini::extract_metadata(workspace) {
            return meta;
        }
        return AgentMetadata {
            agent_name: Some("Gemini/Antigravity".to_string()),
            prompt: None,
            tool_calls: Vec::new(),
        };
    } else if lower_cmd.contains("aider") {
        if let Some(meta) = aider::extract_metadata(workspace) {
            return meta;
        }
        return AgentMetadata {
            agent_name: Some("Aider".to_string()),
            prompt: None,
            tool_calls: Vec::new(),
        };
    } else if lower_cmd.contains("opencode") {
        return AgentMetadata {
            agent_name: Some("OpenCode".to_string()),
            prompt: None,
            tool_calls: Vec::new(),
        };
    }

    AgentMetadata::default()
}
