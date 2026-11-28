//! Regression integrity through the executable, without audio or capture.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str, takes: Value) -> Self {
        let root = std::env::temp_dir().join(format!("tg-regress-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("baseline.json"),
            serde_json::to_vec(&json!({
                "schema": "timegrapher.regress-baseline/1", "software": {}, "takes": takes,
            }))
            .unwrap(),
        )
        .unwrap();
        Self(root)
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_timegrapher"));
        cmd.arg("regress")
            .arg(&self.0)
            .arg("--baseline")
            .arg(self.0.join("baseline.json"))
            .args(["--json", "--quiet", "--jobs", "1"]);
        cmd
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unreadable_new_take_exits_three_and_reports_the_missing_evidence() {
    let f = Fixture::new("new", json!({}));
    std::fs::create_dir_all(f.0.join("new")).unwrap();
    std::fs::write(
        f.0.join("new/session.toml"),
        "[[recording]]\nfile = \"missing.flac\"\nposition = \"DU\"\n",
    )
    .unwrap();
    let out = f.command().output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["passed"], false);
    assert_eq!(doc["unreadable"], json!(["new #1"]));
    assert_eq!(doc["counts"]["new"], 5); // New numerical rows cannot mask failure.

    let before = std::fs::read(f.0.join("baseline.json")).unwrap();
    let out = f.command().arg("--write-baseline").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no baseline was written"));
    assert_eq!(std::fs::read(f.0.join("baseline.json")).unwrap(), before);
}

