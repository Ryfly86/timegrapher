//! A co-axial calibre in the app (#2313, beta): what it is read for, live
//! and opened, and what its card says.

use super::*;
use timegrapher_core::escapement::Escapement::{CoAxial, SwissLever};

/// A co-axial calibre picked is listened to as one: a session started
/// with it, and one listening when it is picked, read for a co-axial; a
/// lever's picked after it reads for a lever again.
#[test]
fn a_co_axial_calibre_is_listened_to_as_one() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    assert_eq!(app.escapement(), SwissLever);
    app.set_calibre("Omega 3861".into());
    assert_eq!(app.escapement(), CoAxial);
    assert_eq!(app.live_config().analysis.amplitude.escapement, CoAxial);
    app.reset_session(48000, "syn".into());
    let read_for = |app: &TimegrapherApp| {
        app.bench
            .recording
            .live
            .as_ref()
            .unwrap()
            .config()
            .analysis
            .amplitude
            .escapement
    };
    assert_eq!(read_for(&app), CoAxial);
    app.set_calibre("ETA 2824-2".into());
    app.keep_escapement();
    assert_eq!(read_for(&app), SwissLever);
    app.set_calibre("Omega 8500".into());
    app.keep_escapement();
    assert_eq!(read_for(&app), CoAxial);
}

/// A take's session file's `escapement` outranks its calibre's, as core
/// reads it, and is saved back with the take; a microphone setup put back
/// has none.
#[test]
fn a_session_files_escapement_outranks_the_calibres() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let mic = app.watch_setup();
    let m = Manifest {
        calibre: Some("ETA 2824-2".into()),
        escapement: Some(CoAxial),
        ..Default::default()
    };
    app.apply_manifest_placed(&m, &[], None, None);
    assert_eq!(app.escapement(), CoAxial);
    assert_eq!(app.manifest_header().escapement, Some(CoAxial));
    // The watch setup carries the key: put back, each setup reads as it
    // did (Vernon on #2319).
    let keyed = app.watch_setup();
    app.set_watch_setup(mic.clone());
    assert_eq!(app.bench.take.escapement_set, None);
    assert_eq!(app.escapement(), SwissLever);
    app.set_watch_setup(keyed);
    assert_eq!(app.bench.take.escapement_set, Some(CoAxial));
    assert_eq!(app.escapement(), CoAxial);
    let m = Manifest {
        calibre: Some("Omega 3861".into()),
        escapement: Some(SwissLever),
        ..Default::default()
    };
    app.apply_manifest_placed(&m, &[], None, None);
    assert_eq!(app.escapement(), SwissLever);
    // A file with no key: the calibre's.
    let m = Manifest {
        calibre: Some("Omega 3861".into()),
        ..Default::default()
    };
    app.apply_manifest_placed(&m, &[], None, None);
    assert_eq!(app.escapement(), CoAxial);
    assert_eq!(app.manifest_header().escapement, None);
    app.set_watch_setup(mic);
    assert_eq!(app.bench.take.escapement_set, None);
    assert_eq!(app.escapement(), SwissLever);
}

/// A co-axial's card sets its lift angle as a lever's does, where a pin
/// lever's or a detent's says it is not supported in its place; its
/// calibre says how it is read.
#[test]
fn a_co_axial_calibres_lift_is_set_as_a_levers() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.set_calibre("Omega 3861".into());
    assert!(app.escapement_unsupported().is_none());
    assert!(app.lift_settable(), "the 3861 has no lift in the table");
    let x = app.cal.find("Omega 3861").unwrap().escapement().unwrap();
    assert_eq!(x.tag(), timegrapher_core::calibres::Escapement::COAXIAL_TAG);
    assert_eq!(x.sentence(), timegrapher_core::calibres::COAXIAL_SENTENCE);
}

