//! Machine-readable output: every `--json` document has the same
//! envelope, so an agent can tell what it is reading and where it came from.
//!
//! The `schema` field names the kind of document and its version
//! (`timegrapher.analyze/3`). Within a version, fields are only ever added;
//! a change that would break a reader bumps the version. See
//! `docs/reference/agent-interface.md`.

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Version of the document kinds that have not changed since the first.
pub const SCHEMA_VERSION: u32 = 1;

/// Current version of a document kind. `record/2` keeps every take and
/// drops the verdict; `series/2` dropped the one-word verdict, the
/// findings' severity and advice, and `moves_with`, and `series/3` the
/// isochronism's quadratic (`curve_centre_deg`, `curve`, `curve_explained`,
/// `turn_deg`), and `series/4` left the wind not given unless set
/// (`wind_h` null, never full wind's); `session/2` dropped
/// the tolerances, limits, verdicts and the findings' severity and advice,
/// and `session/3` made a state of wind not given its own (`wind_h` null)
/// instead of full wind's;
/// `analyze/2`, `doctor/2` and `regress-holdout/2` dropped their grades,
/// verdicts, named faults and advice, and `analyze/3` is core's
/// ([`timegrapher_core::analyze_doc::VERSION`]).
pub fn schema_version(kind: &str) -> u32 {
    match kind {
        "record" => 2,
        "series" => 4,
        "session" => 3,
        "analyze" => timegrapher_core::analyze_doc::VERSION,
        "doctor" => 2,
        "regress-holdout" => 2,
        _ => SCHEMA_VERSION,
    }
}

#[derive(Serialize)]
pub struct Envelope<'a, T: Serialize> {
    pub schema: String,
    pub software: Value,
    pub input: Value,
    #[serde(flatten)]
    pub body: &'a T,
}