#[test]
fn missing_required_session_fails_but_an_explicit_exclusion_does_not() {
    let f = Fixture::new("missing", json!({"removed/session.toml": []}));
    std::fs::create_dir_all(f.0.join("takes")).unwrap();
    std::fs::write(f.0.join("corpus.json"), "{\"takes\": []}").unwrap();
    // A smoke run can have no referenced sessions, but required baseline
    // sessions still have to be present.
    let out = f.command().arg("--smoke").output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["passed"], false);
    assert_eq!(doc["missing_sessions"], json!(["removed"]));
    let before = std::fs::read(f.0.join("baseline.json")).unwrap();
    let out = f
        .command()
        .args(["--smoke", "--write-baseline"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(std::fs::read(f.0.join("baseline.json")).unwrap(), before);

    let out = f
        .command()
        .args(["--smoke", "--take", "other"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["passed"], true);
    assert_eq!(doc["missing_sessions"], json!([]));
    assert_eq!(doc["not_run"], json!(["removed"]));
}

#[test]
fn an_unreadable_existing_baseline_is_not_replaced() {
    let f = Fixture::new("invalid-utf8", json!({}));
    std::fs::create_dir_all(f.0.join("takes")).unwrap();
    std::fs::write(f.0.join("corpus.json"), "{\"takes\": []}").unwrap();
    let baseline = f.0.join("baseline.json");
    let original = b"existing baseline with invalid UTF-8: \xff";
    std::fs::write(&baseline, original).unwrap();
    let out = f
        .command()
        .args(["--smoke", "--write-baseline"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("baseline.json"));
    assert_eq!(std::fs::read(&baseline).unwrap(), original);

    // Holdout still ignores a baseline read error and never rewrites it.
    let out = f.command().arg("--holdout").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(std::fs::read(&baseline).unwrap(), original);

    // The same run can create a genuinely absent baseline.
    std::fs::remove_file(&baseline).unwrap();
    let out = f
        .command()
        .args(["--smoke", "--write-baseline"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let created: Value = serde_json::from_slice(&std::fs::read(&baseline).unwrap()).unwrap();
    assert_eq!(created["schema"], "timegrapher.regress-baseline/1");
}

// A 1-second PCM silence file exercises metadata plumbing, not acoustic accuracy.
fn silence_wav() -> Vec<u8> {
    let frames = 48_000u32;
    let size = frames * 2;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((36 + size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes()); // PCM
    bytes.extend(1u16.to_le_bytes()); // mono
    bytes.extend(48_000u32.to_le_bytes());
    bytes.extend(96_000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    bytes.resize(44 + size as usize, 0);
    bytes
}

#[test]
fn selected_write_captures_actual_context_and_preserves_other_entry_origins() {
    let unknown = json!({ "label":"old.wav", "rate_s_per_day":1.23456789012345,
        "amplitude_deg":270.123456789, "beat_error_unlock_ms":-0.123456789,
        "beat_error_ms":0.23456789, "beats":12345 });
    let mut attributed = unknown.clone();
    attributed["provenance"] = json!({
        "schema":"timegrapher.regress-entry-provenance/1",
        "software":{"git_hash":"old-entry-engine"}, "manifest":{"sha256":"old-manifest"},
        "inputs":[], "settings":{}, "measurement_context":{}, "comparator":{},
        "future_annotation":{"preserve":"unselected-extension"},
    });
    let f = Fixture::new(
        "provenance",
        json!({
            "legacy/session.toml":[unknown.clone()],
            "attributed/session.toml":[attributed.clone()],
        }),
    );
    let dir = f.0.join("takes/selected");
    std::fs::create_dir_all(&dir).unwrap();
    let wav = silence_wav();
    let digest = timegrapher_core::archive::sha256_hex(&wav);
    std::fs::write(dir.join("silence.wav"), &wav).unwrap();
    let manifest = "bph=28800\nlift=49.5\nsettle_s=0\ncard_ppm_fast=0\n[[recording]]\nfile='silence.wav'\nposition='DU'\nstart_s=0.125\nend_s=0.875\nreference={source='historical tg label, executable unknown'}\n";
    std::fs::write(dir.join("session.toml"), manifest).unwrap();
    std::fs::write(
        f.0.join("corpus.json"),
        serde_json::to_vec(&json!({"takes":[{
            "id":"selected", "tier":"referenced", "release":"unused-local-fixture",
            "files":[{"name":"silence.wav","bytes":wav.len(),"sha256":digest}],
        }]}))
        .unwrap(),
    )
    .unwrap();
    let out = f
        .command()
        .args(["--take", "selected", "--write-baseline", "--fetch"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let baseline: Value =
        serde_json::from_slice(&std::fs::read(f.0.join("baseline.json")).unwrap()).unwrap();
    assert_eq!(baseline["takes"]["attributed/session.toml"][0], attributed);
    let mut explicit_unknown = unknown.clone();
    explicit_unknown["provenance"] = Value::Null;
    assert_eq!(
        baseline["takes"]["legacy/session.toml"][0],
        explicit_unknown
    );
    let p = &baseline["takes"]["selected/session.toml"][0]["provenance"];
    assert_eq!(
        p["manifest"]["sha256"],
        timegrapher_core::archive::sha256_hex(manifest.as_bytes())
    );
    assert_eq!(p["manifest"]["recording_index_1based"], 1);
    assert_eq!(p["manifest"]["recording"]["start_s"], 0.125);
    assert_eq!(p["settings"]["requested_bph"], 28800);
    assert_eq!(p["settings"]["lift_deg"], 49.5);
    assert_eq!(p["measurement_context"]["measured_start_s"], 0.125);
    assert_eq!(p["measurement_context"]["measured_end_s"], 0.875);
    assert_eq!(p["measurement_context"]["detected_bph"], 28800);
    assert_eq!(p["inputs"][0]["verified_identity"]["sha256"], digest);
    assert_eq!(p["inputs"][0]["verified_identity"]["bytes"], wav.len());
    assert_eq!(
        p["inputs"][0]["verified_identity"]["timing"],
        "verified_at_fetch_this_run"
    );
    assert!(p["comparator"]["executable"].is_null());
    assert_eq!(
        p["comparator"]["reference_as_declared"]["source"],
        "historical tg label, executable unknown"
    );

    // A later write without fetch must not carry forward a previous hash check.
    let out = f
        .command()
        .args(["--take", "selected", "--write-baseline"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let next: Value =
        serde_json::from_slice(&std::fs::read(f.0.join("baseline.json")).unwrap()).unwrap();
    let p = &next["takes"]["selected/session.toml"][0]["provenance"];
    assert!(p["inputs"][0]["verified_identity"].is_null());
    assert_eq!(p["inputs"][0]["declared_corpus_identity"]["sha256"], digest);
    for field in [
        "rate_s_per_day",
        "amplitude_deg",
        "beat_error_unlock_ms",
        "beat_error_ms",
        "beats",
    ] {
        assert_eq!(
            next["takes"]["selected/session.toml"][0][field],
            baseline["takes"]["selected/session.toml"][0][field]
        );
    }
}

// #2421: a holdout take declared in corpus.json with no session file fails
// the run; it is not passed off as "No holdout takes".
#[test]
fn a_declared_holdout_take_with_no_session_file_fails_the_holdout_run() {
    let f = Fixture::new("holdout-missing", json!({}));
    std::fs::create_dir_all(f.0.join("takes/sealed")).unwrap();
    std::fs::write(
        f.0.join("corpus.json"),
        serde_json::to_vec(&json!({"takes":[{
            "id":"sealed", "tier":"holdout", "release":"unused", "files":[],
        }]}))
        .unwrap(),
    )
    .unwrap();
    let out = f.command().arg("--holdout").output().unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["takes"], 0);
    assert_eq!(doc["missing"], 1);
    // Counts only: the take is named only with --reveal.
    assert!(doc.get("missing_takes").is_none(), "{doc}");
    let out = f
        .command()
        .args(["--holdout", "--reveal"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["missing_takes"], json!(["sealed"]));

    // The printout says so instead of "No holdout takes in this corpus."
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_timegrapher"));
    let out = cmd
        .arg("regress")
        .arg(&f.0)
        .args(["--holdout", "--quiet"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("1 declared holdout take has no session file"),
        "{text}"
    );
    assert!(!text.contains("No holdout takes"), "{text}");

    // --take leaves it out of this run, so it is not missing from it.
    let out = f
        .command()
        .args(["--holdout", "--take", "other"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["missing"], 0);
}

// #2421: the holdout run never reads the development baseline, so a
// malformed one cannot fail it.
#[test]
fn a_malformed_development_baseline_does_not_fail_the_holdout_run() {
    let f = Fixture::new("holdout-baseline", json!({}));
    std::fs::create_dir_all(f.0.join("takes")).unwrap();
    std::fs::write(f.0.join("corpus.json"), "{\"takes\": []}").unwrap();
    let baseline = f.0.join("baseline.json");
    std::fs::write(&baseline, "{ not json").unwrap();
    let out = f.command().arg("--holdout").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(std::fs::read(&baseline).unwrap(), b"{ not json");
    // A wrong layout is not read either.
    std::fs::write(
        &baseline,
        "{\"schema\":\"other/9\",\"software\":{},\"takes\":{}}",
    )
    .unwrap();
    let out = f.command().arg("--holdout").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    // The development run still refuses it.
    let out = f.command().arg("--smoke").output().unwrap();
    assert!(!out.status.success(), "{out:?}");
}

// #2421: the holdout output names the corpus commit it read.
#[test]
fn the_holdout_run_names_the_corpus_revision() {
    let f = Fixture::new("holdout-revision", json!({}));
    std::fs::create_dir_all(f.0.join("takes")).unwrap();
    std::fs::write(f.0.join("corpus.json"), "{\"takes\": []}").unwrap();
    let doc = |f: &Fixture| -> Value {
        let out = f.command().arg("--holdout").output().unwrap();
        assert!(out.status.success(), "{out:?}");
        serde_json::from_slice(&out.stdout).unwrap()
    };
    assert_eq!(doc(&f)["corpus_revision"], Value::Null);
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(&f.0)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    git(&["init", "-q"]);
    git(&["add", "corpus.json"]);
    git(&["commit", "-q", "-m", "corpus"]);
    let head = git(&["rev-parse", "HEAD"]);
    assert_eq!(doc(&f)["corpus_revision"], json!(head));
}
