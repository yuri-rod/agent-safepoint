use safepoint::core::journal::Journal;
use std::process::Command;

#[test]
fn timeline_truncates_prompts_at_character_boundaries() {
    for (prompt, displayed) in [
        ("界".repeat(40), "界".repeat(40)),
        ("é".repeat(81), format!("{}...", "é".repeat(77))),
        ("x".repeat(81), format!("{}...", "x".repeat(77))),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut journal = Journal::default();
        journal.add(
            "a".repeat(64),
            None,
            "synthetic checkpoint".into(),
            0,
            0,
            None,
            None,
            None,
            None,
            Some("synthetic agent".into()),
            Some(prompt),
            vec![],
        );
        journal.save(&dir.path().join(".safepoint")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_safepoint"))
            .arg("-C")
            .arg(dir.path())
            .arg("timeline")
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains(&format!("prompt: \"{displayed}\"")));
    }
}
