use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use safepoint::core::cas::ObjectStore;
use safepoint::core::diff::TreeDiff;
use safepoint::core::journal::Journal;
use safepoint::core::restore::RestorePlan;
use safepoint::core::tree::TreeManifest;

fn create_file(path: &Path, content: &[u8], mode: Option<u32>) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = File::create(path).unwrap();
    file.write_all(content).unwrap();
    file.flush().unwrap();
    #[cfg(unix)]
    if let Some(m) = mode {
        fs::set_permissions(path, fs::Permissions::from_mode(m)).unwrap();
    }
}

#[test]
fn test_adversarial_full_recovery() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path();
    let safepoint_dir = workspace.join(".safepoint");

    let store = ObjectStore::new(&safepoint_dir).unwrap();
    let mut journal = Journal::load(&safepoint_dir).unwrap();

    // 1. Initial State
    create_file(&workspace.join("README.md"), b"# Original Project\nKeep this safe.\n", Some(0o644));
    create_file(&workspace.join("src/lib.rs"), b"pub fn original() -> bool { true }\n", Some(0o644));
    create_file(&workspace.join("scripts/run.sh"), b"#!/bin/bash\necho 'running'\n", Some(0o755));
    create_file(&workspace.join("assets/binary.dat"), &[0x00, 0xFF, 0xAA, 0x55, 0xDE, 0xAD, 0xBE, 0xEF], Some(0o644));

    // 2. Snapshot Checkpoint @1
    let baseline = TreeManifest::scan(workspace, &store).unwrap();
    let hash1 = baseline.save(&store).unwrap();
    let cp1 = journal.add(
        hash1.clone(),
        None,
        "Baseline @1".to_string(),
        baseline.total_files,
        baseline.total_bytes,
        None,
        None,
        None,
        Some("v1".to_string()),
    );
    journal.save(&safepoint_dir).unwrap();

    assert_eq!(cp1.id, 1);
    assert_eq!(baseline.total_files, 4);

    // 3. Simulate Adversarial Agent Chaos
    // A: Delete pre-existing file
    fs::remove_file(workspace.join("README.md")).unwrap();

    // B: Mutate pre-existing file content
    create_file(&workspace.join("src/lib.rs"), b"pub fn corrupted_by_agent() { panic!(); }\n", Some(0o644));

    // C: Change script permissions
    #[cfg(unix)]
    fs::set_permissions(workspace.join("scripts/run.sh"), fs::Permissions::from_mode(0o600)).unwrap();

    // D: Create new untracked nested trash files
    create_file(&workspace.join("src/deep/nested/temp_agent_file.txt"), b"agent debris 1\n", Some(0o644));
    create_file(&workspace.join("tmp_scratch.py"), b"import os\nprint('temp')\n", Some(0o644));
    create_file(&workspace.join("assets/corrupted_image.png"), &[0x89, 0x50, 0x4E, 0x47, 0x00, 0x00], Some(0o644));

    // 4. Verify Diff sees all chaos
    let current_tree = TreeManifest::scan(workspace, &store).unwrap();
    let diff = TreeDiff::compute(&baseline, &current_tree);

    assert_eq!(diff.deleted.len(), 1, "Should detect deleted README.md");
    assert_eq!(diff.modified.len(), 1, "Should detect modified lib.rs");
    assert_eq!(diff.added.len(), 3, "Should detect 3 new files");
    #[cfg(unix)]
    assert_eq!(diff.mode_changed.len(), 1, "Should detect chmod on run.sh");

    // 5. Execute Undo / Rollback to @1
    let plan = RestorePlan::build(&baseline, &current_tree);
    plan.execute(workspace, &store).unwrap();

    // 6. Hard Verification of Post-Restore Workspace
    let restored_tree = TreeManifest::scan(workspace, &store).unwrap();
    assert_eq!(restored_tree.tree_hash, baseline.tree_hash, "Tree hash must match baseline exactly");
    assert_eq!(restored_tree.total_files, 4, "Must have exactly the original 4 files");

    // Check individual files
    assert_eq!(
        fs::read(workspace.join("README.md")).unwrap(),
        b"# Original Project\nKeep this safe.\n",
        "README.md must be fully revived"
    );
    assert_eq!(
        fs::read(workspace.join("src/lib.rs")).unwrap(),
        b"pub fn original() -> bool { true }\n",
        "src/lib.rs must be reverted to original content"
    );
    assert_eq!(
        fs::read(workspace.join("assets/binary.dat")).unwrap(),
        &[0x00, 0xFF, 0xAA, 0x55, 0xDE, 0xAD, 0xBE, 0xEF],
        "binary.dat must be bit-identical"
    );

    #[cfg(unix)]
    {
        let mode = fs::metadata(workspace.join("scripts/run.sh")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "scripts/run.sh permission must be restored to 0755");
    }

    // Verify created files and empty directories are completely gone
    assert!(!workspace.join("src/deep/nested/temp_agent_file.txt").exists(), "Agent debris must be deleted");
    assert!(!workspace.join("src/deep").exists(), "Empty directory tree must be pruned");
    assert!(!workspace.join("tmp_scratch.py").exists(), "Root scratch file must be deleted");
    assert!(!workspace.join("assets/corrupted_image.png").exists(), "Added image must be deleted");
}

