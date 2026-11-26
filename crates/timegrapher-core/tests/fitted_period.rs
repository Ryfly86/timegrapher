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
            // Live after the silence and after the noise (#2240, #2247), on
            // a take long enough to be read again after the gap.
            let long = with_gap(watch(17_570, 70.0), gap, noise);
            let heard = live(&long, set_bph).reading(10.0).amplitude_deg;
            let heard = heard.unwrap_or_else(|| panic!("{at}: no live amplitude"));
            assert!((heard - 280.0).abs() < 1.0, "{at}, live: {heard}°");
        }
    }
}

/// The live Tick and Tock profiles of a watch off its set rate read its
/// own 280°, as its amplitude does, and keep it at another lift (Trevor
/// on #2252).
#[test]
fn a_watch_off_its_set_rate_profiles_at_its_own_amplitude() {
    for set_bph in [Some(18_000), None] {
        let l = live(&watch(17_570, 30.0), set_bph);
        for p in l.tick_profiles() {
            let p = p.as_ref().expect("a profile");
            let a = p.amplitude_deg.expect("a profile amplitude");
            assert!((a - 280.0).abs() < 1.0, "{set_bph:?}, {:?}: {a}°", p.side);
            // At another lift its amplitude is scaled, at the period it was
            // read with, whatever period is given; only one it lacks is read
            // from its edges at that period (`TickProfile::at_lift`).
            let scaled = p.at_lift(0.4, 52.0, 45.0).expect("in the gate");
            assert!(
                (scaled - a * 45.0 / 52.0).abs() < 1e-9,
                "{scaled} for {a}° × 45/52"
            );
        }
    }
}

