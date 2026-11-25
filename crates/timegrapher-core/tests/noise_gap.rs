//! A watch heard either side of a gap of noise holds a watch, live as
//! saved (#2240, Norm's ruling): the noise is a hole in its beats, read
//! with its beats missing, not the watch's own scatter. Once the beats
//! jump back onto it, the watch is a beat being found until it is read,
//! never "No Regular Beat".

use timegrapher_core::analysis::{analyze, AnalysisConfig};
use timegrapher_core::audio::Audio;
use timegrapher_core::live::{
    finding_s, LiveAnalyzer, LiveConfig, LiveReading, TrendReading, AVERAGES,
};
use timegrapher_core::measure::Caveat;
use timegrapher_core::session;
use timegrapher_core::stream::{analyze_samples, BeatLog, StreamConfig};
use timegrapher_core::synth::{generate, SynthConfig};

/// Where the gap starts, s, and how long it is.
const GAP_FROM_S: f64 = 15.0;
const GAP_S: f64 = 20.0;

/// A synthetic balance beating `bph` at 280° for `secs`, with the gap
/// filled with noise at 1.5 times the sound's RMS there, or silence.
fn gapped(bph: u32, secs: f64, noise: bool) -> Audio {
    let cfg = SynthConfig {
        bph,
        duration_s: secs,
        ..Default::default()
    };
    let mut audio = generate(&cfg, |_| 280.0, |_| 0.0);
    let fs = audio.sample_rate as f64;
    let span = (GAP_FROM_S * fs) as usize..((GAP_FROM_S + GAP_S) * fs) as usize;
    let n = span.len() as f32;
    let rms = (audio.samples[span.clone()]
        .iter()
        .map(|v| v * v)
        .sum::<f32>()
        / n)
        .sqrt();
    let mut x: u64 = 12_345;
    for s in &mut audio.samples[span] {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let u = (x >> 33) as f32 / (1u64 << 31) as f32 - 0.5;
        *s = if noise { 1.5 * rms * u } else { 0.0 };
    }
    audio
}

fn set(bph: Option<u32>) -> AnalysisConfig {
    AnalysisConfig {
        bph,
        ..Default::default()
    }
}

fn live_cfg(bph: Option<u32>) -> LiveConfig {
    LiveConfig {
        analysis: set(bph),
        ..Default::default()
    }
}

fn saved_log(audio: &Audio, bph: Option<u32>) -> BeatLog {
    let cfg = StreamConfig {
        analysis: set(bph),
        ..Default::default()
    };
    analyze_samples(&audio.samples, audio.sample_rate, &cfg)
}

/// Each case: the balance's rate, the rate set, and noise or silence.
/// The issue's 17,570 set to 18,000, and 18,000 and 28,800 true (d).
const CASES: [(u32, Option<u32>, bool); 6] = [
    (17_570, Some(18_000), true),
    (18_000, Some(18_000), true),
    (28_800, Some(28_800), true),
    (18_000, None, true),
    (18_000, Some(18_000), false),
    (28_800, Some(28_800), false),
];

/// (a), (d): over the whole 40 s, live reads the watch, and at the take's
/// end its reading is the whole take's: `analyze`'s within 0.05 s/d, with
/// its beats missing.
#[test]
fn live_reads_the_watch_across_a_gap_as_analyze_does() {
    for (bph, set_bph, noise) in CASES {
        let audio = gapped(bph, 40.0, noise);
        let at = format!("{bph} at {set_bph:?}, noise {noise}");
        let a = analyze(&audio, &set(set_bph)).summary;
        let rate = a.rate_s_per_day.expect("analyze reads a rate");
        let missing = |c: &[Caveat]| c.iter().any(|c| matches!(c, Caveat::BeatsMissing { .. }));
        assert!(
            a.watch_heard && missing(&a.caveats),
            "{at}: {:?}",
            a.caveats
        );
        let mut l = LiveAnalyzer::new(audio.sample_rate, live_cfg(set_bph));
        for c in audio.samples.chunks(4800) {
            l.push(c);
        }
        let end = l.duration_s();
        let own = l.reading(end);
        assert!(own.watch_heard, "{at}, live: {:?}", own.caveats);
        // At its end, as the stopped take reads it (`at_whole_end`).
        let log = saved_log(&audio, set_bph);
        let span = [(0.0, log.duration_s)];
        let whole = session::measure_together(&log, None, &[], &[], (&span, &span)).unwrap();
        let r = l.reading_at(end, end).at_whole_end(&whole, end);
        assert!(r.whole_take && r.watch_heard, "{at}: {r:?}");
        let heard = r.rate_s_per_day.expect("a live rate");
        assert!((heard - rate).abs() < 0.05, "{at}: {heard} for {rate}");
        assert!(missing(&r.caveats), "{at}: {:?}", r.caveats);
    }
}

