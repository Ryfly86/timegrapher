//! Save Again into a folder whose copy was deleted and another take put
//! in its place writes a new copy beside it and leaves that take alone
//! (#2437).

use super::super::tests::{append_second, save_fixture};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

fn files(dir: &Path) -> BTreeMap<OsString, Vec<u8>> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.file_type().unwrap().is_file())
        .map(|e| (e.file_name(), fs::read(e.path()).unwrap()))
        .collect()
}

/// The probe from the issue: the copy saved to `A/<minute>` is deleted
/// outside the app and another take of that name copied in. Save Again
/// into `A` saves a new copy (` 2`), leaves that take's files as they
/// were, and goes on saving into the new copy.
#[test]
fn save_again_never_writes_into_a_take_put_in_place_of_its_copy() {
    let (mut r, base) = save_fixture("replaced-copy");
    let takes = base.join("A").join("takes");
    let name = "2026-11-26 05.30";
    let first = r.claim_first_copy(&takes, name).unwrap();
    assert_eq!(r.save_copy(&takes, "", None).unwrap(), first);
    fs::remove_dir_all(&first).unwrap();
    fs::create_dir(&first).unwrap();
    fs::write(first.join("audio-001.wav"), b"other take").unwrap();
    let other_session = br#"{"started_utc": "2026-11-26T05:30:40Z"}"#;
    fs::write(first.join("session.json"), other_session).unwrap();
    let before = files(&first);
    append_second(&mut r, 2);
    let again = r.save_copy(&takes, "", None).unwrap();
    assert_eq!(again, takes.join(format!("{name} 2")));
    assert_eq!(files(&first), before);
    let sound = fs::read(again.join("audio-001.wav")).unwrap();
    assert!(sound.len() > 10);
    // Saved again, it goes on into the new copy and still leaves the
    // other take alone.
    append_second(&mut r, 3);
    assert_eq!(r.save_copy(&takes, "", None).unwrap(), again);
    assert!(fs::read(again.join("audio-001.wav")).unwrap().len() > sound.len());
    assert_eq!(files(&first), before);
    drop(r);
    let _ = fs::remove_dir_all(base);
}

/// A take put in place with no `session.json`, or one that can't be
/// read, is not this take's copy either.
#[test]
fn a_folder_with_no_readable_session_is_not_this_takes_copy() {
    for (case, session) in [("none", None), ("bad", Some(&b"not json"[..]))] {
        let (mut r, base) = save_fixture(&format!("replaced-copy-{case}"));
        let takes = base.join("takes");
        let first = r.save_copy(&takes, "", None).unwrap();
        fs::remove_dir_all(&first).unwrap();
        fs::create_dir(&first).unwrap();
        fs::write(first.join("audio-001.wav"), b"other take").unwrap();
        if let Some(text) = session {
            fs::write(first.join("session.json"), text).unwrap();
        }
        let before = files(&first);
        let again = r.save_copy(&takes, "", None).unwrap();
        assert_ne!(again, first);
        assert_eq!(files(&first), before);
        drop(r);
        let _ = fs::remove_dir_all(base);
    }
}

/// Its own copy, a copy deleted with nothing put back, and a copy emptied
/// are still saved into in place, as before.
#[test]
fn save_again_still_writes_into_its_own_copy() {
    let (mut r, base) = save_fixture("replaced-copy-own");
    let takes = base.join("takes");
    let first = r.save_copy(&takes, "", None).unwrap();
    append_second(&mut r, 2);
    assert_eq!(r.save_copy(&takes, "", None).unwrap(), first);
    fs::remove_dir_all(&first).unwrap();
    assert_eq!(r.save_copy(&takes, "", None).unwrap(), first);
    for e in fs::read_dir(&first).unwrap() {
        fs::remove_file(e.unwrap().path()).unwrap();
    }
    assert_eq!(r.save_copy(&takes, "", None).unwrap(), first);
    assert!(first.join("audio-001.wav").is_file());
    assert_eq!(fs::read_dir(&takes).unwrap().count(), 1);
    drop(r);
    let _ = fs::remove_dir_all(base);
}
