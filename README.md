# agent-safepoint

[![CI](https://github.com/yuri-rod/agent-safepoint/actions/workflows/ci.yml/badge.svg)](https://github.com/yuri-rod/agent-safepoint/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/yuri-rod/agent-safepoint?color=blue)](https://github.com/yuri-rod/agent-safepoint/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Universal, local-first undo and recovery layer for coding agents.**

`safepoint` wraps tools like Codex, Claude Code, OpenCode, Aider, and Gemini CLI, checkpoints a workspace, and restores file changes detected there, including changes made through shell commands and scripts.

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
2. **Git history stays separate**: `safepoint` stores snapshots in `.safepoint/` without creating commits or changing refs. A recovery test confirms that undo leaves `HEAD` unchanged.
3. **Uncommitted work is recoverable**: Files and changes present before a checkpoint, including untracked files, are included in the snapshot and restored by undo.
4. **Content-addressed storage**: SHA-256 blobs are reused for identical file contents across checkpoints.
5. **Agent transcript aware**: Automatically detects Codex, Claude Code, OpenCode, and Aider sessions, linking user prompts and tool calls to each checkpoint.

---

## What Recovery Covers

After running `safepoint undo @n`:
- Files added after a checkpoint are removed by undo.
- Deleted files are restored with their saved contents.
- Modified files return to their saved contents and, on Unix, permissions.
- Untracked work present before the checkpoint is restored if it is changed or deleted afterward.
- Undo does not create Git commits. The recovery test checks that `HEAD` is unchanged.

The adversarial recovery tests exercise added, modified, deleted, and binary files, plus files whose permissions change and uncommitted work present before the checkpoint. They do not establish coverage for every filesystem type or excluded path.

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
