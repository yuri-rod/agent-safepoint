# Changelog

## 0.1.2 - 2026-10-27

- Validate checkpoint hashes and restore paths before changing workspace files.
- Reject corrupted, missing and nonregular checkpoint objects, including symlinks and FIFOs.
- Stage restored files before replacing their destinations, and reject paths through symlinked directories or protected Git/checkpoint metadata.
- Add recovery regressions and a reproducible temporary-workspace example, with documented interruption and concurrency limits.
- Run the declared formatting and Clippy checks in CI with read-only repository permissions, and include the existing MIT license in the source tree.
