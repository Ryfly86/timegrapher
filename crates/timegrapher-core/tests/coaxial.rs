//! A take read for a co-axial escapement (#2313, beta): its beat error is
//! the unlock's or none, never the drop's, and the unlock's is not lost to
//! the lift angle's amplitude gate; its amplitude carries the
//! `coaxial_estimate` caveat.
//!
//! The watch is synthetic: 21,600 bph at 300° with a 36° lift, so its
//! unlock-to-drop time is about 6.4 ms, as short as a co-axial's. Read at
//! the default 52° that is about 433°, over the 380° gate on both sides,
//! the case that left a co-axial take with no unlock beat error.

use timegrapher_core::analysis::analyze;
use timegrapher_core::escapement::Escapement;
use timegrapher_core::live::{LiveAnalyzer, LiveConfig};
use timegrapher_core::measure::Caveat;
use timegrapher_core::session::{self, BeatErrorSource, Measurement};
use timegrapher_core::stream::{analyze_samples, BeatLog, StreamConfig};
use timegrapher_core::synth::{generate, SynthConfig};

const BEAT_ERROR_MS: f64 = 0.6;
const LIFT_DEG: f64 = 36.0;
const AMPLITUDE_DEG: f64 = 300.0;

fn watch(unlock_heard: bool) -> (SynthConfig, timegrapher_core::audio::Audio) {
    let mut cfg = SynthConfig {
        duration_s: 40.0,
        bph: 21_600,
        lift_deg: LIFT_DEG,
        beat_error_ms: BEAT_ERROR_MS,
        ..Default::default()
    };
    if !unlock_heard {
        // Only the drop sounds: nothing before it to take for an unlock.
        cfg.sounds[0].gain = 0.0;
        cfg.sounds[1].gain = 0.0;
    }
    let audio = generate(&cfg, |_| AMPLITUDE_DEG, |_| 0.0);
    (cfg, audio)
}

fn stream_cfg(lift_deg: f64, escapement: Escapement) -> StreamConfig {
    let mut cfg = StreamConfig::default();
    cfg.analysis.bph = Some(21_600);
    cfg.analysis.amplitude.lift_deg = lift_deg;
    cfg.analysis.amplitude.escapement = escapement;
    cfg
}

fn log_at(lift_deg: f64, escapement: Escapement, unlock_heard: bool) -> BeatLog {
    let (synth, audio) = watch(unlock_heard);
    analyze_samples(
        &audio.samples,
        synth.sample_rate,
        &stream_cfg(lift_deg, escapement),
    )
}

fn read(log: &BeatLog) -> Measurement {
    session::measure(log, None, 0.0, log.duration_s)
}

fn has_drop_caveat(c: &[Caveat]) -> bool {
    c.iter()
        .any(|c| matches!(c, Caveat::BeatErrorFromDrop { .. }))
}

/// The case co-axial support starts from: read as a lever at 52°, both
/// sides are culled in every window, so the unlock's beat error is lost and
/// the drop's is shown.
#[test]
fn a_lever_read_at_the_default_lift_loses_the_unlock() {
    let m = read(&log_at(52.0, Escapement::SwissLever, true));
    assert!(m.watch_heard);
    assert_eq!(m.amplitude_deg, None, "433° is out of the gate");
    assert_eq!(m.beat_error_unlock_ms, None);
    assert_eq!(m.beat_error_source, Some(BeatErrorSource::Drop));
    assert!(has_drop_caveat(&m.caveats));
}

/// Read for a co-axial at the same 52°, the unlock's beat error is found
/// in the windows the gate culled, and the drop's is given nowhere.
#[test]
fn a_co_axial_read_keeps_the_unlock_whatever_the_lift() {
    let log = log_at(52.0, Escapement::CoAxial, true);
    assert_eq!(log.escapement, Escapement::CoAxial);
    let m = read(&log);
    assert!(m.watch_heard);
    assert_eq!(m.amplitude_deg, None, "433° is still out of the gate");
    let be = m.beat_error_unlock_ms.expect("the unlock's beat error");
    assert!((be.abs() - BEAT_ERROR_MS).abs() < 0.1, "{be}");
    assert_eq!(m.beat_error_ms, None, "no drop beat error");
    assert_eq!(m.beat_error_source, Some(BeatErrorSource::Unlock));
    assert_eq!(m.beat_error_signed(), Some(be));
    assert!(!has_drop_caveat(&m.caveats));
    // No amplitude, so no caveat on it.
    assert!(!m.caveats.contains(&Caveat::CoaxialEstimate));
    // Every window's choice is the unlock's or none.
    let sel = session::select_window_beat_errors(&log.amplitude_windows, log.escapement);
    assert_eq!(sel.beat_error_source, Some(BeatErrorSource::Unlock));
    assert!(log
        .amplitude_windows
        .iter()
        .all(|w| session::window_beat_error(w, log.escapement)
            .is_none_or(|(_, s)| s == BeatErrorSource::Unlock)));
}