/// The amplitude's co-axial caveat keeps the owner's full words wherever
/// they fit, and tries core's shorter ones before the count (Hugh on
/// #2319).
#[test]
fn the_estimate_chip_tries_shorter_words_before_the_count() {
    use timegrapher_core::measure::{Caveat, COAXIAL_ESTIMATE_SHORTER};
    let c = Caveat::CoaxialEstimate;
    let one = [(c.short(52.0, 0).unwrap(), c.long(None).unwrap())];
    assert_eq!(one[0].0, "co-axial escapement estimate (beta)");
    let words = |room: usize| readouts::limits_chip_words(&one, |w| w.len() <= room);
    assert_eq!(words(99), "Co-axial Escapement Estimate (beta)");
    assert_eq!(words(30), "Co-axial Estimate (beta)");
    assert_eq!(words(10), "1 limit");
    assert_eq!(
        COAXIAL_ESTIMATE_SHORTER.len(),
        "Co-axial Estimate (beta)".len()
    );
    let two = [one[0].clone(), one[0].clone()];
    let words = |room: usize| readouts::limits_chip_words(&two, |w| w.len() <= room);
    assert_eq!(words(30), "Co-axial Estimate (beta) +1");
}

/// The Beat Error chip says where a co-axial's beat error comes from in
/// core's short words, for each reason there is none (Vernon on #2319).
#[test]
fn the_beat_error_chip_says_why_a_co_axial_has_none() {
    use timegrapher_core::measure::{Caveat, DropWhy};
    for (why, short, chip) in [
        (None, "co-axial (beta)", "Co-axial (beta)"),
        (
            Some(DropWhy::UnlockNotFound),
            "co-axial: unlock not found",
            "Co-axial: Unlock Not Found",
        ),
        (
            Some(DropWhy::UnlockInconsistent),
            "co-axial: the unlock's signs disagree beyond their spread",
            "Co-axial: the Unlock's Signs Disagree Beyond Their Spread",
        ),
    ] {
        let c = Caveat::CoaxialBeatError { why };
        let one = [(c.short(52.0, 0).unwrap(), c.long(None).unwrap())];
        assert_eq!(one[0].0, short, "{why:?}");
        assert_eq!(readouts::limits_chip_words(&one, |_| true), chip, "{why:?}");
    }
}

/// The app's whole-take log of a co-axial session is read for a co-axial:
/// no drop beat error, and the reading says where its beat error comes
/// from (Norm on #2319).
#[test]
fn a_co_axial_sessions_whole_take_log_is_read_as_one() {
    use timegrapher_core::measure::Caveat;
    use timegrapher_core::synth::{generate, SynthConfig};
    let audio = generate(
        &SynthConfig {
            bph: 28_800,
            duration_s: 30.0,
            beat_error_ms: 0.6,
            ..Default::default()
        },
        |_| 280.0,
        |_| 0.0,
    );
    for escapement in [CoAxial, SwissLever] {
        let mut cfg = timegrapher_core::live::LiveConfig::default();
        cfg.analysis.bph = Some(28_800);
        cfg.analysis.amplitude.escapement = escapement;
        let mut live = LiveAnalyzer::new(48_000, cfg);
        live.push(&audio.samples);
        let log = crate::steady::log_of(&live).expect("a log");
        assert_eq!(log.escapement, escapement);
        let m = timegrapher_core::session::measure(&log, None, 0.0, log.duration_s);
        let said = m
            .caveats
            .iter()
            .any(|c| matches!(c, Caveat::CoaxialBeatError { .. }));
        assert_eq!(
            m.beat_error_ms.is_none(),
            escapement == CoAxial,
            "{escapement:?}"
        );
        assert_eq!(said, escapement == CoAxial, "{escapement:?}");
    }
}

