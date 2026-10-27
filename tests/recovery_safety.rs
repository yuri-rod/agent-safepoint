use std::fs;
use std::io;

use safepoint::core::cas::ObjectStore;
use safepoint::core::restore::RestorePlan;
use safepoint::core::tree::{FileEntry, TreeManifest};
use tempfile::TempDir;

#[test]
fn unavailable_checkpoint_content_does_not_change_workspace() {
    for corrupt in [false, true] {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path();
        let store = ObjectStore::new(&workspace.join(".safepoint")).unwrap();
        fs::write(workspace.join("draft.txt"), b"saved draft").unwrap();
        let baseline = TreeManifest::scan(workspace, &store).unwrap();
        fs::write(workspace.join("draft.txt"), b"current draft").unwrap();
        fs::write(workspace.join("new.txt"), b"new work").unwrap();
        let current = TreeManifest::scan(workspace, &store).unwrap();
        let blob = store.object_path(&baseline.entries["draft.txt"].hash);
        if corrupt {
            fs::write(blob, b"damaged checkpoint").unwrap();
        } else {
            fs::remove_file(blob).unwrap();
        }

        let result = RestorePlan::build(&baseline, &current).execute(workspace, &store);
        assert!(
            result.is_err(),
            "invalid checkpoint content must be rejected"
        );
        assert_eq!(fs::read(workspace.join("new.txt")).unwrap(), b"new work");
        assert_eq!(
            fs::read(workspace.join("draft.txt")).unwrap(),
            b"current draft"
        );
    }
}

#[test]
fn invalid_object_ids_are_errors_not_paths_or_panics() {
    let temp = TempDir::new().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    for hash in ["", "../outside", "/tmp/outside", "a界b", &"z".repeat(64)] {
        assert_eq!(
            store.read_blob(hash).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

#[test]
fn corrupt_blob_cannot_replace_an_existing_file() {
    let temp = TempDir::new().unwrap();
    let store = ObjectStore::new(&temp.path().join(".safepoint")).unwrap();
    let hash = store.write_blob(b"saved draft").unwrap();
    fs::write(store.object_path(&hash), b"corrupt").unwrap();
    let dest = temp.path().join("draft.txt");
    fs::write(&dest, b"current draft").unwrap();

    assert_eq!(
        store.copy_to_file(&hash, &dest).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(fs::read(dest).unwrap(), b"current draft");
}

fn manifest_entry(path: &str, hash: &str) -> TreeManifest {
    let mut tree = TreeManifest::new();
    tree.entries.insert(
        path.into(),
        FileEntry {
            path: path.into(),
            hash: hash.into(),
            size: 6,
            mode: 0o644,
            is_symlink: false,
            symlink_target: None,
        },
    );
    tree.compute_tree_hash();
    tree
}

#[test]
fn unsafe_checkpoint_paths_are_rejected_before_any_mutation() {
    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let store = ObjectStore::new(&workspace.join(".safepoint")).unwrap();
    let hash = store.write_blob(b"stored").unwrap();
    let outside = temp.path().join("outside");
    fs::write(&outside, b"outside").unwrap();
    fs::create_dir(workspace.join(".git")).unwrap();
    fs::write(workspace.join(".git/config"), b"git metadata").unwrap();
    fs::write(workspace.join("new.txt"), b"new work").unwrap();
    let current = TreeManifest::scan(&workspace, &store).unwrap();
    for path in [
        "../outside",
        outside.to_str().unwrap(),
        ".git/config",
        ".GIT/config",
        ".safepoint/journal.json",
        "nested/../../outside",
        "",
    ] {
        let plan = RestorePlan::build(&manifest_entry(path, &hash), &current);
        assert!(
            plan.execute(&workspace, &store).is_err(),
            "accepted unsafe path {path:?}"
        );
        assert_eq!(fs::read(workspace.join("new.txt")).unwrap(), b"new work");
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        assert_eq!(
            fs::read(workspace.join(".git/config")).unwrap(),
            b"git metadata"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_parent_cannot_redirect_restore_outside_workspace() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    let outside = temp.path().join("outside");
    fs::create_dir_all(workspace.join("nested")).unwrap();
    fs::create_dir(&outside).unwrap();
    let store = ObjectStore::new(&workspace.join(".safepoint")).unwrap();
    fs::write(workspace.join("nested/draft.txt"), b"saved").unwrap();
    let baseline = TreeManifest::scan(&workspace, &store).unwrap();
    fs::remove_dir_all(workspace.join("nested")).unwrap();
    fs::write(workspace.join("new.txt"), b"new work").unwrap();
    let current = TreeManifest::scan(&workspace, &store).unwrap();
    // The filesystem can change after a plan's current manifest was scanned.
    symlink(&outside, workspace.join("nested")).unwrap();
    fs::write(outside.join("draft.txt"), b"outside work").unwrap();

    assert!(RestorePlan::build(&baseline, &current)
        .execute(&workspace, &store)
        .is_err());
    assert_eq!(
        fs::read(outside.join("draft.txt")).unwrap(),
        b"outside work"
    );
    assert_eq!(fs::read(workspace.join("new.txt")).unwrap(), b"new work");
}

#[cfg(unix)]
#[test]
fn permission_changes_never_follow_symlinks() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let outside = temp.path().join("outside");
    fs::write(&outside, b"outside work").unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&outside, workspace.join("link")).unwrap();
    let store = ObjectStore::new(&workspace.join(".safepoint")).unwrap();
    let current = TreeManifest::scan(&workspace, &store).unwrap();
    let mut target = current.clone();
    target.entries.get_mut("link").unwrap().mode = 0o777;
    let _ = RestorePlan::build(&target, &current).execute(&workspace, &store);
    assert_eq!(
        fs::metadata(outside).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn restoring_regular_file_replaces_symlink_without_touching_its_target() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let outside = temp.path().join("outside");
    fs::write(&outside, b"outside work").unwrap();
    let store = ObjectStore::new(&workspace.join(".safepoint")).unwrap();
    fs::write(workspace.join("draft.txt"), b"saved draft").unwrap();
    let baseline = TreeManifest::scan(&workspace, &store).unwrap();
    fs::remove_file(workspace.join("draft.txt")).unwrap();
    symlink(&outside, workspace.join("draft.txt")).unwrap();
    let current = TreeManifest::scan(&workspace, &store).unwrap();

    RestorePlan::build(&baseline, &current)
        .execute(&workspace, &store)
        .unwrap();
    assert_eq!(
        fs::read(workspace.join("draft.txt")).unwrap(),
        b"saved draft"
    );
    assert!(!fs::symlink_metadata(workspace.join("draft.txt"))
        .unwrap()
        .is_symlink());
    assert_eq!(fs::read(outside).unwrap(), b"outside work");
}