/// At the watch's own lift the amplitude is given, with the caveat that it
/// is a co-axial estimate; the beat error is the one read at 52°, as the
/// lift enters only the degrees.
#[test]
fn a_co_axial_amplitude_carries_its_caveat() {
    let m = read(&log_at(LIFT_DEG, Escapement::CoAxial, true));
    let a = m.amplitude_deg.expect("an amplitude at 36°");
    assert!((a - AMPLITUDE_DEG).abs() < 10.0, "{a}");
    assert!(m.caveats.contains(&Caveat::CoaxialEstimate));
    let at52 = read(&log_at(52.0, Escapement::CoAxial, true));
    assert_eq!(m.beat_error_unlock_ms, at52.beat_error_unlock_ms);
    // A lever's amplitude carries no such caveat.
    let lever = read(&log_at(LIFT_DEG, Escapement::SwissLever, true));
    assert!(lever.amplitude_deg.is_some());
    assert!(!lever.caveats.contains(&Caveat::CoaxialEstimate));
}

/// Where the unlock is never heard, a co-axial gives no beat error at all:
/// the drop does not stand in.
#[test]
fn a_co_axial_with_no_unlock_heard_gives_no_beat_error() {
    let m = read(&log_at(LIFT_DEG, Escapement::CoAxial, false));
    assert!(m.watch_heard);
    assert_eq!(m.beat_error_unlock_ms, None);
    assert_eq!(m.beat_error_ms, None);
    assert_eq!(m.beat_error_source, None);
    assert_eq!(m.beat_error(), None);
    assert!(!has_drop_caveat(&m.caveats));
    let lever = read(&log_at(LIFT_DEG, Escapement::SwissLever, false));
    assert_eq!(lever.beat_error_source, Some(BeatErrorSource::Drop));
}

/// A log kept for one escapement and read for the other reads as one read
/// for that one afresh, both ways, at the same lift and at another.
#[test]
fn a_kept_log_reads_as_its_escapement_does() {
    use timegrapher_core::take::kept_at_lift;
    let lever = log_at(52.0, Escapement::SwissLever, true);
    let coaxial = log_at(52.0, Escapement::CoAxial, true);
    let unlocks = |l: &BeatLog| -> Vec<Option<f64>> {
        l.amplitude_windows
            .iter()
            .map(|w| w.beat_error_unlock_ms)
            .collect()
    };
    for lift in [52.0, 45.0] {
        let fresh_c = log_at(lift, Escapement::CoAxial, true);
        let fresh_l = log_at(lift, Escapement::SwissLever, true);
        let as_c = kept_at_lift(&lever, lift, Escapement::CoAxial);
        assert_eq!(as_c.escapement, Escapement::CoAxial);
        assert_eq!(
            unlocks(&as_c),
            unlocks(&fresh_c),
            "lever kept, co-axial at {lift}"
        );
        let as_l = kept_at_lift(&coaxial, lift, Escapement::SwissLever);
        assert_eq!(as_l.escapement, Escapement::SwissLever);
        assert_eq!(
            unlocks(&as_l),
            unlocks(&fresh_l),
            "co-axial kept, lever at {lift}"
        );
        assert_eq!(read(&as_c).beat_error_ms, None);
    }
}

