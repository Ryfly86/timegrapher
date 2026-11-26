//! A watch put back after a gap of noise is a watch arriving: live shows
//! no figures from the run after the gap until a 10 s window fits over
//! it, as the saved read reads it, and Finding the Beat meanwhile. Stopped,
//! the take reads as `analyze` does: where no run lasted a window, no
//! regular beat, as too short, not as scattered (Norm on #2257).

use timegrapher_core::analysis::{analyze, AnalysisConfig};
use timegrapher_core::audio::Audio;
use timegrapher_core::live::{LiveAnalyzer, LiveConfig, LiveReading, ARRIVING_PAUSE_S, AVERAGES};
use timegrapher_core::measure::{no_watch_reason_in, NoWatchReason};
use timegrapher_core::session::{self, Measurement};
use timegrapher_core::stream::{analyze_samples, StreamConfig};
use timegrapher_core::synth::{generate, SynthConfig};

/// How long the gap of noise is, s.
const GAP_S: f64 = 20.0;

/// A uniform noise sample in -0.5..0.5 from the generator state `x`.
fn noise(x: &mut u64) -> f32 {
    *x = x
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*x >> 33) as f32 / (1u64 << 31) as f32 - 0.5
}

/// A synthetic balance beating 28,800 at 280° for `first` s, then
/// [`GAP_S`] of noise at 1.5 times the sound's RMS there, then the watch
/// again for `back` s.
fn put_back(first: f64, back: f64) -> Audio {
    put_back_after(first, GAP_S, back)
}

/// [`put_back`] after `gap` s of noise.
fn put_back_after(first: f64, gap: f64, back: f64) -> Audio {
    let cfg = SynthConfig {
        bph: 28_800,
        duration_s: first + gap + back,
        ..Default::default()
    };
    let mut audio = generate(&cfg, |_| 280.0, |_| 0.0);
    let fs = audio.sample_rate as f64;
    let span = (first * fs) as usize..((first + gap) * fs) as usize;
    let n = span.len() as f32;
    let rms = (audio.samples[span.clone()]
        .iter()
        .map(|v| v * v)
        .sum::<f32>()
        / n)
        .sqrt();
    let mut x: u64 = 12_345;
    for s in &mut audio.samples[span] {
        *s = 1.5 * rms * noise(&mut x);
    }
    audio
}

/// `secs` of low noise, with clicks at no steady rate (one every 80 to
/// 240 ms) from `burst.0` for `burst.1` s.
fn stand_noise(secs: f64, burst: Option<(f64, f64)>) -> Audio {
    let sample_rate = 48_000;
    let fs = f64::from(sample_rate);
    let mut x: u64 = 777;
    let mut samples: Vec<f32> = (0..(secs * fs) as usize)
        .map(|_| 0.004 * noise(&mut x))
        .collect();
    if let Some((from, len)) = burst {
        let mut at = (from * fs) as usize;
        while at + 200 < ((from + len) * fs) as usize {
            for (i, v) in samples[at..at + 200].iter_mut().enumerate() {
                *v += 0.3 * (-(i as f32) / 40.0).exp() * if i % 2 == 0 { 1.0 } else { -1.0 };
            }
            at += ((0.08 + 0.16 * (f64::from(noise(&mut x)) + 0.5)) * fs) as usize;
        }
    }
    Audio {
        samples,
        sample_rate,
    }
}

fn set(bph: Option<u32>) -> AnalysisConfig {
    AnalysisConfig {
        bph,
        ..Default::default()
    }
}

/// Live fed `audio`, with each reading at every Average Over taken once
/// a second.
fn listen(audio: &Audio, bph: Option<u32>) -> (LiveAnalyzer, Vec<(f64, Vec<LiveReading>)>) {
    let cfg = LiveConfig {
        analysis: set(bph),
        ..Default::default()
    };
    let mut l = LiveAnalyzer::new(audio.sample_rate, cfg);
    let mut out = Vec::new();
    let mut next = 1.0;
    for c in audio.samples.chunks(4800) {
        l.push(c);
        let t = l.duration_s();
        if t >= next {
            next += 1.0;
            out.push((t, AVERAGES.iter().map(|&a| l.reading(a)).collect()));
        }
    }
    (l, out)
}

/// The take's whole reading, as the stopped take is read.
fn whole(audio: &Audio, bph: Option<u32>) -> Measurement {
    let cfg = StreamConfig {
        analysis: set(bph),
        ..Default::default()
    };
    let log = analyze_samples(&audio.samples, audio.sample_rate, &cfg);
    let span = [(0.0, log.duration_s)];
    session::measure_together(&log, None, &[], &[], (&span, &span)).unwrap()
}

/// The stopped take's reading at every Average Over that holds the whole
/// take, as the desktop reads it at Stop: a shorter one reads its own
/// last seconds (`LiveReading::at_whole_end`).
fn stopped(l: &LiveAnalyzer, w: &Measurement) -> Vec<LiveReading> {
    let end = l.duration_s();
    AVERAGES
        .iter()
        .copied()
        .filter(|&a| a >= end)
        .chain([end])
        .map(|a| l.reading_at(end, a).at_whole_end(w, a))
        .collect()
}