/// Name, version, build and platform. `git_hash` is the short commit hash
/// ("unknown" in a build without git), `git_dirty` whether the work tree had
/// uncommitted changes (null when unknown), `build_date` the UTC date of the
/// build. These tell builds apart; they are not readings, and anything that
/// compares documents should leave this block out.
pub fn software() -> Value {
    let dirty = match env!("TIMEGRAPHER_GIT_DIRTY") {
        "true" => json!(true),
        "false" => json!(false),
        _ => Value::Null,
    };
    json!({
        "name": "timegrapher",
        "version": env!("CARGO_PKG_VERSION"),
        "git_hash": env!("TIMEGRAPHER_GIT_HASH"),
        "git_dirty": dirty,
        "build_date": env!("TIMEGRAPHER_BUILD_DATE"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    })
}

/// The JSON text of a document of `kind`.
pub fn to_json<T: Serialize>(kind: &str, input: Value, body: &T) -> Result<String, String> {
    serde_json::to_string_pretty(&Envelope {
        schema: format!("timegrapher.{kind}/{}", schema_version(kind)),
        software: software(),
        input,
        body,
    })
    .map_err(|e| e.to_string())
}

pub fn print<T: Serialize>(kind: &str, input: Value, body: &T) -> Result<(), String> {
    println!("{}", to_json(kind, input, body)?);
    Ok(())
}

/// Where the provenance of a recording is kept: `take.wav` -> `take.wav.json`.
pub fn sidecar_path(file: &Path) -> PathBuf {
    timegrapher_core::clockstore::sidecar_path(file)
}

/// Describe the input files and the settings used. A recording made by
/// `timegrapher doctor --save` (or anything else that writes the same
/// sidecar) brings its device and mixer settings along.
pub fn input(files: &[PathBuf], settings: Value) -> Value {
    let files: Vec<Value> = files
        .iter()
        .map(|f| {
            let mut v = json!({ "path": f.display().to_string() });
            if let Some(info) = std::fs::read_to_string(sidecar_path(f))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            {
                v["recording"] = info.get("recording").cloned().unwrap_or(info);
            }
            v
        })
        .collect();
    json!({ "files": files, "settings": settings })
}

/// The file a path names once written: the canonical path of an existing
/// file, or of its folder joined to its name for one not yet written.
fn resolved(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let parent = match p.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    match (parent.canonicalize(), p.file_name()) {
        (Ok(d), Some(name)) => d.join(name),
        _ => p.to_path_buf(),
    }
}

/// Whether `a` and `b` name one file: the same resolved path, or one
/// folder and names that differ only in case, as on a disk that ignores
/// case (macOS, Windows), where they are one file.
fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || (a.parent() == b.parent()
            && match (a.file_name(), b.file_name()) {
                (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
                _ => false,
            })
}

/// Refuse output files that are the input being read, a file of the take
/// it is read from, or each other, before anything is read or written:
/// `analyze x.wav --beats x.wav` would put a CSV where the recording was
/// (#1876), and `--beats TAKE/session.toml` where the take's calibre, lift
/// and positions were (#2372). The recording read is the FLAC when its WAV
/// was kept as one (core's `audio::kept_path`). The take's files are every
/// recording in the folder and its session file, record and clock logs
/// (core's `take` names), refused whether or not they are there yet: a CSV
/// named as one would be read as the take's.
pub fn files_apart(input: &Path, outputs: &[(&str, &Path)]) -> Result<(), String> {
    use timegrapher_core::take::{CLOCK_LOG_NAMES, MANIFEST, RECORD};
    let read = resolved(&timegrapher_core::audio::kept_path(input).unwrap_or(input.to_path_buf()));
    let dir = read.parent().map(Path::to_path_buf);
    let sounds: Vec<PathBuf> = dir
        .as_deref()
        .and_then(|d| timegrapher_core::take_path::recordings_in(d).ok())
        .unwrap_or_default()
        .iter()
        .map(|f| resolved(f))
        .collect();
    let mut seen: Vec<(&str, PathBuf)> = Vec::new();
    for &(flag, p) in outputs {
        let r = resolved(p);
        if same_file(&r, &read) {
            return Err(format!(
                "{flag} {} is the recording being read; name another file",
                p.display()
            ));
        }
        if let Some(name) = dir.as_deref().and_then(|d| {
            [MANIFEST, RECORD]
                .into_iter()
                .chain(CLOCK_LOG_NAMES)
                .find(|n| same_file(&r, &d.join(n)))
        }) {
            return Err(format!(
                "{flag} {} is the take's {name}; name another file",
                p.display()
            ));
        }
        if sounds.iter().any(|s| same_file(&r, s)) {
            return Err(format!(
                "{flag} {} is a recording of the take being read; name another file",
                p.display()
            ));
        }
        if let Some((other, _)) = seen.iter().find(|(_, s)| same_file(s, &r)) {
            return Err(format!(
                "{flag} and {other} both name {}; give each its own file",
                p.display()
            ));
        }
        seen.push((flag, r));
    }
    Ok(())
}

#[cfg(test)]
mod files_apart_tests {
    use super::files_apart;
    use std::path::Path;

    #[test]
    fn an_output_may_not_be_the_input_or_another_output() {
        let dir = std::env::temp_dir().join(format!("tg-apart-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("x.wav");
        std::fs::write(&wav, b"RIFF").unwrap();
        let beats = dir.join("beats.csv");
        let same = dir.join(".").join("x.wav");
        assert!(files_apart(&wav, &[("--beats", &beats)]).is_ok());
        let e = files_apart(&wav, &[("--beats", &same)]).unwrap_err();
        assert!(
            e.contains("--beats") && e.contains("recording being read"),
            "{e}"
        );
        let e = files_apart(&wav, &[("--beats", &beats), ("--windows", &beats)]).unwrap_err();
        assert!(e.contains("--windows and --beats"), "{e}");
        assert!(files_apart(Path::new("x.wav"), &[("--trend", Path::new("t.csv"))]).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A take's own files are refused as outputs, there or not yet, in any
    /// case, and so is the FLAC read for a WAV kept as one (#2372).
    #[test]
    fn an_output_may_not_be_a_file_of_the_take() {
        let dir = std::env::temp_dir().join(format!("tg-apart-take-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("take_part01-001.wav");
        std::fs::write(&wav, b"RIFF").unwrap();
        std::fs::write(dir.join("take_part01-002.wav"), b"RIFF").unwrap();
        std::fs::write(dir.join("session.toml"), b"").unwrap();
        for (name, says) in [
            ("session.toml", "take's session.toml"),
            ("Session.TOML", "take's session.toml"),
            ("session.json", "take's session.json"),
            ("clock.csv", "take's clock.csv"),
            ("clocklog.txt", "take's clocklog.txt"),
            ("clock.log", "take's clock.log"),
            ("take_part01-002.wav", "recording of the take"),
            ("TAKE_PART01-001.WAV", "recording being read"),
        ] {
            let e = files_apart(&wav, &[("--windows", &dir.join(name))]).unwrap_err();
            assert!(e.contains("--windows") && e.contains(says), "{name}: {e}");
        }
        assert!(files_apart(&wav, &[("--beats", &dir.join("beats.csv"))]).is_ok());
        // The WAV kept as FLAC: the FLAC is what is read.
        let gone = dir.join("take_part02-001.wav");
        let flac = dir.join("take_part02-001.flac");
        std::fs::write(&flac, b"fLaC").unwrap();
        let e = files_apart(&gone, &[("--beats", &flac)]).unwrap_err();
        assert!(e.contains("recording being read"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