/// `analyze` gives the same: the unlock's beat error, no drop's in the
/// document, the escapement named in it, and the caveat on an amplitude.
#[test]
fn analyze_reads_a_co_axial_as_a_take_does() {
    let (_, audio) = watch(true);
    let cfg = stream_cfg(LIFT_DEG, Escapement::CoAxial).analysis;
    let s = analyze(&audio, &cfg).summary;
    assert_eq!(s.beat_error_source, Some(BeatErrorSource::Unlock));
    assert!(s.caveats.contains(&Caveat::CoaxialEstimate));
    let doc = timegrapher_core::analyze_doc::body(&s, None).unwrap();
    assert!(
        doc["overall"]["beat_error_ms"].is_null(),
        "{}",
        doc["overall"]
    );
    assert_eq!(doc["escapement"], "co-axial");
    assert!(doc["caveats"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["code"] == "coaxial_estimate"));
    let settings = timegrapher_core::analyze_doc::settings(&cfg, None);
    assert_eq!(settings["escapement"], "co-axial");
    // A lever's document is as before: its drop given, no escapement named.
    let lever = analyze(
        &audio,
        &stream_cfg(LIFT_DEG, Escapement::SwissLever).analysis,
    )
    .summary;
    let doc = timegrapher_core::analyze_doc::body(&lever, None).unwrap();
    assert!(doc["overall"]["beat_error_ms"].is_number());
    assert!(doc.get("escapement").is_none());
    let settings = timegrapher_core::analyze_doc::settings(
        &stream_cfg(52.0, Escapement::SwissLever).analysis,
        None,
    );
    assert!(settings.get("escapement").is_none());
}

/// Live listening reads a co-axial as a saved take does: no drop beat
/// error, the unlock's found at 52°.
#[test]
fn live_reads_a_co_axial_as_a_take_does() {
    let (synth, audio) = watch(true);
    let cfg = LiveConfig {
        analysis: stream_cfg(52.0, Escapement::CoAxial).analysis,
        ..Default::default()
    };
    let mut l = LiveAnalyzer::new(synth.sample_rate, cfg);
    for c in audio.samples.chunks(4800) {
        l.push(c);
    }
    let r = l.reading(20.0);
    assert!(r.watch_heard);
    assert_eq!(r.beat_error_ms, None);
    let be = r.beat_error_unlock_ms.expect("the unlock's beat error");
    assert!((be.abs() - BEAT_ERROR_MS).abs() < 0.1, "{be}");
    assert_eq!(r.beat_error_source, Some(BeatErrorSource::Unlock));
    // A lift change keeps it.
    l.set_lift(LIFT_DEG);
    let r = l.reading(20.0);
    assert!(r.beat_error_unlock_ms.is_some());
    assert!(r.amplitude_deg.is_some());
    assert!(r.caveats.contains(&Caveat::CoaxialEstimate));
}

/// A take's escapement: its session file's, else its calibre's, else the
/// Swiss lever.
#[test]
fn a_takes_escapement_comes_from_its_file_or_its_calibre() {
    use timegrapher_core::calibre_store::UserCalibres;
    use timegrapher_core::take::{calibre_named, stream_settings, Given};
    let user = UserCalibres::default();
    let omega = calibre_named("Omega 3861", &user).expect("in the table");
    assert_eq!(omega.escapement, Some(Escapement::CoAxial));
    let eta = calibre_named("ETA 2824-2", &user).expect("in the table");
    assert_eq!(eta.escapement, None);
    let file = |s: &str| -> session::Manifest { toml::from_str(s).unwrap() };
    let esc = |m: &session::Manifest, c| {
        stream_settings(m, c, Given::default())
            .analysis
            .amplitude
            .escapement
    };
    assert_eq!(esc(&file(""), Some(&omega)), Escapement::CoAxial);
    assert_eq!(esc(&file(""), Some(&eta)), Escapement::SwissLever);
    assert_eq!(esc(&file(""), None), Escapement::SwissLever);
    assert_eq!(
        esc(&file("escapement = \"co-axial\""), Some(&eta)),
        Escapement::CoAxial
    );
    assert_eq!(
        esc(&file("escapement = \"swiss lever\""), Some(&omega)),
        Escapement::SwissLever
    );
}

