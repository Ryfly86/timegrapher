//! A take recorded on Auto is read on Auto (Peter, 26 Nov 2026, on
//! #2357): its `session.json` recorder info says `bph: null`, so the
//! `bph` its `session.toml` holds is the rate Auto guessed and the
//! calibre's rate was not the one chosen. Every reader of the take's
//! entries reads it on Auto (`set_rate::read_as_recorded`, in
//! `take::open_entry`). A rate given for the reading stands, and a take
//! whose recorder info names a rate reads at it.

use std::path::{Path, PathBuf};
use timegrapher_core::analysis::AnalysisConfig;
use timegrapher_core::measure::off_standard;
use timegrapher_core::session::{Manifest, Measurement, RecordingEntry};
use timegrapher_core::set_rate::{read_as_recorded, recorded_on_auto, BphFrom};
use timegrapher_core::synth::{generate, SynthConfig};
use timegrapher_core::take::{self, EntrySettings, Given};

/// A take folder named `name` holding 40 s of a balance at 17,570 bph,
/// over 1000 s/d from every standard rate, and a `session.json` whose
/// recorder info gives `info_bph` (JSON), or none.
fn take_dir(name: &str, info_bph: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-as-recorded-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = SynthConfig {
        bph: 17_570,
        duration_s: 40.0,
        ..Default::default()
    };
    let audio = generate(&cfg, |_| 280.0, |_| 0.0);
    timegrapher_core::audio::write_wav(&dir.join("take.wav"), &audio).unwrap();
    if let Some(b) = info_bph {
        let json = format!(r#"{{"watch": "", "position": "DU", "lift_deg": 52.0, "bph": {b}}}"#);
        std::fs::write(dir.join(take::RECORD), json).unwrap();
    }
    dir
}

/// The take at `dir` read as `timegrapher session` reads it: its session
/// file `m`, its calibre, and the rate `given`.
fn read(dir: &Path, m: &Manifest, given: Option<u32>) -> Measurement {
    let cal = m
        .calibre
        .as_deref()
        .and_then(|c| take::calibre_named(c, &Default::default()));
    let given = Given {
        bph: given,
        lift_deg: None,
    };
    let s = EntrySettings {
        stream: take::stream_settings(m, cal.as_ref(), given),
        card_clock: true,
        settle_s: Some(0.0),
    };
    let e = RecordingEntry {
        file: "take.wav".into(),
        ..Default::default()
    };
    let (usb, card) = (|_: &[_]| Ok(None), |_: &Path| Ok(None));
    let read = take::measure_entry(dir, m, &e, &s, usb, card).unwrap();
    read.measurement
}

fn off(m: &Measurement) -> bool {
    m.rate_s_per_day.is_none() && off_standard(&m.caveats).is_some()
}

fn manifest(bph: Option<u32>, calibre: Option<&str>) -> Manifest {
    Manifest {
        bph,
        calibre: calibre.map(str::to_string),
        ..Default::default()
    }
}

#[test]
fn a_take_recorded_on_auto_reads_on_auto() {
    let dir = take_dir("auto", Some("null"));
    assert!(recorded_on_auto(&dir));
    // Auto's guess kept as the session file's rate.
    assert!(off(&read(&dir, &manifest(Some(17_280), None), None)));
    // A calibre named whose table gives 18,000, and no rate kept.
    let unitas = manifest(None, Some("Unitas 6497-2"));
    assert!(off(&read(&dir, &unitas, None)));
    // A rate given for the reading stands.
    let given = read(&dir, &manifest(Some(17_280), None), Some(18_000));
    assert!(
        given.rate_s_per_day.is_some_and(|r| r < -1_000.0),
        "{given:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_take_recorded_at_a_set_rate_reads_at_it() {
    for (name, info) in [("set", Some("17280")), ("none", None)] {
        let dir = take_dir(name, info);
        assert!(!recorded_on_auto(&dir), "{name}");
        let m = read(&dir, &manifest(Some(17_280), None), None);
        assert!(
            m.rate_s_per_day.is_some_and(|r| r > 1_000.0),
            "{name}: {m:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn only_the_session_files_and_the_calibres_rate_give_way() {
    let dir = take_dir("from", Some("null"));
    for (from, auto) in [
        (BphFrom::Session, true),
        (BphFrom::Calibre, true),
        (BphFrom::Typed, false),
        (BphFrom::Standard, false),
    ] {
        let mut a = AnalysisConfig {
            bph: Some(18_000),
            bph_from: Some(from),
            ..Default::default()
        };
        read_as_recorded(&dir, &mut a);
        let want = if auto {
            (None, None)
        } else {
            (Some(18_000), Some(from))
        };
        assert_eq!((a.bph, a.bph_from), want, "{from:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the recorder itself writes: on Auto (`bph` none), the take was
/// recorded on Auto; at a rate set by hand, it was not.
#[test]
fn a_recorders_own_file_says_whether_it_was_on_auto() {
    use timegrapher_core::recorder::{Recorder, SessionInfo};
    let parent = std::env::temp_dir().join(format!("tg-as-recorded-{}-rec", std::process::id()));
    let _ = std::fs::remove_dir_all(&parent);
    std::fs::create_dir_all(&parent).unwrap();
    for (bph, auto) in [(None, true), (Some(18_000), false)] {
        let info = SessionInfo {
            watch: String::new(),
            position: "DU".into(),
            lift_deg: 52.0,
            bph,
            device: "test".into(),
            sample_rate: 8_000,
            bits: 16,
            software: "test".into(),
        };
        let r = Recorder::start(&parent, info).unwrap();
        assert_eq!(recorded_on_auto(r.dir()), auto, "{bph:?}");
    }
    let _ = std::fs::remove_dir_all(&parent);
}

/// Only a `bph` written null says Auto: a file with no `bph` key, or not
/// the recorder's (no `lift_deg`), never does.
#[test]
fn a_missing_bph_is_never_auto() {
    let dir = take_dir("missing", None);
    let write = |s: &str| std::fs::write(dir.join(take::RECORD), s).unwrap();
    write(r#"{"watch": "", "position": "DU", "lift_deg": 52.0}"#);
    assert!(!recorded_on_auto(&dir));
    write(r#"{"bph": null}"#);
    assert!(!recorded_on_auto(&dir));
    write("not json");
    assert!(!recorded_on_auto(&dir));
    write(r#"{"watch": "", "position": "DU", "lift_deg": 52.0, "bph": null}"#);
    assert!(recorded_on_auto(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}
