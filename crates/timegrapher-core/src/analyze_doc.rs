//! The body of `analyze --json` (`timegrapher.analyze/2`), worded once
//! for every surface that writes one: the command line, and the app's
//! bug report, whose excerpt `report --replay` analyses again and
//! compares. Each surface wraps it in its own envelope (schema, software,
//! input files).

use crate::analysis::{AnalysisConfig, Summary};
use serde_json::{json, Value};

/// The settings `analyze` read at, as its document's `input.settings`
/// gives them; `calibre` is the calibre named, if one was.
pub fn settings(cfg: &AnalysisConfig, calibre: Option<&str>) -> Value {
    json!({
        "calibre": calibre,
        "bph": cfg.bph,
        "lift_deg": cfg.amplitude.lift_deg,
        "notch_hz": cfg.envelope.notch_hz,
        "highpass_hz": cfg.envelope.highpass_hz,
        "escape_teeth": cfg.escape_teeth,
    })
}

/// The document's body: the summary, its rates on the sound card's clock
/// (what tg reads and the regression check compares), with the clock's
/// correction beside them when one is known (`ppm` fast, and where it came
/// from in words), and the beat rate heard ([`measured`]).
pub fn body(summary: &Summary, clock: Option<(f64, &str)>) -> Result<Value, String> {
    let mut body = serde_json::to_value(summary).map_err(|e| e.to_string())?;
    if let Some(m) = measured(summary, clock.map(|(ppm, _)| ppm)) {
        m.write_into(&mut body);
    }
    if let Some((ppm, from)) = clock {
        body["clock"] = crate::clockstore::correction_json(
            ppm,
            from,
            summary.overall.as_ref().map(|f| f.rate_s_per_day),
        );
    }
    Ok(body)
}

/// The beat rate heard over the whole recording (#2223): from the fit
/// over every beat, on the clock its rate is shown on, corrected at `ppm`
/// when the document carries a correction, with the rate's ±. None when
/// no regular beat is found, as every reading.
pub fn measured(summary: &Summary, ppm: Option<f64>) -> Option<crate::timing::MeasuredRate> {
    let rate = summary.overall.as_ref().map(|f| {
        ppm.map_or(f.rate_s_per_day, |p| {
            crate::clockstore::correct_rate(f.rate_s_per_day, p)
        })
    });
    let se = summary.confidence.as_ref().and_then(|c| c.rate.se);
    crate::timing::MeasuredRate::of(summary.bph, rate, se)
}

/// The correction `analyze` puts beside a summary with no other given:
/// the recording's own USB lines, when it has them and they apply
/// ([`crate::usbclock::UsbClock::applies`]).
pub fn usb_clock(summary: &Summary) -> Option<(f64, String)> {
    summary
        .usb_clock
        .and_then(crate::usbclock::UsbClock::applied)
        .map(|c| c.correction())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::analyze;
    use crate::synth::{generate, SynthConfig};

    /// The settings name what was read at, and the body carries the
    /// summary with the correction beside its unchanged card rate.
    #[test]
    fn the_body_carries_the_summary_and_the_correction_beside_it() {
        let audio = generate(
            &SynthConfig {
                duration_s: 20.0,
                ..Default::default()
            },
            |_| 270.0,
            |_| 0.0,
        );
        let cfg = AnalysisConfig::default();
        let a = analyze(&audio, &cfg);
        let s = settings(&cfg, Some("ETA 2824-2"));
        assert_eq!(s["calibre"], "ETA 2824-2");
        assert_eq!(s["highpass_hz"], cfg.envelope.highpass_hz);
        let plain = body(&a.summary, None).unwrap();
        assert_eq!(plain["bph"], a.summary.bph);
        assert!(plain.get("clock").is_none());
        let corrected = body(&a.summary, Some((10.0, "typed in"))).unwrap();
        assert_eq!(corrected["clock"]["ppm"], 10.0);
        assert_eq!(corrected["clock"]["from"], "typed in");
        assert_eq!(corrected["bph"], plain["bph"]);
    }

    /// A balance beating 17,570 read against 18,000 (the hairspring
    /// maker's case, #2223): the document gives the beat rate heard,
    /// 17570 at its place, 430 bph slow of the set rate, from the same
    /// fit as the rate it gives; on the clock the rate is corrected to
    /// when the document carries a correction.
    #[test]
    fn the_document_gives_the_beat_rate_heard() {
        let audio = generate(
            &SynthConfig {
                duration_s: 60.0,
                bph: 17_570,
                rate_s_per_day: 0.0,
                ..Default::default()
            },
            |_| 280.0,
            |_| 0.0,
        );
        let cfg = AnalysisConfig {
            bph: Some(18_000),
            ..Default::default()
        };
        let a = analyze(&audio, &cfg);
        assert!(a.summary.watch_heard);
        let doc = body(&a.summary, None).unwrap();
        let heard = doc["bph_measured"].as_f64().unwrap();
        let m = measured(&a.summary, None).unwrap();
        assert_eq!(heard, m.bph);
        assert_eq!((m.whole(), m.offset_from(18_000)), (17_570.0, -430.0));
        let rate = a.summary.overall.unwrap().rate_s_per_day;
        assert_eq!(
            m,
            crate::timing::MeasuredRate::of(18_000, Some(rate), None).unwrap()
        );
        // 60 s holds too few rate values for a ±, so neither has one.
        assert_eq!(a.summary.confidence.unwrap().rate.se, None);
        assert!(doc["bph_measured_se"].is_null());
        // With a rate ± of 4.8 s/d, the beat rate heard has 4.8 / 86,400
        // of the rate it is read against, 1 bph (Norm on #2303).
        let mut with_pm = a.summary.clone();
        with_pm.confidence.as_mut().unwrap().rate.se = Some(4.8);
        let se = body(&with_pm, None).unwrap()["bph_measured_se"]
            .as_f64()
            .unwrap();
        assert!((se - 18_000.0 * 4.8 / 86_400.0).abs() < 1e-12, "{se}");
        // Corrected for a card 50 ppm fast, the rate reads 50 ppm more
        // (`clockstore::correct_rate`), and so does the beat rate heard.
        let corrected = body(&a.summary, Some((50.0, "typed in"))).unwrap();
        let on_true = corrected["bph_measured"].as_f64().unwrap();
        assert!(
            (on_true / heard - 1.0 - 50e-6).abs() < 1e-8,
            "{on_true} {heard}"
        );
        let corrected_rate = crate::clockstore::correct_rate(rate, 50.0);
        let m50 = crate::timing::MeasuredRate::of(18_000, Some(corrected_rate), None).unwrap();
        assert_eq!(on_true, m50.bph);
    }

    /// No regular beat, no reading: no beat rate heard either.
    #[test]
    fn noise_gives_no_beat_rate_heard() {
        let audio = generate(
            &SynthConfig {
                duration_s: 20.0,
                snr_db: -30.0,
                ..Default::default()
            },
            |_| 280.0,
            |_| 0.0,
        );
        let a = analyze(&audio, &AnalysisConfig::default());
        assert!(!a.summary.watch_heard);
        let doc = body(&a.summary, None).unwrap();
        assert!(doc.get("bph_measured").is_none());
        assert!(doc.get("bph_measured_se").is_none());
    }
}
