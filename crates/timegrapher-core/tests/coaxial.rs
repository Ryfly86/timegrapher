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
