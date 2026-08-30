use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use colored::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: u64,
    pub tag: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
    pub tree_hash: String,
    pub parent_tree_hash: Option<String>,
    pub summary: String,
    pub total_files: usize,
    pub total_bytes: u64,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub agent_prompt: Option<String>,
    #[serde(default)]
    pub agent_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Journal {
    pub current_id: u64,
    pub checkpoints: Vec<Checkpoint>,
}

impl Journal {
    pub fn journal_path(safepoint_dir: &Path) -> PathBuf {
        safepoint_dir.join("journal.json")
    }

    pub fn load(safepoint_dir: &Path) -> io::Result<Self> {
        let path = Self::journal_path(safepoint_dir);
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(&path)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, safepoint_dir: &Path) -> io::Result<()> {
        let path = Self::journal_path(safepoint_dir);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json_bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, json_bytes)
    }

    pub fn add(
        &mut self,
        tree_hash: String,
        parent_tree_hash: Option<String>,
        summary: String,
        total_files: usize,
        total_bytes: u64,
        command: Option<String>,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        tag: Option<String>,
        agent_name: Option<String>,
        agent_prompt: Option<String>,
        agent_tools: Vec<String>,
    ) -> Checkpoint {
        self.current_id += 1;
        let checkpoint = Checkpoint {
            id: self.current_id,
            tag,
            timestamp: Utc::now(),
            command,
            exit_code,
            duration_ms,
            tree_hash,
            parent_tree_hash,
            summary,
            total_files,
            total_bytes,
            agent_name,
            agent_prompt,
            agent_tools,
        };
        self.checkpoints.push(checkpoint.clone());
        checkpoint
    }

    pub fn latest(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    pub fn find_by_selector(&self, selector: &str) -> Option<&Checkpoint> {
        let clean = selector.trim().trim_start_matches('@');
        if clean.eq_ignore_ascii_case("latest") || clean.eq_ignore_ascii_case("head") {
            return self.latest();
        }

        if let Ok(id) = clean.parse::<u64>() {
            return self.checkpoints.iter().find(|c| c.id == id);
        }

        self.checkpoints.iter().find(|c| {
            c.tag.as_deref().map(|t| t.eq_ignore_ascii_case(selector)).unwrap_or(false)
                || c.tree_hash.starts_with(selector)
        })
    }

    pub fn print_timeline(&self) {
        if self.checkpoints.is_empty() {
            println!("No checkpoints recorded yet.");
            return;
        }

        println!("\n{}", "=== Safepoint Timeline ===".cyan().bold());
        for cp in self.checkpoints.iter().rev() {
            let time_str = cp.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string();
            let id_str = format!("@{}", cp.id).yellow().bold();
            let tag_str = cp
                .tag
                .as_ref()
                .map(|t| format!(" [{}]", t.green()))
                .unwrap_or_default();

            println!("{} {} - {}{}", id_str, time_str.dimmed(), cp.summary, tag_str);

            if let Some(cmd) = &cp.command {
                let code_str = match cp.exit_code {
                    Some(0) => "0".green(),
                    Some(c) => format!("{}", c).red(),
                    None => "-".normal(),
                };
                let dur_str = cp
                    .duration_ms
                    .map(|d| format!(" ({}ms)", d))
                    .unwrap_or_default();
                println!("    {} `{}` [exit: {}{}]", "cmd:".dimmed(), cmd, code_str, dur_str.dimmed());
            }

            if let Some(agent) = &cp.agent_name {
                let tools_str = if !cp.agent_tools.is_empty() {
                    format!(" | tools: {}", cp.agent_tools.join(", ").magenta())
                } else {
                    String::new()
                };
                println!("    {} {}{}", "agent:".dimmed(), agent.bold(), tools_str);
            }

            if let Some(prompt) = &cp.agent_prompt {
                let truncated = if prompt.len() > 80 {
                    format!("{}...", &prompt[..77])
                } else {
                    prompt.clone()
                };
                println!("    {} \"{}\"", "prompt:".dimmed(), truncated.italic());
            }

            println!("    {} {} files ({:.2} MB) | tree: {:.8}",
                "tree:".dimmed(),
                cp.total_files,
                cp.total_bytes as f64 / 1_048_576.0,
                cp.tree_hash
            );
            println!();
        }
    }
}
