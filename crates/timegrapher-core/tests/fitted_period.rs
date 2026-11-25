//! A watch off its set rate reads its amplitude with its own fitted
//! period (#2228), and its beats are numbered across a dropout from their
//! timing (#2229). Norm's rulings on #2223 and #2224.

use timegrapher_core::analysis::{analyze, AnalysisConfig};
use timegrapher_core::audio::Audio;
use timegrapher_core::live::{LiveAnalyzer, LiveConfig};
use timegrapher_core::synth::{generate, SynthConfig};

/// A synthetic balance beating `bph` at 280° for `secs`.
fn watch(bph: u32, secs: f64) -> Audio {
    let cfg = SynthConfig {
        bph,
        duration_s: secs,
        ..Default::default()
    };
    generate(&cfg, |_| 280.0, |_| 0.0)
}

fn set(bph: Option<u32>) -> AnalysisConfig {
    AnalysisConfig {
        bph,
        ..Default::default()
    }
}

fn live(audio: &Audio, bph: Option<u32>) -> LiveAnalyzer {
    let cfg = LiveConfig {
        analysis: set(bph),
        ..Default::default()
    };
    let mut l = LiveAnalyzer::new(audio.sample_rate, cfg);
    for c in audio.samples.chunks(4800) {
        l.push(c);
    }
    l
}

/// A balance at 17,570 vph read set to 18,000, or one at 18,400, reads
/// its true 280°, not the 273° or 286° the set rate's period gives:
/// saved and live alike, and its BE angle at the same period.
#[test]
fn a_watch_off_its_set_rate_reads_its_own_amplitude() {
    for (true_bph, set_bph) in [
        (17_570, Some(18_000)),
        (17_570, None),
        (18_400, Some(18_000)),
    ] {
        let audio = watch(true_bph, 30.0);
        let a = analyze(&audio, &set(set_bph));
        let saved = a.summary.amplitude_deg.expect("an amplitude");
        assert!(
            (saved - 280.0).abs() < 1.0,
            "{true_bph} at {set_bph:?}: {saved}°"
        );
        let r = live(&audio, set_bph).reading(20.0);
        let heard = r.amplitude_deg.expect("a live amplitude");
        assert!(
            (heard - 280.0).abs() < 1.0,
            "{true_bph} at {set_bph:?}, live: {heard}°"
        );
        // The BE angle at the balance's own period, not the set rate's.
        // The synth's watch gains its default rate on its own beat.
        let gains = SynthConfig::default().rate_s_per_day;
        let true_period = timegrapher_core::timing::period_s(3600.0 / true_bph as f64, gains);
        let period = timegrapher_core::be_angle::beat_period_of(r.bph, r.rate_s_per_day).unwrap();
        assert!((period / true_period - 1.0).abs() < 1e-6, "{period}");
        let at = timegrapher_core::be_angle::degrees_at(heard, 0.5, period);
        let own = timegrapher_core::be_angle::degrees_at(280.0, 0.5, true_period);
        assert!((at - own).abs() / own < 0.004, "{at} for {own}");
    }
}

/// `audio` with `gap_s` from 15 s replaced by silence, or by noise at
/// 1.5 times the sound's RMS there.
fn with_gap(mut audio: Audio, gap_s: f64, noise: bool) -> Audio {
    let fs = audio.sample_rate as f64;
    let span = (15.0 * fs) as usize..((15.0 + gap_s) * fs) as usize;
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

/// A balance at 17,570 vph read set to 18,000 and on Auto, across a 6 s
/// silence and a 20 s gap of noise, reads the rate it reads with no gap
/// (Norm on #2229): the beats are numbered across the hole from their
/// timing, not by the set rate's beat; and its amplitude is its own,
/// saved and live.
#[test]
fn a_dropout_does_not_miscount_a_watch_off_its_set_rate() {
    let whole = analyze(&watch(17_570, 40.0), &set(Some(18_000)))
        .summary
        .rate_s_per_day
        .unwrap();
    assert!((whole + 2059.1).abs() < 0.1, "{whole}");
    for set_bph in [Some(18_000), None] {
        for (gap, noise) in [(6.0, false), (20.0, true)] {
            let audio = with_gap(watch(17_570, 40.0), gap, noise);
            let at = format!("{set_bph:?}, {gap} s gap (noise {noise})");
            let summary = analyze(&audio, &set(set_bph)).summary;
            let rate = summary.rate_s_per_day.expect("a rate");
            assert!((rate - whole).abs() < 0.05, "{at}: {rate} for {whole}");
            // The amplitude at the balance's own period, not at one fitted
            // across the hole on a provisional count (Trevor on #2252).
            let amp = summary.amplitude_deg.expect("an amplitude");
            assert!((amp - 280.0).abs() < 1.0, "{at}: {amp}°");
            // Live after the silence; after the noise it reads no watch
            // until #2240's fix (#2247), which tests it there.
            if !noise {
                let heard = live(&audio, set_bph).reading(10.0).amplitude_deg;
                let heard = heard.unwrap_or_else(|| panic!("{at}: no live amplitude"));
                assert!((heard - 280.0).abs() < 1.0, "{at}, live: {heard}°");
            }
        }
    }
}