/// Whether a reading says "No Regular Beat": no watch, and no beat being
/// found.
fn no_regular_beat(r: &LiveReading) -> bool {
    !r.watch_heard && !r.finding_beat
}

/// (b), (d): from the jump onto the watch, at every Average Over, each
/// reading is Finding the Beat or figures, never No Regular Beat, and
/// figures come within `finding_s` of the jump.
#[test]
fn after_a_gap_the_watch_is_found_until_it_is_read() {
    let mut bad = Vec::new();
    for (bph, set_bph, noise) in CASES {
        let audio = gapped(bph, 110.0, noise);
        let mut l = LiveAnalyzer::new(audio.sample_rate, live_cfg(set_bph));
        let mut readings: Vec<(f64, Vec<LiveReading>)> = Vec::new();
        for c in audio.samples.chunks(4800) {
            l.push(c);
            let t = l.duration_s();
            if t > GAP_FROM_S + GAP_S {
                // Whether live has found the jump back onto the watch yet.
                let found = l.jumps().iter().any(|&j| j >= GAP_FROM_S + GAP_S - 1.0);
                if found {
                    readings.push((t, AVERAGES.iter().map(|&s| l.reading(s)).collect()));
                }
            }
        }
        let back = GAP_FROM_S + GAP_S;
        let jump = (l.jumps().iter().copied())
            .find(|&j| j >= back - 1.0)
            .unwrap_or(back);
        for (k, &average_s) in AVERAGES.iter().enumerate() {
            let at = format!("{bph} at {set_bph:?}, noise {noise}, {average_s} s");
            let mut read_at = None;
            for (t, rs) in readings.iter().filter(|(t, _)| *t > jump) {
                let r = &rs[k];
                if no_regular_beat(r) {
                    bad.push(format!(
                        "{at}: NRB at {t:.1} jump {jump:.1} {:?} scatter {:?}",
                        r.caveats.first(),
                        r.timing_scatter_fraction
                    ));
                }
                if read_at.is_none() && r.rate_s_per_day.is_some() {
                    read_at = Some(*t);
                }
            }
            let read_at = read_at.unwrap_or(1e9);
            if read_at - jump > finding_s(average_s) + 1e-9 {
                bad.push(format!(
                    "{at}: read {:.1} s after the jump at {jump:.1}",
                    read_at - jump
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Live's verdict on a reading: heard, the beat being found, or No
/// Regular Beat.
fn verdict(r: &LiveReading) -> &'static str {
    match (r.watch_heard, r.finding_beat) {
        (true, _) => "heard",
        (false, true) => "finding",
        (false, false) => "no regular beat",
    }
}

/// (c): the saved take's trend and live's reading agree at each point,
/// the verdict and the figures, outside the gap and the `finding_s`
/// after the watch is found again, where each finds it in its own time:
/// each jumps back onto it a second or so apart.
#[test]
fn the_saved_trend_after_a_gap_is_lives_reading() {
    let mut bad = Vec::new();
    for (bph, set_bph, noise) in CASES {
        let audio = gapped(bph, 130.0, noise);
        let mut l = LiveAnalyzer::new(audio.sample_rate, live_cfg(set_bph));
        for c in audio.samples.chunks(4800) {
            l.push(c);
        }
        let log = saved_log(&audio, set_bph);
        // Where each found the watch again: the later of its jumps back.
        let back = GAP_FROM_S + GAP_S;
        let jump = |jumps: &[f64]| jumps.iter().copied().find(|&j| j >= back - 1.0);
        let found = [jump(l.jumps()), jump(&log.jumps), Some(back)];
        let found = found.into_iter().flatten().fold(back, f64::max);
        let saved = LiveAnalyzer::from_log(log, live_cfg(set_bph));
        for average_s in [10.0, 60.0] {
            let edge = GAP_FROM_S..found + finding_s(average_s);
            for p in saved.trend(average_s) {
                if edge.contains(&p.t) {
                    continue;
                }
                let at = format!(
                    "{bph} at {set_bph:?}, noise {noise}, {average_s} s at {:.1} s",
                    p.t
                );
                let (lv, sv) = (
                    l.reading_at(p.t, average_s),
                    saved.reading_at(p.t, average_s),
                );
                if verdict(&lv) != verdict(&sv) {
                    bad.push(format!(
                        "{at}: {} saved, {} live",
                        verdict(&sv),
                        verdict(&lv)
                    ));
                }
                let r = TrendReading::of(p.t, &lv);
                if p.heard != r.heard {
                    bad.push(format!("{at}: heard {} saved, {} live", p.heard, r.heard));
                }
                match (p.rate_s_per_day, r.rate_s_per_day) {
                    (Some(a), Some(b)) if (a - b).abs() < 0.5 => {}
                    (None, None) => {}
                    (a, b) => bad.push(format!("{at}: {a:?} s/d saved, {b:?} live")),
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
