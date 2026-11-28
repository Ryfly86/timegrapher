//! Every reader beside the long-run report takes its verdicts (Trevor 86
//! on #2487): the maps, spectra, coherences, kurtograms and a wheel's beat
//! error line read nothing where the take's whole reading finds no
//! regular beat, and `series` gives no wind-down where the rate is
//! withheld. Each reads on a hand-built take of 2 h and more, so each
//! gives its figures without the verdict.
use super::*;
use crate::amplitude::AmplitudeWindow;
use std::f64::consts::TAU;

const TAKE_S: f64 = 7400.0;

/// A 18,000 bph take of [`TAKE_S`]: its beats' timing swings 0.02 ms
/// every 600 s (about 18 s/d), its amplitude and its beat error swing
/// every 600 s and every 40 s, its Tick and Tock apart.
fn take() -> BeatLog {
    let n = (TAKE_S / 0.2) as i64;
    let beats = (0..n)
        .map(|i| {
            let t = i as f64 * 0.2;
            Beat {
                index: i,
                time: t + 2e-5 * (TAU * t / 600.0).sin(),
                quality: 1.0,
            }
        })
        .collect();
    let windows = (0..(TAKE_S / 2.0) as usize)
        .map(|k| {
            let start_s = k as f64 * 2.0;
            let (slow, wheel) = ((TAU * start_s / 600.0).sin(), (TAU * start_s / 40.0).sin());
            AmplitudeWindow {
                start_s,
                end_s: start_s + 2.0,
                even_deg: Some(270.0 + 10.0 * slow + 3.0 * wheel),
                odd_deg: Some(266.0 + 8.0 * slow - 2.0 * wheel),
                beat_error_ms: None,
                beat_error_unlock_ms: Some(0.3 + 0.05 * slow + 0.02 * wheel),
                even_unlock_ms: None,
                odd_unlock_ms: None,
                even_drop_ms: None,
                odd_drop_ms: None,
                extra: None,
                sound_1: [f32::NAN; 2],
            }
        })
        .collect();
    BeatLog {
        lock_template_kept: false,
        sample_rate: 48_000,
        duration_s: TAKE_S,
        bph: 18_000,
        lift_deg: 52.0,
        escapement: Default::default(),
        beats,
        amplitude_windows: windows,
        clipped_beats: vec![],
        clipped_samples: 0,
        jumps: vec![],
        sided: vec![],
        by_count: vec![],
        snippets: vec![],
    }
}

fn config() -> LongConfig {
    LongConfig {
        wheels: vec![crate::periodicity::Wheel {
            name: "test wheel".into(),
            period_s: 40.0,
        }],
        ..Default::default()
    }
}

/// What the readers beside the report give: whether any of the gappy
/// spectra, the beat error's maps, the coherences, the kurtograms and the
/// wheel's beat error line reads a finite value.
fn reads(log: &BeatLog, r: &LongReport, lc: &LongConfig) -> [bool; 6] {
    let reading_s = crate::steadiness::Config::default().rate_reading_s;
    let finite = |v: &[f64]| v.iter().any(|x| x.is_finite());
    let gappy = crate::gappy::for_take(log, None, r, lc, reading_s);
    let maps = crate::timefreq::for_take(log, None, r, lc);
    let coherences = crate::timefreq::coherences_for_take(log, None, r, reading_s);
    let kurtograms = crate::timefreq::kurtograms_for_take(log, None, r, lc);
    let lines = crate::wheel_lines::lines_for_take(log, None, r, lc, false);
    let side = |x: &str| {
        coherences
            .iter()
            .any(|c| c.x == x && c.coherence.iter().any(|col| finite(col)))
    };
    [
        gappy.iter().any(|s| finite(&s.power)),
        maps.iter()
            .any(|m| m.series == "beat_error" && m.values.iter().any(|col| finite(col))),
        side("rate"),
        side("tick_amplitude"),
        kurtograms
            .iter()
            .any(|k| k.series == "beat_error" && k.values.iter().any(|row| finite(row))),
        lines
            .iter()
            .any(|l| l.series == "beat_error" && l.bins.iter().any(|b| b.size.is_finite())),
    ]
}

/// No regular beat: a report of the take with the whole reading's
/// no-watch verdict laid on by hand gives nothing in any reader beside
/// it, where the same report without the verdict gives each figure.
#[test]
fn no_regular_beat_reads_nothing_beside_the_report() {
    let (log, lc) = (take(), config());
    let heard = analyse(&log, None, &lc);
    assert!(heard.no_watch().is_none(), "{:?}", heard.caveats);
    assert_eq!(reads(&log, &heard, &lc), [true; 6]);
    let mut none = heard.clone();
    none.read_no_watch(crate::measure::NoWatchReason::Scatter);
    assert_eq!(reads(&log, &none, &lc), [false; 6]);
}

/// A take of 2 h and more gives a wind-down; with the rate withheld (its
/// lock off every standard rate, laid on by hand) it gives none.
#[test]
fn a_withheld_rate_gives_no_wind_down() {
    let (log, lc) = (take(), config());
    let check = crate::steadiness::Config::default();
    let read = analyse(&log, None, &lc);
    assert!(!read.rate_withheld(), "{:?}", read.caveats);
    let st = crate::steadiness::check(&log, None, &read, &lc, &check);
    assert!(st.wind_down.is_some());
    let mut withheld = read.clone();
    let o = crate::timing::OffStandard::reading(17_570, Some(5.0), None).expect("off standard");
    withheld.read_off_standard(o);
    assert!(withheld.rate_withheld());
    let st = crate::steadiness::check(&log, None, &withheld, &lc, &check);
    assert!(st.wind_down.is_none(), "{:?}", st.wind_down);
}
