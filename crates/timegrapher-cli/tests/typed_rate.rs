//! A typed beat rate (#2224): any whole rate from 12,000 to 72,000 bph is
//! read against as set, outside that range it is refused with the range,
//! and every document says where the set rate came from (`bph_set_from`).
use std::{path::Path, path::PathBuf, process::Command};

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("tg-typed-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .env("TIMEGRAPHER_CONFIG_DIR", std::env::temp_dir())
        .args(args)
        .output()
        .unwrap()
}

fn json(args: &[&str]) -> serde_json::Value {
    let out = run(args);
    assert!(out.status.success(), "{args:?}: {out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}

/// A 40 s synthetic balance at 17,570 bph, on time.
fn balance(dir: &Path) -> String {
    let wav = dir.join("take.wav").to_string_lossy().into_owned();
    let out = run(&["synth", &wav, "--duration=40", "--bph=17570", "--rate=0"]);
    assert!(out.status.success(), "{out:?}");
    wav
}

#[test]
fn a_typed_rate_is_read_against_as_set() {
    let dir = Dir::new("read");
    let wav = balance(&dir.0);
    let at = |bph: &str| json(&["analyze", &wav, &format!("--bph={bph}"), "--json"]);
    let exact = at("17570");
    let one_up = at("17571");
    // (17570 / 17571 − 1) × 86,400 = −4.9 s/d; the amplitude does not move.
    let rate = |d: &serde_json::Value| d["rate_s_per_day"].as_f64().unwrap();
    assert!(rate(&exact).abs() < 0.05, "{}", rate(&exact));
    assert!((rate(&one_up) + 4.917).abs() < 0.05, "{}", rate(&one_up));
    assert_eq!(exact["amplitude_deg"], one_up["amplitude_deg"]);
    assert_eq!(exact["bph"], 17570);
    assert_eq!(exact["bph_set"], 17570);
    assert_eq!(exact["bph_set_from"], "typed");
    assert!(exact["caveats"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["code"] != "off_standard_rate"));
    // A standard rate given says so; on Auto there is none to say.
    assert_eq!(at("18000")["bph_set_from"], "standard");
    let auto = json(&["analyze", &wav, "--json"]);
    assert!(auto["bph_set"].is_null());
    assert!(auto.get("bph_set_from").is_none(), "{auto}");
}

#[test]
fn a_rate_outside_the_range_or_with_a_decimal_is_refused() {
    let dir = Dir::new("refuse");
    let wav = dir.0.join("never-read.wav").to_string_lossy().into_owned();
    for bph in ["11999", "72001", "0", "8000", "17570.5", "17570.0"] {
        let out = run(&["analyze", &wav, &format!("--bph={bph}")]);
        assert_eq!(out.status.code(), Some(2), "{bph}: {out:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("the beat rate must be a whole number from 12000 to 72000 bph"),
            "{bph}: {err}"
        );
    }
    for bph in ["12000", "72000"] {
        let out = run(&["analyze", &wav, &format!("--bph={bph}")]);
        // Past the flag, to the missing file.
        assert_eq!(out.status.code(), Some(1), "{bph}: {out:?}");
    }
}

#[test]
fn a_session_file_says_its_rate_is_the_sessions() {
    let dir = Dir::new("session");
    balance(&dir.0);
    let manifest = dir.0.join("session.toml");
    std::fs::write(
        &manifest,
        "bph = 17570\nlift = 52\n\n[[recording]]\nfile = \"take.wav\"\nposition = \"DU\"\n",
    )
    .unwrap();
    let take = dir.0.to_string_lossy().into_owned();
    let doc = json(&["session", &take, "--json"]);
    let r = &doc["readings"][0];
    assert_eq!(r["bph_set"], 17570);
    assert_eq!(r["bph_set_from"], "session");
    assert!(r["measurement"]["rate_s_per_day"].as_f64().unwrap().abs() < 0.05);
    // Given on the command line too, it is the flag's (the two agree).
    let doc = json(&["session", &take, "--bph=17570", "--json"]);
    assert_eq!(doc["readings"][0]["bph_set_from"], "typed");
}

#[test]
fn a_session_file_outside_the_range_is_refused_and_left_as_it_is() {
    let dir = Dir::new("session-out");
    balance(&dir.0);
    let manifest = dir.0.join("session.toml");
    let text = "bph = 10800\nlift = 52\n\n[[recording]]\nfile = \"take.wav\"\nposition = \"DU\"\n";
    std::fs::write(&manifest, text).unwrap();
    let take = dir.0.to_string_lossy().into_owned();
    let out = run(&["session", &take, "--json"]);
    assert!(!out.status.success(), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("session.toml"), "{err}");
    assert!(err.contains("bph"), "{err}");
    assert!(err.contains("from 12000 to 72000"), "{err}");
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text);
    // `analyze` of a file it lists says the same beside the file's own
    // reading, or refuses: either way, never at 10,800.
    let wav = dir.0.join("take.wav").to_string_lossy().into_owned();
    let out = run(&["analyze", &wav, "--json"]);
    if out.status.success() {
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_ne!(doc["bph_set"], 10800, "{doc}");
    }
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text);
}