/// The mean of a log's window amplitudes, both sides; None with none.
fn windows_mean(log: &timegrapher_core::stream::BeatLog) -> Option<f64> {
    let v: Vec<f64> = log
        .amplitude_windows
        .iter()
        .flat_map(|w| [w.even_deg, w.odd_deg])
        .flatten()
        .collect();
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

fn at_lift(bph: Option<u32>, lift_deg: f64) -> AnalysisConfig {
    let mut cfg = set(bph);
    cfg.amplitude.lift_deg = lift_deg;
    cfg
}

/// A balance at 17,570 vph, read set to 18,000, has its lift changed,
/// saved (`take::kept_at_lift`) and live (`set_lift`): its amplitude, and
/// live its profiles, read 280° × new / old. Read first at 72°, where its
/// 388° is past the gate and culled, then at 52°, each side is read again
/// from its kept edges, at the balance's own period: 280°, not the 273°
/// the set rate gives (Trevor on #2252).
#[test]
fn a_lift_change_off_the_set_rate_reads_at_the_own_period() {
    let lift = timegrapher_core::take::DEFAULT_LIFT_DEG;
    let audio = watch(17_570, 30.0);
    for (from, to) in [(lift, 45.0), (72.0, lift)] {
        let want = 280.0 * lift.min(to) / lift;
        let at = format!("{from}° to {to}°");
        let cfg = timegrapher_core::stream::StreamConfig {
            analysis: at_lift(Some(18_000), from),
            ..Default::default()
        };
        let log =
            timegrapher_core::stream::analyze_samples(&audio.samples, audio.sample_rate, &cfg);
        if from > lift {
            assert_eq!(windows_mean(&log), None, "{at}: culled at {from}°");
        }
        let saved = windows_mean(&timegrapher_core::take::kept_at_lift(
            &log,
            to,
            Default::default(),
        ));
        let saved = saved.unwrap_or_else(|| panic!("{at}: no saved amplitude"));
        assert!(
            (saved - want).abs() < 1.0,
            "{at}, saved: {saved}° for {want}°"
        );

        let mut l = LiveAnalyzer::new(
            audio.sample_rate,
            LiveConfig {
                analysis: at_lift(Some(18_000), from),
                ..Default::default()
            },
        );
        for c in audio.samples.chunks(4800) {
            l.push(c);
        }
        if from > lift {
            assert!(l
                .tick_profiles()
                .iter()
                .flatten()
                .all(|p| p.amplitude_deg.is_none()));
        }
        l.set_lift(to);
        let heard = l.reading(20.0).amplitude_deg;
        let heard = heard.unwrap_or_else(|| panic!("{at}: no live amplitude"));
        assert!(
            (heard - want).abs() < 1.0,
            "{at}, live: {heard}° for {want}°"
        );
        for p in l.tick_profiles() {
            let p = p.as_ref().expect("a profile");
            let a = p.amplitude_deg.expect("a profile amplitude");
            assert!(
                (a - want).abs() < 1.0,
                "{at}, {:?}: {a}° for {want}°",
                p.side
            );
        }
    }
}

/// The profiles a take keeps of a watch off its set rate, from its second
/// reading (`keeper::SecondReading`), read its own 280° (Trevor on #2252).
#[test]
fn a_kept_take_off_its_set_rate_profiles_at_its_own_amplitude() {
    let dir = std::env::temp_dir().join(format!("tg-fitted-period-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("take_part01-001.wav");
    timegrapher_core::audio::write_wav(&path, &watch(17_570, 40.0)).unwrap();
    let cfg = timegrapher_core::stream::StreamConfig {
        analysis: set(Some(18_000)),
        ..Default::default()
    };
    let soft = serde_json::json!({"name": "timegrapher", "version": "test"});
    let kept = timegrapher_core::analysis_take::measure(&[path.as_path()], &cfg, &[], soft, |_| {});
    let _ = std::fs::remove_dir_all(&dir);
    let kept = kept.unwrap();
    let sides: Vec<_> = kept
        .profiles
        .iter()
        .flat_map(|s| s.sides.iter().flatten())
        .collect();
    assert!(sides.len() >= 4, "{} profiles", sides.len());
    for p in sides {
        let a = p.amplitude_deg.expect("a profile amplitude");
        assert!((a - 280.0).abs() < 1.0, "{:?}: {a}°", p.side);
    }
}

/// An hour of a Power Reserve run that reads −2059 s/d at 18,000 has its
/// BE angle at its own period, not the set rate's (Trevor on #2252).
#[test]
fn an_hour_off_its_set_rate_has_its_be_angle_at_its_own_period() {
    let hour: timegrapher_core::power_reserve::Hour = serde_json::from_value(serde_json::json!({
        "hour": 0, "span_s": 3600.0, "read_s": 3600.0, "bph": 18000,
        "rate_s_per_day": -2059.1, "amplitude_deg": 280.0, "beat_error_ms": 2.0,
    }))
    .unwrap();
    let own = timegrapher_core::timing::period_s(0.2, -2059.1);
    let want = timegrapher_core::be_angle::degrees_at(280.0, 2.0, own);
    let at = hour.be_angle().expect("an angle").degrees;
    assert!((at - want).abs() < 1e-9, "{at} for {want}");
    let set = timegrapher_core::be_angle::degrees_at(280.0, 2.0, 0.2);
    assert!(
        (set - want).abs() > 0.05,
        "the set rate's {set}° is told apart"
    );
}

/// Beats `from` on, `n` of them a period `p` s apart, from `t0` s, of
/// `quality`.
fn run(from: i64, n: i64, t0: f64, p: f64, quality: f32) -> Vec<timegrapher_core::beats::Beat> {
    (0..n)
        .map(|k| timegrapher_core::beats::Beat {
            index: from + k,
            time: t0 + k as f64 * p,
            quality,
        })
        .collect()
}

/// The own period's two fallbacks (Trevor on #2252), a balance at 17,570
/// set to 18,000 whose count across a 3 s pause is two beats out: with
/// no run of 10 s between pauses, the longest run alone is fitted, not
/// the nominal 3600/bph taken; and a pause where only bad beats are
/// heard still cuts, so the count across it is not fitted.
#[test]
fn the_own_period_cuts_at_good_pauses_and_falls_back_to_the_longest_run() {
    use timegrapher_core::timing::own_beat_period_s;
    let p: f64 = 3600.0 / 17_570.0;
    let gap = (3.0 / p).round() as i64;
    let pieced = |a: i64, b: i64, bad: bool| {
        let mut beats = run(0, a, 0.0, p, 1.0);
        if bad {
            beats.extend(run(a, gap, a as f64 * p, p, 0.0));
        }
        beats.extend(run(a + gap + 2, b, (a + gap) as f64 * p, p, 1.0));
        beats
    };
    // 8 s, then 6 s: neither a whole piece.
    let short = own_beat_period_s(&pieced(39, 29, false), 18_000);
    assert!((short / p - 1.0).abs() < 1e-9, "{short} for {p}");
    // 15 s either side of 3 s of bad beats.
    let bad = own_beat_period_s(&pieced(73, 73, true), 18_000);
    assert!((bad / p - 1.0).abs() < 1e-9, "{bad} for {p}");
}

/// A BE angle needs a period: none at 0 or NaN (Trevor on #2252).
#[test]
fn a_be_angle_needs_a_period() {
    use timegrapher_core::be_angle::Angle;
    for period in [0.0, f64::NAN, -0.2] {
        assert_eq!(
            Angle::at((280.0, None), (2.0, None), period),
            None,
            "{period}"
        );
    }
    assert!(Angle::at((280.0, None), (2.0, None), 0.2).is_some());
}