#[test]
fn test_git_zero_pollution() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path();
    let safepoint_dir = workspace.join(".safepoint");

    // Init real git repo
    Command::new("git").args(["init"]).current_dir(workspace).output().unwrap();
    Command::new("git").args(["config", "user.name", "Safepoint Test"]).current_dir(workspace).output().unwrap();
    Command::new("git").args(["config", "user.email", "test@safepoint.dev"]).current_dir(workspace).output().unwrap();

    create_file(&workspace.join("committed.txt"), b"committed content\n", None);
    Command::new("git").args(["add", "committed.txt"]).current_dir(workspace).output().unwrap();
    Command::new("git").args(["commit", "-m", "initial commit"]).current_dir(workspace).output().unwrap();

    // Add dirty uncommitted work
    create_file(&workspace.join("dirty_uncommitted.txt"), b"my uncommitted draft\n", None);
    let git_head_before = fs::read_to_string(workspace.join(".git/refs/heads/main")).or_else(|_| fs::read_to_string(workspace.join(".git/refs/heads/master"))).unwrap();

    // Safepoint Checkpoint
    let store = ObjectStore::new(&safepoint_dir).unwrap();
    let baseline = TreeManifest::scan(workspace, &store).unwrap();

    // Agent destroys dirty work and creates new junk
    fs::remove_file(workspace.join("dirty_uncommitted.txt")).unwrap();
    create_file(&workspace.join("agent_overwrite.txt"), b"agent wrote this\n", None);

    // Safepoint Undo
    let current_tree = TreeManifest::scan(workspace, &store).unwrap();
    RestorePlan::build(&baseline, &current_tree).execute(workspace, &store).unwrap();

    // Invariant: Uncommitted work restored, agent junk removed
    assert_eq!(fs::read_to_string(workspace.join("dirty_uncommitted.txt")).unwrap(), "my uncommitted draft\n");
    assert!(!workspace.join("agent_overwrite.txt").exists());

    // Invariant: Git HEAD & commit history completely untouched
    let git_head_after = fs::read_to_string(workspace.join(".git/refs/heads/main")).or_else(|_| fs::read_to_string(workspace.join(".git/refs/heads/master"))).unwrap();
    assert_eq!(git_head_before, git_head_after, "Git HEAD must remain 100% untouched");
}

#[test]
fn test_multi_step_time_travel() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path();
    let safepoint_dir = workspace.join(".safepoint");

    let store = ObjectStore::new(&safepoint_dir).unwrap();
    let mut journal = Journal::load(&safepoint_dir).unwrap();

    // Step 1
    create_file(&workspace.join("file.txt"), b"version 1\n", None);
    let t1 = TreeManifest::scan(workspace, &store).unwrap();
    let h1 = t1.save(&store).unwrap();
    journal.add(h1.clone(), None, "v1".into(), t1.total_files, t1.total_bytes, None, None, None, None);

    // Step 2
    create_file(&workspace.join("file.txt"), b"version 2\n", None);
    create_file(&workspace.join("file2.txt"), b"hello v2\n", None);
    let t2 = TreeManifest::scan(workspace, &store).unwrap();
    let h2 = t2.save(&store).unwrap();
    journal.add(h2.clone(), Some(h1.clone()), "v2".into(), t2.total_files, t2.total_bytes, None, None, None, None);

    // Step 3
    create_file(&workspace.join("file.txt"), b"version 3\n", None);
    fs::remove_file(workspace.join("file2.txt")).unwrap();
    create_file(&workspace.join("file3.txt"), b"hello v3\n", None);
    let t3 = TreeManifest::scan(workspace, &store).unwrap();
    let h3 = t3.save(&store).unwrap();
    journal.add(h3.clone(), Some(h2.clone()), "v3".into(), t3.total_files, t3.total_bytes, None, None, None, None);

    // Rollback to v2
    let curr = TreeManifest::scan(workspace, &store).unwrap();
    RestorePlan::build(&t2, &curr).execute(workspace, &store).unwrap();
    assert_eq!(fs::read_to_string(workspace.join("file.txt")).unwrap(), "version 2\n");
    assert_eq!(fs::read_to_string(workspace.join("file2.txt")).unwrap(), "hello v2\n");
    assert!(!workspace.join("file3.txt").exists());

    // Rollback to v1
    let curr = TreeManifest::scan(workspace, &store).unwrap();
    RestorePlan::build(&t1, &curr).execute(workspace, &store).unwrap();
    assert_eq!(fs::read_to_string(workspace.join("file.txt")).unwrap(), "version 1\n");
    assert!(!workspace.join("file2.txt").exists());
    assert!(!workspace.join("file3.txt").exists());
}
