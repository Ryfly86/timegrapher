//! A table piped into a reader that stops early, `timegrapher calibres |
//! head -1` or `less` quit at once, ends quietly with status 141 (128 +
//! SIGPIPE, as `cat` or `grep` give), not a "Broken pipe" panic and exit
//! 101 (#2098).
#![cfg(unix)]

use std::process::{Command, Output, Stdio};

/// Run `args` with stdout a pipe whose reader has already gone, so the
/// first line written fails, as it does once `head` has read its line.
fn with_reader_gone(args: &[&str]) -> Output {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .expect("run timegrapher")
}

#[test]
fn a_table_whose_reader_has_gone_ends_quietly() {
    let out = with_reader_gone(&["calibres"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stderr.contains("Broken pipe"), "{stderr}");
    assert_eq!(out.status.code(), Some(141), "{stderr}");
}

/// `devices` too, whatever this machine's sound inputs: its first line,
/// a device or "No sound inputs found.", meets the closed pipe. Skipped
/// where the sound system can't be read at all, as nothing is printed.
#[test]
fn devices_whose_reader_has_gone_ends_quietly() {
    let probe = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .arg("devices")
        .stdin(Stdio::null())
        .output()
        .expect("run timegrapher");
    if !probe.status.success() {
        return;
    }
    let out = with_reader_gone(&["devices"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(out.status.code(), Some(141), "{stderr}");
}

/// Output to a file is as before: the whole table, with status 0.
#[test]
fn a_table_written_whole_is_unchanged() {
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .arg("calibres")
        .stdin(Stdio::null())
        .output()
        .expect("run timegrapher");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("Calibre"), "{text}");
    assert!(text.lines().count() > 1, "{text}");
}
