//! A recording whose program stopped without finishing it (#2412,
//! #2431): its `session.json` says about as much as the sound on disk,
//! and recovery reads the rest from the sound.

use super::*;

fn info(rate: u32) -> SessionInfo {
    SessionInfo {
        watch: "Depth".into(),
        position: "DU".into(),
        lift_deg: 52.0,
        bph: None,
        device: "test".into(),
        sample_rate: rate,
        bits: 16,
        software: "test".into(),
        calibre: String::new(),
        escapement: None,
    }
}

fn block(rate: u32, seconds: u64, at_s: u64) -> Block {
    Block {
        samples: vec![0.25; (rate as u64 * seconds) as usize],
        at: UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000 + at_s),
    }
}

fn parent(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("tg-rec-ondisk-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    p
}

/// The program stops as a kill stops it: nothing more is written (no
/// buffer flushed, no `Drop`), and the system lets go of its lock.
fn killed(mut r: Recorder) -> PathBuf {
    let dir = r.dir().to_path_buf();
    drop(r.lock.take());
    std::mem::forget(r);
    dir
}

fn record(dir: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(dir.join(take::RECORD)).unwrap()).unwrap()
}

/// Stopped before its first hour closed (Roland's repro on #2412: 120 s
/// heard, `session.json` said 0 s and no hours): `session.json` is at
/// most a clock line behind the sound, and recovery brings it up to the
/// sound on disk, which Open reads too.
#[test]
fn a_take_stopped_in_its_first_hour_says_about_its_length() {
    let parent = parent("first");
    let mut r = Recorder::start(&parent, info(8000)).unwrap();
    for s in 1..=120 {
        r.write(&block(8000, 1, s)).unwrap();
    }
    let dir = killed(r);
    let said = record(&dir);
    let said_s = said["duration_s"].as_f64().unwrap();
    assert!(said_s >= 120.0 - CLOCK_EVERY_S, "{said}");
    assert_eq!(said["segments"], serde_json::json!(["audio-001.wav"]));
    let on_disk = sound_on_disk_s(&dir).expect("the sound's length");
    assert!(on_disk >= said_s && on_disk <= 120.0, "{on_disk}");
    assert_eq!(mend_stopped(&dir).unwrap(), Some(on_disk));
    assert_eq!(record(&dir)["duration_s"].as_f64(), Some(on_disk));
    // What it was written with stays.
    assert_eq!(record(&dir)["watch"], "Depth");
    // Mended, it is left as it is.
    let before = fs::read(dir.join(take::RECORD)).unwrap();
    assert_eq!(mend_stopped(&dir).unwrap(), Some(on_disk));
    assert_eq!(fs::read(dir.join(take::RECORD)).unwrap(), before);
    let _ = fs::remove_dir_all(&parent);
}

/// Stopped seconds into its second hour, before `session.json` was
/// written again: recovery counts and lists the new hour too.
#[test]
fn a_take_stopped_just_after_a_new_hour_counts_that_hour() {
    let parent = parent("rollover");
    let mut r = Recorder::start(&parent, info(100)).unwrap();
    r.keep_flac = false;
    for s in 1..=SEGMENT_S / 10 {
        r.write(&block(100, 10, s * 10)).unwrap();
    }
    r.write(&block(100, 5, SEGMENT_S + 5)).unwrap();
    // As the app does to read it while it records.
    r.flush().unwrap();
    let dir = killed(r);
    let said = record(&dir);
    assert_eq!(
        said["duration_s"].as_f64(),
        Some(SEGMENT_S as f64),
        "{said}"
    );
    assert_eq!(said["segments"], serde_json::json!(["audio-001.wav"]));
    let length = SEGMENT_S as f64 + 5.0;
    assert_eq!(sound_on_disk_s(&dir), Some(length));
    assert_eq!(mend_stopped(&dir).unwrap(), Some(length));
    let mended = record(&dir);
    assert_eq!(mended["duration_s"].as_f64(), Some(length));
    assert_eq!(
        mended["segments"],
        serde_json::json!(["audio-001.wav", "audio-002.wav"])
    );
    let _ = fs::remove_dir_all(&parent);
}

/// A recording kept as its analysis alone has no sound to read a length
/// from, but its `session.json` still follows it as it goes.
#[test]
fn a_take_without_its_sound_still_says_its_length_as_it_goes() {
    let parent = parent("nosound");
    let mut r = Recorder::start(&parent, info(8000)).unwrap();
    r.without_sound().unwrap();
    for s in 1..=60 {
        r.write(&block(8000, 1, s)).unwrap();
    }
    let dir = killed(r);
    let said_s = record(&dir)["duration_s"].as_f64().unwrap();
    assert!(said_s >= 60.0 - CLOCK_EVERY_S, "{said_s}");
    assert_eq!(sound_on_disk_s(&dir), None);
    let before = fs::read(dir.join(take::RECORD)).unwrap();
    assert_eq!(mend_stopped(&dir).unwrap(), None);
    assert_eq!(fs::read(dir.join(take::RECORD)).unwrap(), before);
    let _ = fs::remove_dir_all(&parent);
}

/// No length when an hour `session.json` lists is not in the folder
/// (kept in its take alone, #2040), rather than a short one; and a folder
/// being recorded into is never written by recovery.
#[test]
fn recovery_gives_no_short_length_and_leaves_a_live_take_alone() {
    let parent = parent("missing");
    let mut r = Recorder::start(&parent, info(8000)).unwrap();
    for s in 1..=30 {
        r.write(&block(8000, 1, s)).unwrap();
    }
    let dir = r.dir().to_path_buf();
    let before = fs::read(dir.join(take::RECORD)).unwrap();
    // Recorded into still: its length, but nothing written.
    assert!(mend_stopped(&dir).unwrap().is_some());
    assert_eq!(fs::read(dir.join(take::RECORD)).unwrap(), before);
    let dir = killed(r);
    let mut about = record(&dir);
    about["segments"] = serde_json::json!(["audio-000.flac", "audio-001.wav"]);
    fs::write(dir.join(take::RECORD), about.to_string()).unwrap();
    let before = fs::read(dir.join(take::RECORD)).unwrap();
    assert_eq!(sound_on_disk_s(&dir), None);
    assert_eq!(mend_stopped(&dir).unwrap(), None);
    assert_eq!(fs::read(dir.join(take::RECORD)).unwrap(), before);
    let _ = fs::remove_dir_all(&parent);
}
