//! A live reading set to another escapement (#2313) is read for it in
//! every copy it keeps: the template's reread, the newest read as a file
//! and the reading held from before a rate change (Vernon on #2319).

use super::*;
use crate::escapement::Escapement;
use crate::session::BeatErrorSource;
use crate::synth::{generate, SynthConfig};

/// 21,600 bph at 300° with a 36° lift, read at 52°: a lever loses the
/// unlock's beat error and gives the drop's; a co-axial keeps the
/// unlock's.
fn watch(duration_s: f64) -> Vec<f32> {
    let cfg = SynthConfig {
        duration_s,
        bph: 21_600,
        lift_deg: 36.0,
        beat_error_ms: 0.6,
        ..Default::default()
    };
    generate(&cfg, |_| 300.0, |_| 0.0).samples
}

fn config(escapement: Escapement) -> LiveConfig {
    let mut cfg = LiveConfig::default();
    cfg.analysis.bph = Some(21_600);
    cfg.analysis.amplitude.escapement = escapement;
    cfg
}

/// A reading's beat error figures and caveats.
fn figures(
    r: &LiveReading,
) -> (
    Option<f64>,
    Option<f64>,
    Option<BeatErrorSource>,
    Vec<Caveat>,
) {
    (
        r.beat_error_ms,
        r.beat_error_unlock_ms,
        r.beat_error_source,
        r.caveats.clone(),
    )
}

/// Set while the first chunk is being read again with the template built
/// from it, the reread is read for the new escapement, so the readings
/// once it takes over are the new escapement's.
#[test]
fn the_templates_reread_is_read_for_the_escapement_set() {
    let sound = watch(80.0);
    let fed = |l: &mut LiveAnalyzer, set: bool| {
        let mut caught = false;
        for c in sound.chunks(4800) {
            l.push(c);
            if set && !caught && l.rereading() {
                l.set_escapement(Escapement::CoAxial);
                caught = true;
            }
        }
        caught
    };
    let mut want = LiveAnalyzer::new(48_000, config(Escapement::CoAxial));
    fed(&mut want, false);
    let mut l = LiveAnalyzer::new(48_000, config(Escapement::SwissLever));
    assert!(fed(&mut l, true), "set while the reread runs");
    assert!(l.template_switch_s().is_some(), "the reread took over");
    let r = l.reading(20.0);
    assert_eq!(r.beat_error_source, Some(BeatErrorSource::Unlock));
    assert_eq!(figures(&r), figures(&want.reading(20.0)));
}

/// With the figures read as a file in the background, the newest read is
/// read for the new escapement at once: the reading shown before the next
/// read lands is the new escapement's.
#[test]
fn the_newest_read_as_a_file_is_read_for_the_escapement_set() {
    let sound = watch(20.0);
    let landed = |escapement| {
        let mut l = LiveAnalyzer::new(48_000, config(escapement)).with_figures_as_file();
        l.push_without_waiting(&sound);
        while !l.read_finished() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        l.push_without_waiting(&[]);
        assert_eq!(l.read_state(), (false, true));
        l
    };
    let want = figures(&landed(Escapement::CoAxial).reading(20.0));
    assert_eq!(want.2, Some(BeatErrorSource::Unlock));
    let mut l = landed(Escapement::SwissLever);
    assert_eq!(
        l.reading(20.0).beat_error_source,
        Some(BeatErrorSource::Drop)
    );
    l.set_escapement(Escapement::CoAxial);
    assert_eq!(l.read_state(), (false, true), "no new read started");
    assert_eq!(figures(&l.reading(20.0)), want);
}

/// Set while a reading is held from before a rate change, the held
/// reading is read for it too: set back to its rate, the reading that
/// comes back is the new escapement's.
#[test]
fn a_held_reading_is_read_for_the_escapement_set() {
    let sound = watch(20.0);
    let mut want = LiveAnalyzer::new(48_000, config(Escapement::CoAxial));
    want.push(&sound);
    let want = figures(&want.reading(20.0));
    assert_eq!(want.2, Some(BeatErrorSource::Unlock));
    let mut l = LiveAnalyzer::new(48_000, config(Escapement::SwissLever));
    l.push(&sound);
    assert!(!l.set_bph(Some(25_200)));
    assert!(l.holds_reading());
    l.set_escapement(Escapement::CoAxial);
    assert!(l.set_bph(Some(21_600)), "the held reading back");
    assert_eq!(
        l.config().analysis.amplitude.escapement,
        Escapement::CoAxial
    );
    assert_eq!(figures(&l.reading(20.0)), want);
}

/// A co-axial reading whose sides are not yet told apart again, after a
/// jump, says nothing of its beat error: it waits for the sides, as its
/// figures do (Vernon on #2319).
#[test]
fn a_co_axial_reading_with_its_sides_unknown_says_nothing_of_its_beat_error() {
    let said = |r: &LiveReading| {
        r.caveats
            .iter()
            .any(|c| matches!(c, Caveat::CoaxialBeatError { .. }))
    };
    let mut l = LiveAnalyzer::new(48_000, config(Escapement::CoAxial));
    l.push(&watch(30.0));
    let now = l.duration_s();
    let r = l.reading_at(now, 20.0);
    assert!(said(&r), "{:?}", r.caveats);
    l.jump_at(now - 5.0, false);
    assert!(!l.sides_known_at(now));
    let r = l.reading_at(now, 20.0);
    assert!(
        r.watch_heard && r.listening.is_none() && r.beats_used > 0,
        "{r:?}"
    );
    assert_eq!(r.beat_error_unlock_ms, None);
    assert!(!said(&r), "{:?}", r.caveats);
}
