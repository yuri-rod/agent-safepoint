# agent-safepoint

Checkpoint workspace files, inspect changes, and restore an earlier state without creating Git commits or changing refs.

Use `safepoint` before a coding-agent session, a refactoring script, or another command that edits files. Snapshots include uncommitted and untracked work, subject to the ignore rules described below.

## Install

Install [Rust stable](https://www.rust-lang.org/tools/install), then build the CLI from this repository:

```sh
cargo install --locked --git https://github.com/yuri-rod/agent-safepoint.git
safepoint --version
```

For development:

```sh
git clone https://github.com/yuri-rod/agent-safepoint.git
cd agent-safepoint
cargo build --locked
```

The development binary is `target/debug/safepoint`. The checked-in [Homebrew formula](Formula/safepoint.rb) is packaging source; Cargo is the documented installation path.

## Try a recovery

This example uses a disposable directory and a shell command. It needs no agent account, model, or network service after installation.

```sh
demo=$(mktemp -d)
printf 'original draft\n' > "$demo/draft.txt"
safepoint -C "$demo" checkpoint --tag before --message 'before script'
safepoint -C "$demo" run -- sh -c 'printf "changed draft\n" > draft.txt; printf "scratch\n" > scratch.txt'
safepoint -C "$demo" diff before
safepoint -C "$demo" undo before --dry-run
safepoint -C "$demo" undo before
cat "$demo/draft.txt"
test ! -e "$demo/scratch.txt" && printf 'scratch removed\n'
```

The final two commands print:

```text
original draft
scratch removed
```

`undo` overwrites files that differ from the selected checkpoint and removes included files added afterward. Review `--dry-run` first. Create a new manual checkpoint if you also want to retain the current edits.

For a real session, create a named checkpoint first, then wrap the command:

```sh
safepoint checkpoint --tag before-refactor
safepoint run -- codex
safepoint diff before-refactor
```

## Commands

| Command | Purpose |
| --- | --- |
| `checkpoint -m "message" --tag name` | Save a selectable workspace snapshot |
| `run -- command args...` | Save a baseline, run a command, and journal its resulting state and exit code |
| `timeline` | List checkpoints, commands, and available agent metadata |
| `status` | Compare the workspace with the latest checkpoint |
| `diff before-refactor` | Show changes since a checkpoint |
| `diff @2 @1` | Compare checkpoint 1 with checkpoint 2 |
| `undo before-refactor --dry-run` | Preview removals, replacements, and permission changes |
| `undo before-refactor` | Apply the restore plan |
| `list-files before-refactor` | Inspect the selected snapshot's file list |
| `completions bash` | Generate shell completion definitions |

Selectors accept a checkpoint number such as `@1`, a tag, or `latest`. Tags should be unique: the first matching tag is selected. `-C` explicitly selects a workspace; otherwise the CLI searches parent directories for `.safepoint` or `.git`.

## Recovery and storage

- [Workspace scanning](src/core/tree.rs) records regular files, symbolic links, contents, and Unix file permissions. `.git` and `.safepoint` are excluded. `.ignore`, repository `.gitignore`, and Git exclude rules also affect coverage; ignored files are not protected.
- [The object store](src/core/cas.rs) reuses SHA-256-addressed contents. Reads require regular files and verify each object's digest. Regular-file restoration writes a temporary file beside the destination, verifies it, and replaces the destination atomically.
- [Restore planning](src/core/restore.rs) checks affected paths and required objects before removing current files. Missing or corrupt contents, paths outside the workspace, Git/store metadata paths, and symlink parents stop restoration. Permission changes do not follow symbolic links.
- [The journal](src/core/journal.rs) records checkpoint IDs, tags, timestamps, and command results in `.safepoint/journal.json`. Undo changes workspace files, not Git commits, refs, or the index.
- [Transcript adapters](src/adapters) can attach available metadata for recognized agent commands. Codex, Claude, and Gemini adapters also search home-directory logs; newest-file selection does not prove that a transcript belongs to the current command. OpenCode currently supplies only the agent name.

Keep `.safepoint/` out of version control and shared artifacts: it contains copies of workspace files and may contain transcript text. It is not encrypted.

Stop other writers before checkpointing or restoring. A restore is not a transaction across the entire workspace: interruption or a later filesystem error can leave only part of a valid plan applied. Fix the reported problem and run the same undo again. Empty directories, ownership, ACLs, extended attributes, and non-UTF-8 filenames are not preserved.

`run` writes its timeline entry after the child exits. Its automatic pre-run baseline is not a separate selectable checkpoint, so use the manual tag shown above for rollback and interrupted runs. The wrapper does not implement signal forwarding. The optional `hook install` command replaces `.git/hooks/pre-rebase`; preserve an existing hook before using it.

## Tests and development

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --locked --release
```

The tests create disposable directories and Git repositories:

- [Recovery scenarios](tests/adversarial_recovery.rs): added, deleted, modified, binary, permission-changed, and uncommitted files; multiple checkpoints; unchanged Git HEAD.
- [Adapter parsing](src/adapters): synthetic transcripts verify valid metadata and rejection of read/UTF-8 decoding errors, without accessing home-directory sessions.
- [Recovery safety](tests/recovery_safety.rs): missing/corrupt objects, symlinks and FIFOs in the object store, invalid object IDs, unsafe paths, symlink redirection, and preservation of current work when validation fails.

CI runs on Linux and macOS. Its formatting check covers changed Rust files. Before contributing, format each changed Rust file with `rustfmt --edition 2021 path/to/file.rs`, run the relevant regression and full suite, and describe any checks that could not run. Keep filesystem-destructive tests inside disposable workspaces.

## License

[MIT](LICENSE).
