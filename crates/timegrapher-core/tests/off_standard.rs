//! Auto far from every standard rate (Norm on #2223, point 5): after the
//! fit, a rate further than `beats::STANDARD_REACH_S_PER_DAY` from the
//! standard rate guessed gives no rate in s/d, the beat rate heard, and
//! the caveat `off_standard_rate` with the standard rate on each side.
//! `analyze`, live and a saved take's reading make the one check.

use timegrapher_core::analysis::{analyze, AnalysisConfig, Summary};
use timegrapher_core::audio::Audio;
use timegrapher_core::live::{LiveAnalyzer, LiveConfig, LiveReading};
use timegrapher_core::measure::{off_standard, Caveat};
use timegrapher_core::synth::{generate, SynthConfig};
use timegrapher_core::timing::OffStandard;

/// A synthetic balance beating `bph` at 280° for `secs`, gaining `rate`
/// s/d on it.
fn watch(bph: u32, rate: f64, secs: f64) -> Audio {
    let cfg = SynthConfig {
        bph,
        rate_s_per_day: rate,
        duration_s: secs,
        ..Default::default()
    };
    generate(&cfg, |_| 280.0, |_| 0.0)
}

/// The synth's balance heard: `bph` gaining `rate` s/d, as bph.
fn heard(bph: u32, rate: f64) -> f64 {
    f64::from(bph) * (1.0 + rate / 86_400.0)
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

/// `audio` with `gap_s` from 15 s replaced by silence, or by noise at
/// 1.5 times the sound's RMS there, as `tests/fitted_period.rs` builds it
/// (#2229, #2253).
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

/// The rate a reading off every standard rate reads against `bph`, s/d,
/// from its caveat's side.
fn side_rate(o: &OffStandard, bph: u32) -> f64 {
    [o.below, o.above]
        .into_iter()
        .flatten()
        .find(|s| s.bph == bph)
        .unwrap_or_else(|| panic!("no {bph} side in {o:?}"))
        .rate_s_per_day
}

/// No rate in s/d anywhere in a summary off every standard rate.
fn no_rate(s: &Summary) {
    assert_eq!(s.rate_s_per_day, None);
    assert_eq!(s.rate_corrected_s_per_day, None);
    assert_eq!((s.rate_p05, s.rate_p95), (None, None));
    assert_eq!(s.confidence.as_ref().and_then(|c| c.rate.se), None);
}

/// No rate in s/d in a live reading off every standard rate.
fn no_live_rate(r: &LiveReading) {
    assert_eq!(r.rate_s_per_day, None);
    assert_eq!(r.rate_error_s_per_day, None);
}

/// A balance at 17,570 on Auto reads no rate, the beat rate heard, and
/// the standard rate on each side with the rate it reads against each:
/// against 18,000 the rate a set 18,000 reads. Its amplitude and beat
/// error stay.
#[test]
fn a_watch_at_17570_on_auto_has_no_rate_and_the_rate_heard() {
    let audio = watch(17_570, 5.0, 60.0);
    let at_18000 = analyze(&audio, &set(Some(18_000))).summary;
    assert!(off_standard(&at_18000.caveats).is_none(), "a set rate");
    let set_rate = at_18000.rate_s_per_day.expect("a set rate's rate");
    let s = analyze(&audio, &set(None)).summary;
    let o = off_standard(&s.caveats).expect("off_standard_rate");
    no_rate(&s);
    let want = heard(17_570, 5.0);
    assert!((o.bph_measured - want).abs() < 0.05, "{o:?} for {want}");
    let (below, above) = (o.below.expect("below"), o.above.expect("above"));
    assert_eq!((below.bph, above.bph), (17_280, 18_000));
    assert!(
        (side_rate(&o, 18_000) - set_rate).abs() < 0.05,
        "{o:?}, {set_rate}"
    );
    let against_17280 = (want / 17_280.0 - 1.0) * 86_400.0;
    assert!(
        (below.rate_s_per_day - against_17280).abs() < 0.2,
        "{below:?}"
    );
    // A minute holds too few 10 s windows for a rate ±, so none here.
    assert_eq!(o.bph_measured_se, None);
    assert!(s.amplitude_deg.is_some_and(|a| (a - 280.0).abs() < 1.0));
    assert!(s.beat_error_unlock_ms.is_some());
}

/// A watch within the reach (+900 s/d off 18,000, and −900) reads its
/// rate against 18,000 with no caveat; one past it (+1,100) has none.
#[test]
fn the_reach_is_1000_s_per_day_after_the_fit() {
    for rate in [900.0, -900.0] {
        let s = analyze(&watch(18_000, rate, 40.0), &set(None)).summary;
        assert_eq!(s.bph, 18_000, "{rate}");
        assert!(
            off_standard(&s.caveats).is_none(),
            "{rate}: {:?}",
            s.caveats
        );
        let read = s.rate_s_per_day.expect("a rate");
        assert!((read - rate).abs() < 0.5, "{read} for {rate}");
    }
    let s = analyze(&watch(18_000, 1_100.0, 40.0), &set(None)).summary;
    let o = off_standard(&s.caveats).expect("past the reach");
    no_rate(&s);
    assert!((side_rate(&o, 18_000) - 1_100.0).abs() < 0.5, "{o:?}");
}

/// Live on Auto across a 6 s silence or a 20 s gap of noise (#2253):
/// before, live re-guessed 17,280 after the silence and read +1455.1 s/d
/// where `analyze` read −2059.1 against 18,000. Now both read no rate and
/// the beat rate heard, and their sides' rates agree within 0.05 s/d,
/// over the whole take and over the last 10 s; with no gap too.
#[test]
fn live_on_auto_across_a_gap_agrees_with_analyze() {
    for (secs, gap) in [
        (40.0, None),
        (40.0, Some((6.0, false))),
        (70.0, Some((20.0, true))),
    ] {
        let audio = watch(17_570, 5.0, secs);
        let audio = gap.map_or(audio.clone(), |(g, noise)| with_gap(audio, g, noise));
        let s = analyze(&audio, &set(None)).summary;
        let saved = off_standard(&s.caveats).expect("saved off_standard_rate");
        let l = live(&audio, None);
        for average_s in [secs, 10.0] {
            let at = format!("gap {gap:?}, over {average_s} s");
            let r = l.reading(average_s);
            let o = off_standard(&r.caveats).unwrap_or_else(|| panic!("{at}: {:?}", r.caveats));
            no_live_rate(&r);
            let m = r.measured().expect("the beat rate heard");
            assert_eq!(m.bph, o.bph_measured, "{at}");
            for side in [17_280, 18_000] {
                let (heard, read) = (side_rate(&o, side), side_rate(&saved, side));
                assert!(
                    (heard - read).abs() < 0.05,
                    "{at}: {heard} for {read} at {side}"
                );
            }
        }
    }
}

/// A set rate is the target however far the watch is from it: live and
/// saved, 17,570 set to 18,000 reads its rate and no caveat.
#[test]
fn a_set_rate_is_never_off_standard() {
    let audio = watch(17_570, 5.0, 30.0);
    let r = live(&audio, Some(18_000)).reading(20.0);
    assert!(off_standard(&r.caveats).is_none(), "{:?}", r.caveats);
    let rate = r.rate_s_per_day.expect("a rate");
    assert!((rate + 2_059.1).abs() < 0.1, "{rate}");
}

/// The caveat in words: the beat rate heard, the reach, and each side.
#[test]
fn the_caveat_in_words() {
    let o = OffStandard::of(None, 18_000, Some(-2_059.11), None).unwrap();
    let c = Caveat::OffStandardRate(o);
    assert_eq!(
        c.short(52.0, 0).as_deref(),
        Some("not a standard rate: 17571 bph")
    );
    assert_eq!(
        c.long(None).as_deref(),
        Some(
            "The beat rate heard, 17571 bph, is more than 1000 s/d from every standard rate \
             (+1455.1 s/d against 17280; -2059.1 s/d against 18000), so no rate in s/d is \
             given."
        )
    );
    assert_eq!(
        c.reading_scope(),
        Some(timegrapher_core::measure::Scope::Rate)
    );
}

/// The JSON of `analyze` off every standard rate: no rate anywhere, the
/// caveat with the beat rate heard and both sides, and `bph_measured`
/// equal to the caveat's, with and without a clock correction.
#[test]
fn the_analyze_document_off_standard() {
    let s = analyze(&watch(17_570, 5.0, 40.0), &set(None)).summary;
    for clock in [None, Some((20.0, "a stored correction"))] {
        let doc = timegrapher_core::analyze_doc::body(&s, clock).unwrap();
        assert!(doc["rate_s_per_day"].is_null(), "{clock:?}");
        assert!(doc["overall"]["rate_s_per_day"].is_null(), "{clock:?}");
        assert!(doc["clock"]["rate_s_per_day"].is_null(), "{clock:?}");
        let c = doc["caveats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["code"] == "off_standard_rate")
            .unwrap_or_else(|| panic!("{clock:?}: {}", doc["caveats"]));
        assert_eq!(c["bph_measured"], doc["bph_measured"], "{clock:?}");
        assert_eq!(c["bph_measured_se"], doc["bph_measured_se"], "{clock:?}");
        assert_eq!(c["below"]["bph"], 17_280);
        assert_eq!(c["above"]["bph"], 18_000);
        // The beat rate heard on the clock the document corrects to.
        let card = side_rate(&off_standard(&s.caveats).unwrap(), 18_000);
        let rate = clock.map_or(card, |(ppm, _)| {
            timegrapher_core::clockstore::correct_rate(card, ppm)
        });
        let want = 18_000.0 * (1.0 + rate / 86_400.0);
        let got = doc["bph_measured"].as_f64().unwrap();
        assert!((got - want).abs() < 0.01, "{clock:?}: {got} for {want}");
    }
}

/// Each side is the nearest standard rate, none past the slowest or the
/// fastest; within the reach, or with a rate set, there is no caveat.
#[test]
fn the_sides_and_the_reach() {
    let o = OffStandard::of(None, 12_000, Some(-8_640.0), None).unwrap();
    assert_eq!(o.below, None);
    assert_eq!(o.above.map(|s| s.bph), Some(12_000));
    let o = OffStandard::of(None, 72_000, Some(2_000.0), None).unwrap();
    assert_eq!((o.below.map(|s| s.bph), o.above), (Some(72_000), None));
    assert!(OffStandard::of(None, 18_000, Some(1_000.0), None).is_none());
    assert!(OffStandard::of(None, 18_000, Some(-1_000.0), None).is_none());
    assert!(OffStandard::of(None, 18_000, Some(1_000.01), None).is_some());
    assert!(OffStandard::of(Some(18_000), 18_000, Some(-2_059.0), None).is_none());
    assert!(OffStandard::of(None, 18_000, None, None).is_none());
    // The rate's ± goes into the beat rate heard's, `bph × σ / 86,400`,
    // never shown as s/d.
    let o = OffStandard::of(None, 18_000, Some(-2_059.0), Some(4.8)).unwrap();
    assert!((o.bph_measured_se.unwrap() - 1.0).abs() < 1e-12, "{o:?}");
    assert_eq!(timegrapher_core::beats::STANDARD_REACH_S_PER_DAY, 1_000.0);
}

/// A saved take's reading on Auto makes the same check
/// (`session::Measurement::read_off_standard`, which `take::measure_opened`
/// calls for every `session` reading and `record` check): no rate, the
/// beat rate heard as `analyze` reads it, and nothing with a rate set.
#[test]
fn a_saved_reading_on_auto_is_off_standard_as_analyze_is() {
    let audio = watch(17_570, 5.0, 40.0);
    let saved = off_standard(&analyze(&audio, &set(None)).summary.caveats).unwrap();
    let cfg = timegrapher_core::stream::StreamConfig::default();
    let log = timegrapher_core::stream::analyze_samples(&audio.samples, audio.sample_rate, &cfg);
    let read = timegrapher_core::session::measure(&log, None, 0.0, log.duration_s);
    let mut set_18000 = read.clone();
    set_18000.read_off_standard(Some(18_000), log.bph);
    assert!(off_standard(&set_18000.caveats).is_none());
    assert!(set_18000.rate_s_per_day.is_some());
    let mut m = read;
    m.read_off_standard(None, log.bph);
    let o = off_standard(&m.caveats).expect("off_standard_rate");
    assert_eq!(m.rate_s_per_day, None);
    assert_eq!(m.rate_window_median_s_per_day, None);
    assert_eq!((m.rate_p05, m.rate_p95), (None, None));
    assert_eq!(m.measured(log.bph).map(|h| h.bph), Some(o.bph_measured));
    for side in [17_280, 18_000] {
        let (read, want) = (side_rate(&o, side), side_rate(&saved, side));
        assert!((read - want).abs() < 0.05, "{read} for {want} at {side}");
    }
}
