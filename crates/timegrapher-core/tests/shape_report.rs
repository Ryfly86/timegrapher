//! `analysis::shape_report`: the shape of the beats `analyze` read, on
//! the envelope it was asked for and on the beats' fitted period (#2104).
use timegrapher_core::analysis::{self, AnalysisConfig};
use timegrapher_core::dsp::envelope;
use timegrapher_core::shape::{self, ShapeConfig, ShapeReport};
use timegrapher_core::synth::{generate, SynthConfig};
use timegrapher_core::timing;

/// The noise-to-silence ratios of each side, windowed and whole: the
/// figures that rest on the beat period.
fn noise(r: &ShapeReport) -> [Option<f64>; 4] {
    let whole = |s: &shape::SideReport| s.whole.as_ref().and_then(|w| w.noise_ratio);
    [
        r.even.windows.noise_ratio,
        r.odd.windows.noise_ratio,
        whole(&r.even),
        whole(&r.odd),
    ]
}

#[test]
fn the_shape_is_read_on_the_envelope_asked_for_and_the_fitted_period() {
    // 300 s/d fast, so the fitted beat period is 0.35% under 3600/bph.
    let audio = generate(
        &SynthConfig {
            duration_s: 20.0,
            rate_s_per_day: 300.0,
            ..Default::default()
        },
        |_| 275.0,
        |_| 0.0,
    );
    let fs = audio.sample_rate as f64;
    let mut cfg = AnalysisConfig {
        bph: Some(28800),
        ..Default::default()
    };
    // Not the default envelope, so a read on the default one differs.
    cfg.envelope.highpass_hz = 2500.0;
    let a = analysis::analyze(&audio, &cfg);
    let got = analysis::shape_report(&audio, &cfg, &a);

    let env = envelope(&audio.samples, fs, &cfg.envelope);
    let period = timing::beat_period_s(a.summary.overall.as_ref(), a.summary.bph);
    let on = |env: &[f32], period: f64| {
        shape::analyze(env, fs, &a.beats, period, &ShapeConfig::default())
    };
    let want = on(&env, period);
    assert!(noise(&want).iter().all(Option::is_some), "{want:?}");
    assert_eq!(noise(&got), noise(&want));
    assert_eq!(got.even.windows.i13_ms, want.even.windows.i13_ms);
    assert_eq!(got.odd.windows.i13_ms, want.odd.windows.i13_ms);

    // Either input read otherwise gives other figures, so each counts.
    let nominal = on(&env, 3600.0 / a.summary.bph as f64);
    assert_ne!(noise(&nominal), noise(&want), "the period moves no figure");
    let default_env = envelope(&audio.samples, fs, &Default::default());
    let plain = on(&default_env, period);
    assert_ne!(
        plain.even.windows.i13_ms, want.even.windows.i13_ms,
        "the envelope moves no figure"
    );
}
