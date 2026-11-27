//! `clock set` and `clock forget` run at the same moment, each its own
//! process, keep every change that reports success (#2408).

use std::process::{Child, Command, Stdio};

#[test]
fn clock_sets_and_forgets_at_once_are_all_kept() {
    let dir = std::env::temp_dir().join(format!("tg-clock-at-once-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let start = |args: &[&str]| -> Child {
        Command::new(env!("CARGO_BIN_EXE_timegrapher"))
            .args(args)
            .env("TIMEGRAPHER_CONFIG_DIR", &dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let wait = |c: Child| {
        let out = c.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    for k in 0..6 {
        wait(start(&["clock", "set", &format!("Old {k}"), "1"]));
    }
    let mut running: Vec<Child> = (0..12)
        .map(|k| start(&["clock", "set", &format!("Device {k}"), &format!("{k}.5")]))
        .collect();
    running.extend((0..6).map(|k| start(&["clock", "forget", &format!("Old {k}")])));
    for c in running {
        wait(c);
    }
    let list = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(["clock", "list", "--json"])
        .env("TIMEGRAPHER_CONFIG_DIR", &dir)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(list.status.success());
    let doc: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    let mut names: Vec<String> = doc["devices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["device"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    let mut want: Vec<String> = (0..12).map(|k| format!("Device {k}")).collect();
    want.sort();
    assert_eq!(names, want);
}

/// Forgetting a name with no correction stored is refused and leaves the
/// store byte for byte as it was (#2408).
#[test]
fn forgetting_a_name_not_stored_is_refused_and_changes_nothing() {
    let dir = std::env::temp_dir().join(format!("tg-clock-forget-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_timegrapher"))
            .args(args)
            .env("TIMEGRAPHER_CONFIG_DIR", &dir)
            .output()
            .unwrap()
    };
    assert!(run(&["clock", "set", "Kept", "2.5"]).status.success());
    let before = std::fs::read(dir.join("clocks.json")).unwrap();
    let out = run(&["clock", "forget", "Never Stored"]);
    let after = std::fs::read(dir.join("clocks.json")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.status.success());
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(
        error.contains("no clock correction stored for 'Never Stored'"),
        "{error}"
    );
    assert_eq!(after, before);
}