/// A stopped co-axial card still finding the beat at the watch's last
/// beat reads as the saved read of its own span: read for a co-axial, so
/// no drop beat error and its co-axial caveat (Norm on #2319; the lever's
/// case is `a_stopped_card_still_finding_reads_its_own_span`).
#[test]
fn a_stopped_co_axial_card_reads_its_own_span_as_a_co_axial() {
    use timegrapher_core::measure::Caveat;
    let base = std::env::temp_dir().join(format!("tg-coaxial-span-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&base);
    let mut app =
        super::readouts_tests::stopped_short_return_as(&base, (30.0, 20.0, 11.0), |app| {
            app.bench.take.escapement_set = Some(CoAxial)
        });
    assert_eq!(app.escapement(), CoAxial);
    let mut read = 0;
    for average in timegrapher_core::live::AVERAGES {
        app.bench.take.average_s = average;
        let _ = app.whole_take();
        let s = app.stopped_take().expect("stopped");
        let at = timegrapher_core::beats::last_watch_beat_s(s.live.beats()).expect("a last beat");
        if !s.live.reading_at(at, average).finding_beat {
            continue;
        }
        let own = app.own_span_read(at - average, at).expect("its span read");
        let r = app.card_reading().expect("a reading");
        assert!(!r.finding_beat, "{average} s: {r:?}");
        assert_eq!(own.beat_error_ms, None, "{average} s: {own:?}");
        assert_eq!(r.beat_error_ms, None, "{average} s: {r:?}");
        if r.watch_heard {
            read += 1;
            assert!(
                r.caveats
                    .iter()
                    .any(|c| matches!(c, Caveat::CoaxialBeatError { .. })),
                "{average} s: {:?}",
                r.caveats
            );
        }
    }
    assert!(read > 0, "a card read as its own span's saved read");
    let _ = std::fs::remove_dir_all(base);
}

/// Sound never saved keeps its calibre and escapement in its
/// session.json as it is heard, a calibre picked mid-take included, and
/// opens with them: a co-axial's read for a co-axial (#2396).
#[test]
fn unsaved_sound_opens_with_the_calibre_it_was_heard_with() {
    use timegrapher_core::recorder::{recorded_setup, Recorder};
    let scratch = std::env::temp_dir().join(format!("tg-coax-unsaved-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let r = Recorder::start(&scratch, app.info("test", 8000, 16)).unwrap();
    let dir = r.dir().to_path_buf();
    assert_eq!(recorded_setup(&dir).unwrap().calibre, None);
    app.session.recorder = Some(r);
    // One that changes neither lift nor beat rate is written too.
    app.set_calibre("Bench Calibre".into());
    let heard = recorded_setup(&dir).unwrap().calibre;
    assert_eq!(heard.as_deref(), Some("Bench Calibre"));
    app.set_calibre("ETA 2824-2".into());
    app.set_calibre("Omega 3861".into());
    let heard = recorded_setup(&dir).unwrap();
    assert_eq!(heard.calibre.as_deref(), Some("Omega 3861"));
    assert_eq!(heard.escapement, None);
    let mut opened = TimegrapherApp::with_devices(Vec::new(), None);
    opened.put_recorded(heard, &dir);
    assert_eq!(opened.bench.take.calibre, "Omega 3861");
    assert_eq!(opened.escapement(), CoAxial);
    assert_eq!(opened.bench.take.lift_deg, app.bench.take.lift_deg);
    assert_eq!(opened.lift_known(), app.lift_known());
    // The take's own escapement, set by the file it came from, goes too.
    let m = Manifest {
        calibre: Some("ETA 2824-2".into()),
        escapement: Some(CoAxial),
        ..Default::default()
    };
    app.apply_manifest_placed(&m, &[], None, None);
    app.note_settings();
    let heard = recorded_setup(&dir).unwrap();
    assert_eq!(heard.escapement, Some(CoAxial));
    let mut opened = TimegrapherApp::with_devices(Vec::new(), None);
    opened.put_recorded(heard, &dir);
    assert_eq!(opened.escapement(), CoAxial);
    drop(app);
    let _ = std::fs::remove_dir_all(&scratch);
}