/// The first good beat live heard after `from_s`.
fn back_at(l: &LiveAnalyzer, from_s: f64) -> f64 {
    l.beats()
        .iter()
        .find(|b| b.is_good() && b.time > from_s)
        .expect("live hears the watch again")
        .time
}

/// (1) 7 s, 20 s of noise, 9 s back, at 28,800, set and on Auto: live
/// shows no figures from the run put back, at every Average Over, and
/// finds the beat from 2 s after it is heard again. The stopped take
/// reads as `analyze` does: no regular beat, too short.
#[test]
fn a_short_return_is_found_and_stops_as_too_short() {
    let audio = put_back(7.0, 9.0);
    for bph in [Some(28_800), None] {
        let a = analyze(&audio, &set(bph)).summary;
        assert!(!a.watch_heard, "{bph:?}: {:?}", a.caveats);
        assert_eq!(
            no_watch_reason_in(&a.caveats),
            Some(NoWatchReason::TooShort),
            "{bph:?}"
        );
        let (l, readings) = listen(&audio, bph);
        let back = back_at(&l, 7.0 + GAP_S);
        for (t, rs) in readings.iter().filter(|(t, _)| *t > 7.0 + GAP_S) {
            for (r, avg) in rs.iter().zip(AVERAGES) {
                let at = format!("{bph:?} at {t:.1} s, {avg} s");
                assert!(r.rate_s_per_day.is_none(), "{at}: {r:?}");
                assert!(
                    r.amplitude_deg.is_none() && r.beat_error_ms.is_none(),
                    "{at}"
                );
                if *t >= back + 2.0 {
                    assert!(r.finding_beat && !r.watch_heard, "{at}: {:?}", r.caveats);
                }
            }
        }
        let w = whole(&audio, bph);
        assert!(!w.watch_heard, "{bph:?}");
        for r in stopped(&l, &w) {
            assert!(!r.watch_heard && !r.finding_beat, "{bph:?}: {r:?}");
            assert!(r.rate_s_per_day.is_none(), "{bph:?}");
            assert_eq!(
                no_watch_reason_in(&r.caveats),
                Some(NoWatchReason::TooShort),
                "{bph:?}"
            );
        }
    }
}

/// (b) 7 s, 20 s of noise, then 15 s (Norm's case) or 20 s back: live's
/// figures never come before a 10 s window fits over the run put back,
/// at any Average Over. With 20 s back they come before the end; with 15
/// s the end may still be finding the beat. Stopped, both read
/// `analyze`'s rate.
#[test]
fn a_return_is_read_once_a_window_fits_over_it() {
    for (back_s, bph) in [15.0, 20.0]
        .into_iter()
        .flat_map(|s| [(s, Some(28_800)), (s, None)])
    {
        let audio = put_back(7.0, back_s);
        let bph_at = (back_s, bph);
        let rate = analyze(&audio, &set(bph)).summary.rate_s_per_day;
        let rate = rate.unwrap_or_else(|| panic!("{bph_at:?}: analyze reads a rate"));
        let (l, readings) = listen(&audio, bph);
        let back = back_at(&l, 7.0 + GAP_S);
        for (t, rs) in readings.iter().filter(|(t, _)| *t > 7.0 + GAP_S) {
            for (r, avg) in rs.iter().zip(AVERAGES) {
                if r.rate_s_per_day.is_some() {
                    assert!(*t >= back + 10.0, "{bph_at:?} at {t:.1} s, {avg} s: {r:?}");
                }
            }
        }
        let (_, last) = readings.last().unwrap();
        // 20 s back meets the finding wait (`FINDING_S`) before the end;
        // 15 s back may not, and is then still finding the beat, never
        // no regular beat.
        let read = |r: &LiveReading| r.rate_s_per_day.is_some();
        let ok = |r: &LiveReading| read(r) || (back_s < 20.0 && r.finding_beat);
        assert!(last.iter().all(ok), "{bph_at:?}: {last:?}");
        let w = whole(&audio, bph);
        let end = l.duration_s();
        let r = l.reading_at(end, end).at_whole_end(&w, end);
        let heard = r.rate_s_per_day.expect("a stopped rate");
        assert!(
            (heard - rate).abs() < 0.05,
            "{bph_at:?}: {heard} for {rate}"
        );
    }
}

