//! A closed hour its saved copy holds whole is kept there alone (#2040).

use super::tests::{append_second, save_fixture};
use super::*;

/// The sound files left in `dir`, by name.
fn sound_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| is_sound(n))
        .collect();
    v.sort();
    v
}

/// A recording kept as the app keeps its sessions.
fn fixture(name: &str) -> (Recorder, PathBuf) {
    let (mut r, base) = save_fixture(name);
    r.keep_saved_hours_in_copy();
    (r, base)
}

/// Unless asked, a recording keeps every hour, saved or not.
#[test]
fn a_recording_keeps_its_hours_unless_asked() {
    let (mut r, base) = save_fixture("released-off");
    append_second(&mut r, 2);
    r.pause().unwrap();
    let take = stream(&mut r, &base.join("unsorted"));
    r.tidy().unwrap();
    assert!(!r.releasing());
    assert_eq!(sound_in(&take), ["audio-001.flac"]);
    let dir = r.finish(None).unwrap();
    assert_eq!(sound_in(&dir), ["audio-001.flac"]);
    fs::remove_dir_all(&base).unwrap();
}

/// Tidy until no hour waits to go, as the app's frames do while paused.
fn tidy_all(r: &mut Recorder) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while r.releasing() && std::time::Instant::now() < until {
        r.tidy().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!r.releasing(), "the hour was let go in time");
}

/// A streamed save into `parent`, noted.
fn stream(r: &mut Recorder, parent: &Path) -> PathBuf {
    let copy = r
        .begin_stream_save(parent, "", None, &[])
        .unwrap()
        .write()
        .unwrap();
    r.note_saved(&copy);
    copy.dir
}

/// A kept take stopped: its saved copy holds the closed hour, so the
/// recording folder lets its own go, and reads it from the copy from then
/// on. Carried on and saved again, the copy keeps the hour as it was and
/// gains the next; saved into another folder, the hour is copied from the
/// take.
#[test]
fn a_closed_hour_its_copy_holds_whole_is_kept_there_alone() {
    let (mut r, base) = fixture("released");
    append_second(&mut r, 2);
    let take = stream(&mut r, &base.join("unsorted"));
    // Stop: the hour closes, and the Stop's save holds it as FLAC.
    r.pause().unwrap();
    assert_eq!(stream(&mut r, &base.join("unsorted")), take);
    tidy_all(&mut r);
    let flac = take.join("audio-001.flac");
    assert_eq!(sound_in(r.dir()), Vec::<String>::new(), "no second copy");
    assert_eq!(sound_in(&take), ["audio-001.flac"]);
    assert_eq!(crate::audio::load(&flac).unwrap().samples.len(), 2 * 8000);
    assert_eq!(r.segment_paths(), std::slice::from_ref(&flac));
    assert!(r.hours_reachable());
    let first = fs::read(&flac).unwrap();
    // Carried on: the next hour is the recording's own until it closes.
    r.resume("test");
    append_second(&mut r, 30);
    assert_eq!(sound_in(r.dir()), ["audio-002.wav"]);
    r.pause().unwrap();
    assert_eq!(stream(&mut r, &base.join("unsorted")), take);
    tidy_all(&mut r);
    assert_eq!(sound_in(r.dir()), Vec::<String>::new());
    assert_eq!(sound_in(&take), ["audio-001.flac", "audio-002.flac"]);
    assert_eq!(fs::read(&flac).unwrap(), first, "the first hour left alone");
    // Saved into another folder: every hour, from the take.
    let other = r.save_copy(&base.join("other"), "", None).unwrap();
    assert_eq!(sound_in(&other), ["audio-001.flac", "audio-002.flac"]);
    assert_eq!(fs::read(other.join("audio-001.flac")).unwrap(), first);
    let session: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(other.join("session.json")).unwrap()).unwrap();
    assert_eq!(
        session["segments"],
        serde_json::json!(["audio-001.flac", "audio-002.flac"])
    );
    fs::remove_dir_all(&base).unwrap();
}

/// While listening, an hour closed and saved goes from the recording
/// folder as the next sound is written; the hour still being written is
/// never let go.
#[test]
fn an_hour_closed_while_listening_goes_once_its_copy_holds_it() {
    let (mut r, base) = fixture("released-live");
    append_second(&mut r, 2);
    r.pause().unwrap();
    r.resume("test");
    append_second(&mut r, 30);
    let take = stream(&mut r, &base.join("unsorted"));
    // Kept as FLAC here by now, then let go as more is heard.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut second = 31;
    while r.releasing() && std::time::Instant::now() < until {
        append_second(&mut r, second);
        second += 1;
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!r.releasing());
    assert_eq!(sound_in(r.dir()), ["audio-002.wav"]);
    assert_eq!(r.segment_paths()[0], take.join("audio-001.flac"));
    assert_eq!(r.segment_paths()[1], r.dir().join("audio-002.wav"));
    fs::remove_dir_all(&base).unwrap();
}

