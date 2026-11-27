//! A take recorded on Auto is read on Auto (Peter, 26 Nov 2026, on
//! #2357): its recorder's `session.json` writes `bph` null, so the rate
//! its `session.toml` keeps is the one Auto guessed. `session` and
//! `analyze` read it on Auto and say so, and `--bph` still sets the rate.
use std::{path::Path, path::PathBuf, process::Command};

struct Dir(PathBuf);
impl Dir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("tg-on-auto-{name}-{}", std::process::id()));
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

fn json(args: &[&str]) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .env("TIMEGRAPHER_CONFIG_DIR", std::env::temp_dir())
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}

/// A take folder: 40 s of a balance at 17,570 bph, a session file that
/// keeps 17,280, and a recorder's `session.json` with `bph` (JSON).
fn take(dir: &Path, bph: &str) -> (String, String) {
    let wav = dir.join("take.wav").to_string_lossy().into_owned();
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(["synth", &wav, "--duration=40", "--bph=17570", "--rate=0"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    std::fs::write(
        dir.join("session.toml"),
        "bph = 17280\n\n[[recording]]\nfile = \"take.wav\"\nposition = \"DU\"\n",
    )
    .unwrap();
    let record = format!(r#"{{"watch": "", "position": "DU", "lift_deg": 52.0, "bph": {bph}}}"#);
    std::fs::write(dir.join("session.json"), record).unwrap();
    (dir.to_string_lossy().into_owned(), wav)
}

#[test]
fn a_take_recorded_on_auto_is_read_and_said_on_auto() {
    let dir = Dir::new("auto");
    let (take, wav) = take(&dir.0, "null");
    let doc = json(&["session", &take, "--json"]);
    assert!(doc["input"]["settings"]["bph"].is_null(), "{doc}");
    let r = &doc["readings"][0];
    assert!(r["bph_set"].is_null(), "{r}");
    assert!(r["rate_s_per_day"].is_null(), "{r}");
    let a = json(&["analyze", &wav, "--json"]);
    assert!(a["bph_set"].is_null(), "{a}");
    assert!(a["rate_s_per_day"].is_null(), "{a}");
    let out = dir.0.join("long-out").to_string_lossy().into_owned();
    let long = json(&["long", &take, "--out", &out, "--json"]);
    assert!(long["input"]["settings"]["bph"].is_null(), "{long}");
    // `--bph` sets the rate.
    let doc = json(&["session", &take, "--bph=18000", "--json"]);
    assert_eq!(doc["input"]["settings"]["bph"], 18000);
    assert_eq!(doc["readings"][0]["bph_set"], 18000);
}

#[test]
fn a_take_recorded_at_a_set_rate_is_read_at_it() {
    let dir = Dir::new("set");
    let (take, wav) = take(&dir.0, "17280");
    let doc = json(&["session", &take, "--json"]);
    assert_eq!(doc["input"]["settings"]["bph"], 17280);
    assert_eq!(doc["readings"][0]["bph_set"], 17280);
    assert_eq!(json(&["analyze", &wav, "--json"])["bph_set"], 17280);
    let out = dir.0.join("long-out").to_string_lossy().into_owned();
    let long = json(&["long", &take, "--out", &out, "--json"]);
    assert_eq!(long["input"]["settings"]["bph"], 17280, "{long}");
}
