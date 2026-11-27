//! `analyze`, `long` and `session` read a take for the escapement its
//! session file or its calibre gives (#2313): on a co-axial, no drop beat
//! error, the escapement named, and the same reading from each.
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tg-coaxial-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(config: &Path, args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .env("TIMEGRAPHER_CONFIG_DIR", config)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap()
}

/// A 21,600 bph watch at 420°, over the 380° gate at the default 52° on
/// both sides, as a co-axial's short unlock-to-drop time reads there.
fn take(tag: &str, session: Option<&str>) -> (PathBuf, PathBuf) {
    let d = dir(tag);
    let wav = d.join("watch.wav");
    let w = wav.to_str().unwrap();
    run(
        &d,
        &[
            "synth",
            w,
            "--duration",
            "40",
            "--bph",
            "21600",
            "--amplitude",
            "420",
        ],
    );
    if let Some(s) = session {
        let toml = format!("{s}\n[[recording]]\nfile = \"watch.wav\"\nposition = \"DU\"\n");
        std::fs::write(d.join("session.toml"), toml).unwrap();
    }
    (d, wav)
}

fn says_coaxial(doc: &Value) {
    assert_eq!(doc["escapement"], "co-axial", "{doc}");
    assert_eq!(doc["input"]["settings"]["escapement"], "co-axial");
    assert!(
        doc["overall"]["beat_error_ms"].is_null(),
        "{}",
        doc["overall"]
    );
    assert_eq!(doc["beat_error_source"], "unlock");
    assert!(doc["beat_error_unlock_ms"].is_number());
}

#[test]
fn analyze_reads_a_take_whose_session_file_says_co_axial_as_one() {
    let (d, wav) = take("file", Some("bph = 21600\nescapement = \"co-axial\""));
    says_coaxial(&json(&run(
        &d,
        &["analyze", wav.to_str().unwrap(), "--json"],
    )));
}

#[test]
fn analyze_reads_a_co_axial_calibre_as_one() {
    let (d, wav) = take("calibre", None);
    let w = wav.to_str().unwrap();
    says_coaxial(&json(&run(
        &d,
        &["analyze", w, "--calibre", "3861", "--json"],
    )));
    // A lever is read as before: the drop's beat error, no escapement.
    let lever = json(&run(
        &d,
        &[
            "analyze",
            w,
            "--calibre",
            "2824-2",
            "--bph",
            "21600",
            "--json",
        ],
    ));
    assert!(lever.get("escapement").is_none());
    assert!(lever["input"]["settings"].get("escapement").is_none());
    assert!(lever["overall"]["beat_error_ms"].is_number());
    assert_eq!(lever["beat_error_source"], "drop");
}

/// A custom calibre the user set to co-axial is read as one, as a table
/// row is (#2387), and `calibres NAME` says how; one left a Swiss lever
/// is read as a lever.
#[test]
fn analyze_reads_a_custom_co_axial_calibre_as_one() {
    let (d, wav) = take("custom", None);
    let custom = serde_json::json!({
        "schema": "timegrapher.user-calibres/1",
        "custom": [
            {"name": "Bench CA", "bph": 21600, "wheels": [],
             "escapement": {"kind": "co-axial", "source": "Added by you"}},
            {"name": "Bench SL", "bph": 21600, "wheels": []},
        ],
    });
    std::fs::write(d.join("calibres.json"), custom.to_string()).unwrap();
    let w = wav.to_str().unwrap();
    says_coaxial(&json(&run(
        &d,
        &["analyze", w, "--calibre", "Bench CA", "--json"],
    )));
    let lever = json(&run(&d, &["analyze", w, "--calibre", "Bench SL", "--json"]));
    assert!(lever.get("escapement").is_none());
    assert_eq!(lever["beat_error_source"], "drop");
    let shown = run(&d, &["calibres", "Bench CA"]);
    let text = String::from_utf8_lossy(&shown.stdout);
    let sentence = timegrapher_core::calibres::COAXIAL_SENTENCE;
    assert!(text.lines().any(|l| l == sentence), "{text}");
    let listed = json(&run(&d, &["calibres", "Bench CA", "--json"]));
    assert_eq!(listed["custom"][0]["escapement"]["kind"], "co-axial");
}

#[test]
fn long_reads_a_co_axial_take_as_one() {
    let (d, _) = take("long", Some("bph = 21600\nescapement = \"co-axial\""));
    let dir = d.to_str().unwrap();
    let out = d.join("out");
    let o = out.to_str().unwrap();
    let doc = json(&run(&d, &["long", dir, "--out", o, "--json"]));
    assert_eq!(doc["escapement"], "co-axial", "{doc}");
    assert!(doc["overall"]["rate_s_per_day"].is_number(), "{doc}");
    assert!(
        doc["overall"]["beat_error_ms"].is_null(),
        "{}",
        doc["overall"]
    );
    assert!(doc["beat_error_unlock_ms"].is_number(), "{doc}");
    // Each slice's drop beat error is empty in its table, its unlock's given.
    let slices = std::fs::read_to_string(out.join("slices.csv")).unwrap();
    let rows: Vec<Vec<&str>> = slices
        .lines()
        .skip(1)
        .map(|l| l.split(',').collect())
        .collect();
    assert!(!rows.is_empty(), "{slices}");
    assert!(
        rows.iter().all(|r| r[3].is_empty() || r[3] == "-"),
        "{slices}"
    );
    assert!(rows.iter().any(|r| r[4].parse::<f64>().is_ok()), "{slices}");
    assert!(!doc.to_string().contains("beat_error_from_drop"), "{doc}");
    // The text names no drop figure.
    let text = String::from_utf8(run(&d, &["long", dir]).stdout).unwrap();
    let line = text
        .lines()
        .find(|l| l.starts_with("Beat error"))
        .expect(&text);
    assert!(line.contains("from the unlock"), "{line}");
    assert!(!line.contains("drop"), "{line}");
}
