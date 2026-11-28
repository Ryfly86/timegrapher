//! A take's Watches reading ([`library::reading`]) follows what it is
//! read from, as `timegrapher session` reads it. One test, because it sets
//! `TIMEGRAPHER_CONFIG_DIR` (where readings are kept) for the whole
//! process.

use std::fs;
use std::path::Path;
use timegrapher_core::{audio, library, session::Manifest, synth, take};

/// 45 s of a watch running 10 s/d fast.
fn wav(p: &Path) {
    let cfg = synth::SynthConfig {
        duration_s: 45.0,
        rate_s_per_day: 10.0,
        ..Default::default()
    };
    audio::write_wav(p, &synth::generate(&cfg, |_| 270.0, |_| 0.0)).unwrap();
}

/// The rate of `dir`'s first recording, read afresh as `session` reads
/// it.
fn fresh(dir: &Path) -> f64 {
    let text = fs::read_to_string(dir.join(take::MANIFEST)).unwrap();
    let m: Manifest = toml::from_str(&text).unwrap();
    let settings = take::EntrySettings {
        stream: take::stream_settings(&m, None, take::Given::default()),
        card_clock: false,
        settle_s: None,
    };
    take::measure_entry(
        dir,
        &m,
        &m.recordings[0],
        &settings,
        |_| Ok(None),
        |_| Ok(None),
    )
    .unwrap()
    .measurement
    .rate_s_per_day
    .unwrap()
}

/// A clock log of the 45 s, its clock running `ppm` fast of the card.
fn clock_log(p: &Path, ppm: f64) {
    let mut text = String::from("unix_s,frames\n");
    for k in 0..=45 {
        let t = 1_700_000_000.0 + k as f64 * (1.0 + ppm * 1e-6);
        text += &format!("{t:.6},{}\n", k * 48_000);
    }
    fs::write(p, text).unwrap();
}