/// A copy moved (filed under a watch) is where the hours it holds alone
/// are read from; one deleted leaves them out of reach, which the app
/// takes as a recording that can't be carried on.
#[test]
fn hours_kept_in_the_copy_alone_follow_it_and_are_missed_when_it_goes() {
    let (mut r, base) = fixture("released-moved");
    append_second(&mut r, 2);
    r.pause().unwrap();
    let from = stream(&mut r, &base.join("unsorted"));
    tidy_all(&mut r);
    let to = base.join("watch").join("moved take");
    fs::create_dir_all(to.parent().unwrap()).unwrap();
    fs::rename(&from, &to).unwrap();
    r.copy_moved(&from, &to);
    assert_eq!(r.segment_paths(), [to.join("audio-001.flac")]);
    assert!(r.hours_reachable());
    fs::remove_dir_all(&to).unwrap();
    assert!(!r.hours_reachable());
    fs::remove_dir_all(&base).unwrap();
}

/// An hour whose copy isn't the same, byte for byte, by the time it would
/// go stays in the recording folder.
#[test]
fn an_hour_its_copy_does_not_hold_whole_stays() {
    let (mut r, base) = fixture("released-changed");
    append_second(&mut r, 2);
    r.pause().unwrap();
    // Kept as FLAC here first, so the save checks the FLAC's bytes.
    r.tidy().unwrap();
    while !r.converting.is_empty() {
        std::thread::sleep(std::time::Duration::from_millis(20));
        r.tidy().unwrap();
    }
    let copy = r
        .begin_stream_save(&base.join("unsorted"), "", None, &[])
        .unwrap()
        .write()
        .unwrap();
    assert_eq!(copy.held_whole.len(), 1);
    // Changed behind the save's back before it is noted: its length no
    // longer the one checked.
    let flac = copy.dir.join("audio-001.flac");
    let mut f = fs::OpenOptions::new().append(true).open(&flac).unwrap();
    f.write_all(&[0; 7]).unwrap();
    drop(f);
    r.note_saved(&copy);
    tidy_all(&mut r);
    assert_eq!(sound_in(r.dir()), ["audio-001.flac"]);
    assert_eq!(r.segment_paths(), [r.dir().join("audio-001.flac")]);
    fs::remove_dir_all(&base).unwrap();
}

/// The byte check: the same bytes at the same length only.
#[test]
fn the_copy_is_checked_byte_for_byte() {
    let dir = std::env::temp_dir().join(format!("tg-released-bytes-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let here = dir.join("here");
    let bytes: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7 % 251) as u8).collect();
    fs::write(&here, &bytes).unwrap();
    let held = File::open(&here).unwrap();
    let len = bytes.len() as u64;
    let there = dir.join("there");
    fs::write(&there, &bytes).unwrap();
    assert!(released::same_bytes(&held, len, &there).unwrap());
    let mut late = bytes.clone();
    *late.last_mut().unwrap() ^= 1;
    fs::write(&there, &late).unwrap();
    assert!(!released::same_bytes(&held, len, &there).unwrap());
    fs::write(&there, &bytes[..bytes.len() - 1]).unwrap();
    assert!(!released::same_bytes(&held, len, &there).unwrap());
    fs::remove_dir_all(&dir).unwrap();
}

/// Trevor on #2046: no hour counts as held whole in the copy until the
/// copy's folder is synced, so a folder sync that fails lets nothing go;
/// the next save that syncs does.
#[test]
fn nothing_goes_before_the_copys_folder_is_synced() {
    let (mut r, base) = fixture("released-unsynced");
    append_second(&mut r, 2);
    r.pause().unwrap();
    let parent = base.join("unsorted");
    let refused = |step: SaveStep, _: &Path| match step {
        SaveStep::Sync => Err(io::Error::other("injected folder sync failure")),
        _ => Ok(()),
    };
    let copy = r
        .begin_save(&parent, "", None, &[])
        .unwrap()
        .write_using(refused)
        .unwrap();
    assert!(copy.held_whole.is_empty());
    r.note_saved(&copy);
    tidy_all(&mut r);
    assert_eq!(sound_in(r.dir()).len(), 1, "kept here");
    let copy = r
        .begin_save(&parent, "", None, &[])
        .unwrap()
        .write()
        .unwrap();
    assert_eq!(copy.held_whole.len(), 1);
    r.note_saved(&copy);
    tidy_all(&mut r);
    assert_eq!(sound_in(r.dir()), Vec::<String>::new());
    fs::remove_dir_all(&base).unwrap();
}
