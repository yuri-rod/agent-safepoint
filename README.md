# agent-safepoint

[![CI](https://github.com/yuri-rod/agent-safepoint/actions/workflows/ci.yml/badge.svg)](https://github.com/yuri-rod/agent-safepoint/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/yuri-rod/agent-safepoint?color=blue)](https://github.com/yuri-rod/agent-safepoint/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Universal, local-first undo and recovery layer for coding agents.**

`safepoint` wraps tools like Codex, Claude Code, OpenCode, Aider, and Gemini CLI, records what the agent did, checkpoints the workspace, and safely restores files created, modified, renamed, or deleted, even when changes were made through Bash subshells, Python scripts, heredocs, or arbitrary shell commands.

```bash
# Wrap any agent run with automatic pre/post recovery checkpoints
$ safepoint run -- claude
$ safepoint run -- codex
$ safepoint run -- python worker.py

# Inspect timeline, prompts, tool invocations, and visual unified diffs
$ safepoint timeline
$ safepoint diff @1

# Instantly revert workspace to clean state without touching Git history
$ safepoint undo @1
```

---

## Why Safepoint?

1. **Vendor rewinds are leaky**: Built-in rewinds (like Claude Code rewind) miss files created or altered through Bash subshells, `sed`, or script executions.
2. **Git is untouched**: Unlike naive git checkpoint wrappers, `safepoint` operates an independent content-addressed store in `.safepoint/`. It never mutates `git HEAD`, refs, staging index, or stash history.
3. **Untracked & pre-existing work protected**: Untracked scratch files, config files, and uncommitted edits are fully protected and recoverable.
4. **Instant & deduplicated**: Content-addressed blob store (SHA-256) ensures identical files across checkpoints cost zero additional disk space.
5. **Agent transcript aware**: Automatically detects Codex, Claude Code, OpenCode, and Aider sessions, linking user prompts and tool calls to each checkpoint.

---

## The Invariant Restoration Contract

After running `safepoint undo @n`:
- **Created files** (by agent or shell commands) are completely deleted.
- **Modified files** are restored byte-for-byte to their original contents and file permissions.
- **Deleted files** are revived with exact original bytes and Unix file modes.
- **Renamed files** are restored to their original paths.
- **Pre-existing uncommitted work** remains untouched.
- **Git history** remains 100% clean and unaltered.

---

## CLI Command Reference

| Command | Description | Example |
| :--- | :--- | :--- |
| `run` | Wraps command with automatic pre/post checkpoints | `safepoint run -- codex` |
| `checkpoint` | Creates a manual checkpoint of workspace state | `safepoint checkpoint -m "clean base"` |
| `timeline` | Displays timeline of recorded checkpoints with prompts and tools | `safepoint timeline` |
| `status` | Shows workspace modifications relative to latest checkpoint | `safepoint status` |
| `diff` | Renders unified line-by-line diff against a checkpoint | `safepoint diff @1` |
| `undo` | Restores workspace byte-for-byte to a prior checkpoint | `safepoint undo @1` |
| `list-files` | Lists all files tracked in a given checkpoint | `safepoint list-files @1` |

---

## Installation

### Via Cargo

```bash
cargo install --git https://github.com/yuri-rod/agent-safepoint.git
```

### From Source

```bash
git clone https://github.com/yuri-rod/agent-safepoint.git
cd agent-safepoint
cargo build --release
cp target/release/safepoint /usr/local/bin/
```

### Homebrew

```bash
brew install yuri-rod/tap/safepoint
```

---

## Architecture

```
Command Wrapper (safepoint run -- <cmd>)
    ↓
Pre-run Baseline Manifest & Content-Addressed Store (CAS)
    ↓
Subprocess Execution & Signal Forwarding
    ↓
Agent Transcript & Metadata Adapter (Codex, Claude Code, OpenCode, Aider)
    ↓
Post-run Mutation Scanner & Merkle Tree
    ↓
Timeline & Journal Ledger (.safepoint/journal.json)
    ↓
Conflict-Aware Restoration Engine (safepoint undo @n)
```

---

## License

MIT License.