#[test]
fn a_reading_follows_what_it_is_read_from() {
    let root = std::env::temp_dir().join(format!("tg-reading-inputs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("config")).unwrap();
    std::env::set_var("TIMEGRAPHER_CONFIG_DIR", root.join("config"));

    // Audit R2-1: a clock log outside the take's folder, edited after the
    // reading was kept, is read again.
    let dir = root.join("take");
    fs::create_dir_all(&dir).unwrap();
    wav(&dir.join("audio.wav"));
    fs::write(
        dir.join(take::MANIFEST),
        "bph = 28800\nlift = 52\nsettle_s = 0\n\n[[recording]]\nfile = \"audio.wav\"\n\
         position = \"DU\"\nclock = \"../clock.csv\"\n",
    )
    .unwrap();
    let clock = root.join("clock.csv");
    clock_log(&clock, 0.0);
    let before = library::reading(&dir).unwrap().rate_s_per_day.unwrap();
    assert!(library::kept_reading(&dir).is_some(), "kept");
    clock_log(&clock, 50.0);
    assert!(
        library::kept_reading(&dir).is_none(),
        "a kept reading outlived its clock log"
    );
    let after = library::reading(&dir).unwrap().rate_s_per_day.unwrap();
    let want = fresh(&dir);
    assert!((after - want).abs() < 0.01, "{after} vs fresh {want}");
    assert!((after - before).abs() > 1.0, "{before} -> {after}");
    assert!(library::kept_reading(&dir).is_some(), "kept again");
    // Audit: engine identity. A reading kept by other code of the same
    // version (here the version alone, as older builds kept it) is read
    // again, not reused.
    let kept = root.join("config").join(library::READINGS_FILE);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&kept).unwrap()).unwrap();
    for k in json.as_object_mut().unwrap().values_mut() {
        k["outside"]["engine"] = env!("CARGO_PKG_VERSION").into();
    }
    fs::write(&kept, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(
        library::kept_reading(&dir).is_none(),
        "a reading kept by another engine of the same version was reused"
    );
    library::reading(&dir).unwrap();
    assert!(library::kept_reading(&dir).is_some(), "kept by this engine");
    // And one gone is said, not read from what was kept.
    fs::remove_file(&clock).unwrap();
    assert!(
        library::reading(&dir).is_err(),
        "a missing clock log read as kept"
    );

    // Audit R2-3: a last position with nothing settled to read is left
    // out, as `session` leaves it out, and the one before it is the
    // reading.
    let tail = root.join("unsettled-tail");
    fs::create_dir_all(&tail).unwrap();
    wav(&tail.join("audio.wav"));
    fs::write(
        tail.join(take::MANIFEST),
        "bph = 28800\nlift = 52\nsettle_s = 20\n\n\
         [[recording]]\nfile = \"audio.wav\"\nposition = \"DU\"\nstart_s = 0\nend_s = 35\n\n\
         [[recording]]\nfile = \"audio.wav\"\nposition = \"DD\"\nstart_s = 40\nend_s = 45\n",
    )
    .unwrap();
    let r = library::reading(&tail).unwrap();
    assert_eq!(r.position.as_deref(), Some("DU"), "{r:?}");
    assert!((r.rate_s_per_day.unwrap() - 10.0).abs() < 0.2, "{r:?}");
    // With no position to read, it still says why.
    fs::write(
        tail.join(take::MANIFEST),
        "bph = 28800\nlift = 52\nsettle_s = 40\n\n\
         [[recording]]\nfile = \"audio.wav\"\nposition = \"DU\"\nstart_s = 0\nend_s = 35\n",
    )
    .unwrap();
    let e = library::reading(&tail).unwrap_err();
    assert!(e.contains("settle_s"), "{e}");

    readings_revision_follows_settings_and_scans(&root);

    let _ = fs::remove_dir_all(&root);
}

/// #2410: a surface keeping the readings asks again when the library is
/// read again or a setting a reading reads beside its take changes: the
/// clock corrections, the user's calibres. Nothing changed, it stays.
/// Called from the one test, as it needs `TIMEGRAPHER_CONFIG_DIR`.
fn readings_revision_follows_settings_and_scans(root: &Path) {
    let mut lib = library::Library::create(&root.join("library")).unwrap();
    let was = lib.readings_revision();
    assert_eq!(lib.readings_revision(), was, "nothing changed");
    let clocks = timegrapher_core::clockstore::path().unwrap();
    let calibres = timegrapher_core::calibre_store::default_path().unwrap();
    assert!(!clocks.exists() && !calibres.exists());
    let changed = |lib: &library::Library, was: &mut library::ReadingsRevision, what: &str| {
        let now = lib.readings_revision();
        assert_ne!(&now, was, "{what}");
        *was = now;
    };
    let mut was = was;
    fs::write(&clocks, "{}").unwrap();
    changed(&lib, &mut was, "a clock correction stored");
    fs::write(&clocks, "{ }").unwrap();
    changed(&lib, &mut was, "a clock correction changed");
    // At the same size (a correction of 1.23 ppm made 1.32): told by its
    // last change.
    let later = |p: &Path| {
        let f = fs::OpenOptions::new().write(true).open(p).unwrap();
        let t = f.metadata().unwrap().modified().unwrap();
        f.set_modified(t + std::time::Duration::from_secs(10))
            .unwrap();
    };
    fs::write(&clocks, "{\"ppm\":1.23}").unwrap();
    changed(&lib, &mut was, "a clock correction of 1.23 ppm");
    fs::write(&clocks, "{\"ppm\":1.32}").unwrap();
    later(&clocks);
    changed(&lib, &mut was, "the same size, changed later");
    fs::write(&calibres, "{}").unwrap();
    changed(&lib, &mut was, "a calibre of the user's added");
    lib.scan();
    changed(&lib, &mut was, "the library read again");
    // Two libraries each opened afresh, each read once, are told apart:
    // the scans are counted across the process, not library by library.
    let one = library::Library::open(&root.join("library")).unwrap();
    let two = library::Library::open(&root.join("library")).unwrap();
    assert_ne!(
        one.readings_revision(),
        two.readings_revision(),
        "a library opened afresh"
    );
}