/// (2) Pure stand noise, and stand noise with a 9 s burst of clicks at
/// no steady rate: no regular beat, as scattered, not too short, live and
/// stopped.
#[test]
fn noise_and_an_irregular_burst_stay_scattered() {
    for burst in [None, Some((20.0, 9.0))] {
        let audio = stand_noise(40.0, burst);
        for bph in [Some(28_800), None] {
            let at = format!("burst {burst:?}, {bph:?}");
            let a = analyze(&audio, &set(bph)).summary;
            assert!(!a.watch_heard, "{at}");
            assert_ne!(
                no_watch_reason_in(&a.caveats),
                Some(NoWatchReason::TooShort),
                "{at}"
            );
            let (l, _) = listen(&audio, bph);
            let end = l.duration_s();
            // Live gives no figures and never says too short. (After the
            // burst, on Auto, a 2 s average says a watch with no beats in
            // it, as on main.)
            for &avg in &AVERAGES {
                let r = l.reading(avg);
                assert!(r.rate_s_per_day.is_none(), "{at}, {avg} s: {r:?}");
                assert_ne!(
                    no_watch_reason_in(&r.caveats),
                    Some(NoWatchReason::TooShort),
                    "{at}, {avg} s"
                );
            }
            let w = whole(&audio, bph);
            let r = l.reading_at(end, end).at_whole_end(&w, end);
            assert!(!r.watch_heard, "{at}");
            assert_ne!(
                no_watch_reason_in(&r.caveats),
                Some(NoWatchReason::TooShort),
                "{at}"
            );
        }
    }
}

/// (4) 9 s, 20 s of noise, 9 s back: `analyze` reads the one window the
/// first run fills, and the stopped take reads the same at every Average
/// Over that holds the take, though live is finding the beat put back.
#[test]
fn a_short_return_after_a_read_run_stops_as_analyze_reads() {
    let audio = put_back(9.0, 9.0);
    for bph in [Some(28_800), None] {
        let rate = analyze(&audio, &set(bph)).summary.rate_s_per_day;
        let rate = rate.unwrap_or_else(|| panic!("{bph:?}: analyze reads a rate"));
        assert!((rate - 4.98).abs() < 0.05, "{bph:?}: {rate}");
        let (l, _) = listen(&audio, bph);
        let w = whole(&audio, bph);
        let end = l.duration_s();
        let r = l.reading_at(end, end).at_whole_end(&w, end);
        assert!(
            r.whole_take && r.watch_heard && !r.finding_beat,
            "{bph:?}: {r:?}"
        );
        let heard = r.rate_s_per_day.expect("a stopped rate");
        assert!((heard - rate).abs() < 0.05, "{bph:?}: {heard} for {rate}");
    }
}

/// 7 s, `gap` s of noise, 9 s back.
/// Live's good beats pause for the gap and the time it takes to hear the
/// watch again; at the end, live's reading at each Average Over, and the
/// pause it heard.
fn at_the_end(gap: f64, bph: Option<u32>) -> (Vec<LiveReading>, f64) {
    let audio = put_back_after(7.0, gap, 9.0);
    let (l, _) = listen(&audio, bph);
    let good: Vec<f64> = l
        .beats()
        .iter()
        .filter(|b| b.is_good())
        .map(|b| b.time)
        .collect();
    let pause = good.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max);
    (AVERAGES.iter().map(|&a| l.reading(a)).collect(), pause)
}

/// (Norm on #2257) After 11 s of noise, a pause live hears as 13 s or
/// more, past [`ARRIVING_PAUSE_S`], the watch is arriving: at the end it
/// is found, with no figures, at every Average Over, set and on Auto.
#[test]
fn a_pause_of_a_window_or_more_is_a_watch_arriving() {
    for bph in [Some(28_800), None] {
        let (rs, pause) = at_the_end(11.0, bph);
        assert!(pause >= ARRIVING_PAUSE_S, "{bph:?}: {pause}");
        for (r, avg) in rs.iter().zip(AVERAGES) {
            assert!(
                r.finding_beat && r.rate_s_per_day.is_none(),
                "{bph:?}, {avg} s: {r:?}"
            );
        }
    }
}

/// Norm's HOLD on #2311: the watch read for 15 s, then lifted into
/// [`GAP_S`]-style stand noise for 40 s, never put back. Live's look
/// back cuts the read run, which is no short run: never too short, at
/// any Average Over, set and Auto, live each second and stopped.
#[test]
fn a_lift_after_a_read_run_is_never_too_short() {
    let audio = put_back_after(15.0, 40.0, 0.0);
    for bph in [Some(28_800), None] {
        let (l, readings) = listen(&audio, bph);
        for (t, rs) in &readings {
            for (r, avg) in rs.iter().zip(AVERAGES) {
                assert_ne!(
                    no_watch_reason_in(&r.caveats),
                    Some(NoWatchReason::TooShort),
                    "{bph:?} at {t:.1} s, {avg} s: {r:?}"
                );
            }
        }
        let w = whole(&audio, bph);
        for r in stopped(&l, &w) {
            assert_ne!(
                no_watch_reason_in(&r.caveats),
                Some(NoWatchReason::TooShort),
                "{bph:?} stopped: {r:?}"
            );
        }
        let a = analyze(&audio, &set(bph)).summary;
        assert_ne!(
            no_watch_reason_in(&a.caveats),
            Some(NoWatchReason::TooShort),
            "{bph:?}"
        );
    }
}
