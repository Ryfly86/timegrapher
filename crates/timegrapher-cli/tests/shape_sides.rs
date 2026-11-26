//! `timegrapher shape` reads the recording as `analyze` does: the same
//! beats with the same sides, so its Tick and Tock are analyze's Tick and
//! Tock, and each side's amplitude is analyze's (#2104).
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use timegrapher_core::synth::{generate, SynthConfig};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tg-shape-sides-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The command's stdout, run with a settings folder of its own.
fn run(config: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(args)
        .env("TIMEGRAPHER_CONFIG_DIR", config)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn json(config: &Path, args: &[&str]) -> Value {
    let text = run(config, args);
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{args:?}: {e}\n{text}"))
}

/// A watch whose two sides sound apart: the even beats' unlock comes
/// 0.3 ms earlier (a larger unlock-to-drop time, so a smaller amplitude)
/// and the odd beats' impulse sits later, with a beat error of
/// `beat_error_ms` (its sign sets which side the engine names Tick).
fn two_sided_wav(path: &Path, beat_error_ms: f64) {
    let audio = generate(
        &SynthConfig {
            duration_s: 20.0,
            beat_error_ms,
            even_unlock_lead_s: 0.0003,
            odd_impulse_at: Some(0.6),
            ..Default::default()
        },
        |_| 275.0,
        |_| 0.0,
    );
    timegrapher_core::audio::write_wav(path, &audio).unwrap();
}

/// The figure after `word ` in `line`, as a whole number.
fn after(line: &str, word: &str) -> i64 {
    let at = line
        .find(word)
        .unwrap_or_else(|| panic!("{word} in {line}"))
        + word.len();
    let digits: String = line[at..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("{word} in {line}"))
}

#[test]
fn shape_names_the_sides_and_reads_the_amplitudes_as_analyze_does() {
    let dir = scratch("sides");
    let config = dir.join("config");
    // Both signs of beat error, so one of the two takes has its Tick on
    // the odd beats of a plain beat count.
    for (k, be) in [0.4, -0.4].into_iter().enumerate() {
        let wav = dir.join(format!("watch{k}.wav"));
        two_sided_wav(&wav, be);
        let w = wav.to_str().unwrap();
        let a = json(&config, &["analyze", w, "--json"]);
        let s = json(&config, &["shape", w, "--json"]);
        for side in ["even", "odd"] {
            let of_analyze = a["shape"][side]["i12_ms"].as_f64();
            let of_shape = s[side]["windows"]["i12_ms"].as_f64();
            assert!(of_analyze.is_some(), "{a}");
            assert_eq!(of_shape, of_analyze, "{side} at {be} ms");
        }
        // The sides' amplitudes, as analyze words them and as shape's
        // table gives them.
        let text = run(&config, &["analyze", w]);
        let line = text.lines().find(|l| l.starts_with("Amplitude")).unwrap();
        let (tick, tock) = (after(line, "Tick"), after(line, "Tock"));
        assert_ne!(tick, tock, "the sides should differ: {line}");
        let table = run(&config, &["shape", w]);
        let row = table
            .lines()
            .find(|l| l.starts_with("amplitude, deg"))
            .unwrap_or_else(|| panic!("{table}"));
        let cells: Vec<&str> = row["amplitude, deg".len()..].split_whitespace().collect();
        assert_eq!(
            cells,
            [tick.to_string(), tock.to_string(), "-".into(), "-".into()],
            "{line}\n{row}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
