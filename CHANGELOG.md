# Changelog

## Unreleased

- Reject transcript files on read or UTF-8 decoding errors instead of silently
  skipping failures or repeatedly retrying a failed read. Cover all four agent
  adapters and Aider input history with synthetic regression fixtures.
- Keep only the last Aider input-history line in memory.
- Add the standard default constructor for tree manifests and remove redundant
  formatting, hashing borrows and transcript-extension checks flagged by Clippy.

## 0.1.2 - 2026-10-27

- Validate checkpoint hashes and restore paths before changing workspace files.
- Reject corrupted, missing and nonregular checkpoint objects, including symlinks and FIFOs.
- Stage restored files before replacing their destinations, and reject paths through symlinked directories or protected Git/checkpoint metadata.
- Add recovery regressions and a reproducible temporary-workspace example, with documented interruption and concurrency limits.
- Run the declared formatting and Clippy checks in CI with read-only repository permissions, and include the existing MIT license in the source tree.