/// The long-run report reads a co-axial as a take does: no slice and no
/// whole-run drop beat error, the unlock's where it is heard, none where it
/// isn't, and the caveat on its amplitude.
#[test]
fn a_long_run_report_reads_a_co_axial_as_a_take_does() {
    use timegrapher_core::longrun::analyse;
    use timegrapher_core::longterm::LongConfig;
    let lc = LongConfig::default();
    let r = analyse(&log_at(LIFT_DEG, Escapement::CoAxial, true), None, &lc);
    assert_eq!(r.escapement, Escapement::CoAxial);
    assert!(r.overall.is_some());
    assert_eq!(r.drop_beat_error_ms(), None);
    assert!(!r.slices.is_empty());
    assert!(r.slices.iter().all(|s| s.beat_error_ms.is_none()));
    assert!(r.slices.iter().any(|s| s.beat_error_unlock_ms.is_some()));
    let be = r.beat_error_unlock_ms.expect("the unlock's beat error");
    assert!((be.abs() - BEAT_ERROR_MS).abs() < 0.1, "{be}");
    assert!(r.amplitude_deg.is_some());
    assert!(r.caveats.contains(&Caveat::CoaxialEstimate));
    assert!(!has_drop_caveat(&r.caveats));
    // Not heard: no beat error from either.
    let r = analyse(&log_at(LIFT_DEG, Escapement::CoAxial, false), None, &lc);
    assert!(r.overall.is_some());
    assert_eq!(r.drop_beat_error_ms(), None);
    assert_eq!(r.beat_error_unlock_ms, None);
    assert!(r.slices.iter().all(|s| s.beat_error_ms.is_none()));
    // A lever's report is as before: the drop's given, no caveat.
    let r = analyse(&log_at(LIFT_DEG, Escapement::SwissLever, false), None, &lc);
    assert!(r.drop_beat_error_ms().is_some());
    assert!(r.slices.iter().any(|s| s.beat_error_ms.is_some()));
    assert!(!r.caveats.contains(&Caveat::CoaxialEstimate));
}

/// Each place a co-axial reading could fall back to the drop, on the watch
/// whose unlock is never heard, so the drop is all there is to fall back
/// to; each against a lever's, which does fall back, so the check sees it.
#[test]
fn no_reading_of_a_co_axial_falls_back_to_the_drop() {
    for (escapement, drop) in [(Escapement::CoAxial, false), (Escapement::SwissLever, true)] {
        let log = log_at(LIFT_DEG, escapement, false);
        // The windows' choice.
        let sel = session::select_window_beat_errors(&log.amplitude_windows, log.escapement);
        assert_eq!(sel.beat_error_source.is_some(), drop, "{escapement:?}");
        assert_eq!(
            log.amplitude_windows
                .iter()
                .any(|w| session::window_beat_error(w, log.escapement).is_some()),
            drop,
            "{escapement:?}"
        );
        // `analyze`.
        let (_, audio) = watch(false);
        let s = analyze(&audio, &stream_cfg(LIFT_DEG, escapement).analysis).summary;
        assert_eq!(s.beat_error_source.is_some(), drop, "{escapement:?}");
        assert_eq!(has_drop_caveat(&s.caveats), drop, "{escapement:?}");
        // Live, from a kept log, under a config that names no escapement:
        // the log's own is read for.
        let cfg = LiveConfig {
            analysis: stream_cfg(LIFT_DEG, Escapement::SwissLever).analysis,
            ..Default::default()
        };
        let r = LiveAnalyzer::from_log(log, cfg).reading(20.0);
        assert!(r.watch_heard);
        assert_eq!(r.beat_error_ms.is_some(), drop, "{escapement:?}");
    }
}

/// A co-axial read from the beat cache is read for a co-axial.
#[test]
fn a_co_axial_read_from_the_beat_cache_stays_one() {
    use std::time::{Duration, SystemTime};
    use timegrapher_core::beat_cache;
    let d = std::env::temp_dir().join(format!("tg-coaxial-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let wav = d.join("watch.wav");
    let (_, audio) = watch(true);
    timegrapher_core::audio::write_wav(&wav, &audio).unwrap();
    // Older than a file still being written.
    std::fs::File::options()
        .write(true)
        .open(&wav)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(600))
        .unwrap();
    let cfg = stream_cfg(52.0, Escapement::CoAxial);
    let log = log_at(52.0, Escapement::CoAxial, true);
    let cache = d.join("cache");
    beat_cache::write(&cache, &[wav.as_path()], &cfg, &log, 0.5).unwrap();
    let (got, _) = beat_cache::read(&cache, &[wav.as_path()], &cfg).expect("kept");
    assert_eq!(got.escapement, Escapement::CoAxial);
    assert_eq!(read(&got).beat_error_ms, None);
    let _ = std::fs::remove_dir_all(&d);
}

/// The two-state blocks of a co-axial, in a `session` reading and in the
/// Steadiness check, give no drop beat error: a watch whose amplitude
/// steps between 310° and 270° every 30 s over 7 minutes.
#[test]
fn a_co_axial_two_state_gives_no_drop_beat_error() {
    use timegrapher_core::longrun::analyse;
    use timegrapher_core::longterm::LongConfig;
    use timegrapher_core::steadiness::{self, SeriesKind};
    let synth = SynthConfig {
        duration_s: 420.0,
        bph: 21_600,
        lift_deg: LIFT_DEG,
        beat_error_ms: BEAT_ERROR_MS,
        ..Default::default()
    };
    let audio = generate(
        &synth,
        |t| {
            if (t / 30.0).floor() as i64 % 2 == 0 {
                310.0
            } else {
                270.0
            }
        },
        |_| 0.0,
    );
    for (escapement, drop) in [(Escapement::CoAxial, false), (Escapement::SwissLever, true)] {
        let log = analyze_samples(
            &audio.samples,
            synth.sample_rate,
            &stream_cfg(LIFT_DEG, escapement),
        );
        let m = read(&log);
        let two = &m.amplitude_states;
        assert!(
            two.beat_error_unlock_ms.is_some(),
            "{escapement:?}: {two:?}"
        );
        assert_eq!(two.beat_error_drop_ms.is_some(), drop, "{escapement:?}");
        let lc = LongConfig::default();
        let long = analyse(&log, None, &lc);
        let report = steadiness::check(&log, None, &long, &lc, &Default::default());
        let two = report
            .series
            .iter()
            .find(|s| s.series == SeriesKind::Amplitude)
            .and_then(|s| s.two_state.as_ref())
            .expect("the amplitude's two-state find");
        assert!(
            two.beat_error_unlock_ms.is_some(),
            "{escapement:?}: {two:?}"
        );
        assert_eq!(two.beat_error_drop_ms.is_some(), drop, "{escapement:?}");
    }
}

/// Every co-axial reading says where its beat error comes from: the
/// unlock's alone, or none and why (Hugh's option (a) on #2317), in a
/// saved take's reading, `analyze`, live and the long-run report alike. A
/// lever's carries no such caveat.
#[test]
fn every_co_axial_reading_says_where_its_beat_error_comes_from() {
    use timegrapher_core::longrun::analyse;
    use timegrapher_core::longterm::LongConfig;
    use timegrapher_core::measure::DropWhy;
    let coaxial = |c: &[Caveat]| -> Vec<Caveat> {
        c.iter()
            .filter(|c| matches!(c, Caveat::CoaxialBeatError { .. }))
            .copied()
            .collect()
    };
    for heard in [true, false] {
        let said = vec![Caveat::CoaxialBeatError {
            why: (!heard).then_some(DropWhy::UnlockNotFound),
        }];
        for (escapement, want) in [
            (Escapement::CoAxial, said),
            (Escapement::SwissLever, vec![]),
        ] {
            let log = log_at(LIFT_DEG, escapement, heard);
            let m = read(&log);
            assert_eq!(coaxial(&m.caveats), want, "session {escapement:?} {heard}");
            let (_, audio) = watch(heard);
            let s = analyze(&audio, &stream_cfg(LIFT_DEG, escapement).analysis).summary;
            assert_eq!(coaxial(&s.caveats), want, "analyze {escapement:?} {heard}");
            let r = analyse(&log, None, &LongConfig::default());
            assert_eq!(coaxial(&r.caveats), want, "long {escapement:?} {heard}");
            let cfg = LiveConfig {
                analysis: stream_cfg(LIFT_DEG, escapement).analysis,
                ..Default::default()
            };
            let r = LiveAnalyzer::from_log(log, cfg).reading(20.0);
            assert_eq!(coaxial(&r.caveats), want, "live {escapement:?} {heard}");
        }
    }
    // The JSON code and its reason, as agents read them.
    let c = Caveat::CoaxialBeatError {
        why: Some(DropWhy::UnlockNotFound),
    };
    let v = serde_json::to_value(c).unwrap();
    assert_eq!(v["code"], "coaxial_beat_error");
    assert_eq!(v["why"], "unlock_not_found");
    let v = serde_json::to_value(Caveat::CoaxialBeatError { why: None }).unwrap();
    assert!(v.get("why").is_none(), "{v}");
}

/// A reading with no figures says nothing of where its beat error comes
/// from: no regular beat, so no "unlock not found", in `analyze` and a
/// saved take's reading, and none in a long-run report with no beats
/// (Vernon on #2319).
#[test]
fn a_co_axial_with_no_watch_heard_says_nothing_of_its_beat_error() {
    use timegrapher_core::longrun::analyse;
    use timegrapher_core::longterm::LongConfig;
    let mut cfg = SynthConfig {
        duration_s: 20.0,
        ..Default::default()
    };
    for s in cfg.sounds.iter_mut() {
        s.gain = 0.0;
    }
    let audio = generate(&cfg, |_| AMPLITUDE_DEG, |_| 0.0);
    let said = |c: &[Caveat]| {
        c.iter()
            .any(|c| matches!(c, Caveat::CoaxialBeatError { .. }))
    };
    let stream = stream_cfg(LIFT_DEG, Escapement::CoAxial);
    let s = analyze(&audio, &stream.analysis).summary;
    assert!(!s.watch_heard);
    assert!(!said(&s.caveats), "analyze");
    let log = analyze_samples(&audio.samples, cfg.sample_rate, &stream);
    assert_eq!(log.escapement, Escapement::CoAxial);
    let m = read(&log);
    assert!(!m.watch_heard);
    assert!(!said(&m.caveats), "session: {:?}", m.caveats);
    // The long-run report has no judge of a watch heard: it fits what
    // beats it finds, noise too. With none found it has no fit, and says
    // nothing (as `BeatErrorAsSize` does not).
    let mut quiet = log_at(LIFT_DEG, Escapement::CoAxial, true);
    assert!(said(&analyse(&quiet, None, &LongConfig::default()).caveats));
    quiet.beats.clear();
    let r = analyse(&quiet, None, &LongConfig::default());
    assert!(r.overall.is_none());
    assert!(!said(&r.caveats), "long: {:?}", r.caveats);
}

/// A take whose unlock reads one way in 6 s of every 10 and the other in
/// the rest (#1439's case, as `analyze` meets it on a lever): read for a
/// co-axial it gives no beat error, and says the unlock's disagree, in a
/// saved take's reading, `analyze` and live alike (Vernon on #2319). A
/// lever's falls back to the drop and says the same of it.
#[test]
fn a_co_axial_with_an_inconsistent_unlock_says_so() {
    use timegrapher_core::audio::Audio;
    use timegrapher_core::measure::DropWhy;
    let take = |even_unlock_lead_s: f64| {
        let cfg = SynthConfig {
            duration_s: 90.0,
            beat_error_ms: BEAT_ERROR_MS,
            even_unlock_lead_s,
            ..Default::default()
        };
        generate(&cfg, |_| 270.0, |_| 0.0)
    };
    let (far, near) = (take(0.001), take(0.0));
    let fs = far.sample_rate as f64;
    // Spliced between beats, 6 s of every 10 from the far take.
    let samples: Vec<f32> = (0..far.samples.len())
        .map(|i| match (i as f64 / fs - 0.1125).rem_euclid(10.0) < 6.0 {
            true => far.samples[i],
            false => near.samples[i],
        })
        .collect();
    let audio = Audio {
        sample_rate: far.sample_rate,
        samples,
    };
    let said = |c: &[Caveat]| -> Vec<Caveat> {
        c.iter()
            .filter(|c| {
                matches!(
                    c,
                    Caveat::CoaxialBeatError { .. } | Caveat::BeatErrorFromDrop { .. }
                )
            })
            .copied()
            .collect()
    };
    for (escapement, want) in [
        (
            Escapement::CoAxial,
            Caveat::CoaxialBeatError {
                why: Some(DropWhy::UnlockInconsistent),
            },
        ),
        (
            Escapement::SwissLever,
            Caveat::BeatErrorFromDrop {
                why: DropWhy::UnlockInconsistent,
            },
        ),
    ] {
        let mut stream = StreamConfig::default();
        stream.analysis.bph = Some(28_800);
        stream.analysis.amplitude.escapement = escapement;
        let s = analyze(&audio, &stream.analysis).summary;
        assert_eq!(s.beat_error_unlock_ms, None, "analyze {escapement:?}");
        assert_eq!(said(&s.caveats), vec![want], "analyze {escapement:?}");
        let log = analyze_samples(&audio.samples, audio.sample_rate, &stream);
        let m = read(&log);
        assert_eq!(m.beat_error_unlock_ms, None, "session {escapement:?}");
        assert_eq!(said(&m.caveats), vec![want], "session {escapement:?}");
        let cfg = LiveConfig {
            analysis: stream.analysis.clone(),
            ..Default::default()
        };
        let mut l = LiveAnalyzer::new(audio.sample_rate, cfg);
        for c in audio.samples.chunks(4800) {
            l.push(c);
        }
        let r = l.reading(80.0);
        assert_eq!(r.beat_error_unlock_ms, None, "live {escapement:?}");
        assert_eq!(said(&r.caveats), vec![want], "live {escapement:?}");
        if escapement == Escapement::CoAxial {
            assert_eq!(s.beat_error_source, None);
            assert_eq!(m.beat_error_source, None);
            assert_eq!(r.beat_error_source, None);
        }
    }
}

/// Live listening set to another escapement, as when a co-axial calibre
/// is picked while listening, reads every reading as one read for it from
/// the start, and back again.
#[test]
fn live_set_to_another_escapement_reads_as_one_read_for_it() {
    let (synth, audio) = watch(true);
    let at = |escapement| {
        let cfg = LiveConfig {
            analysis: stream_cfg(52.0, escapement).analysis,
            ..Default::default()
        };
        let mut l = LiveAnalyzer::new(synth.sample_rate, cfg);
        for c in audio.samples.chunks(4800) {
            l.push(c);
        }
        l
    };
    let figures = |r: &timegrapher_core::live::LiveReading| {
        (
            r.beat_error_ms,
            r.beat_error_unlock_ms,
            r.beat_error_source,
            r.caveats.clone(),
        )
    };
    let lever = figures(&at(Escapement::SwissLever).reading(20.0));
    let coaxial = figures(&at(Escapement::CoAxial).reading(20.0));
    // The two differ at 52°: the lever loses the unlock, the co-axial
    // keeps it and gives no drop.
    assert_eq!(lever.2, Some(BeatErrorSource::Drop));
    assert_eq!(coaxial.2, Some(BeatErrorSource::Unlock));
    let mut l = at(Escapement::SwissLever);
    l.set_escapement(Escapement::CoAxial);
    assert_eq!(
        l.config().analysis.amplitude.escapement,
        Escapement::CoAxial
    );
    assert_eq!(figures(&l.reading(20.0)), coaxial);
    l.set_escapement(Escapement::SwissLever);
    assert_eq!(figures(&l.reading(20.0)), lever);
}

/// A reading with no beats read, as when a co-axial calibre of another
/// beat rate is picked while listening, says nothing of its beat error:
/// it waits for beats (Walter on #2319).
#[test]
fn a_co_axial_reading_with_no_beats_read_says_nothing_of_its_beat_error() {
    let (synth, audio) = watch(true);
    let cfg = LiveConfig {
        analysis: stream_cfg(LIFT_DEG, Escapement::CoAxial).analysis,
        ..Default::default()
    };
    let mut l = LiveAnalyzer::new(synth.sample_rate, cfg);
    let (first, rest) = audio.samples.split_at(audio.samples.len() / 2);
    for c in first.chunks(4800) {
        l.push(c);
    }
    let said = |r: &timegrapher_core::live::LiveReading| {
        r.caveats
            .iter()
            .any(|c| matches!(c, Caveat::CoaxialBeatError { .. }))
    };
    assert!(said(&l.reading(10.0)), "heard at its own rate");
    l.set_bph(Some(25_200));
    let mut waited = false;
    for c in rest.chunks(4800) {
        l.push(c);
        let r = l.reading(10.0);
        if r.beats_used == 0 {
            waited = true;
            assert!(!said(&r), "no beats read at {:.1} s", r.time_s);
        }
    }
    assert!(waited, "the wrong rate read no beats at some point");
}
