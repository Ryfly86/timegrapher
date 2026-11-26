//! `analyze` never writes over the recording it reads (#1876), and reads
//! a take folder as the recording in it (#2092).
use std::{path::PathBuf, process::Command};

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tg(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn an_output_naming_the_recording_is_refused_and_the_recording_kept() {
    let dir = Dir(std::env::temp_dir().join(format!("tg-outputs-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let wav = dir.0.join("x.wav");
    let made = tg(&["synth".as_ref(), wav.as_os_str(), "--duration=10".as_ref()]);
    assert!(made.status.success(), "{made:?}");
    let before = std::fs::read(&wav).unwrap();
    for flag in ["--beats", "--windows", "--trend"] {
        let arg = format!("{flag}={}", wav.display());
        let out = tg(&[
            "analyze".as_ref(),
            wav.as_os_str(),
            "--bph=21600".as_ref(),
            arg.as_ref(),
        ]);
        assert!(!out.status.success(), "{flag}: {out:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("recording being read"), "{flag}: {err}");
        assert_eq!(
            std::fs::read(&wav).unwrap(),
            before,
            "{flag} changed the recording"
        );
    }
    let csv = dir.0.join("both.csv");
    let (b, w) = (
        format!("--beats={}", csv.display()),
        format!("--windows={}", csv.display()),
    );
    let out = tg(&["analyze".as_ref(), wav.as_os_str(), b.as_ref(), w.as_ref()]);
    assert!(!out.status.success(), "{out:?}");
    assert!(
        !csv.exists(),
        "nothing is written when two outputs share a file"
    );
}

/// A take folder, as the app saves one, is read as its one recording with
/// the take's own settings (its session file's calibre, lift and beat
/// rate) by `analyze`, `shape` and `profile`, giving the file's document, and that
/// recording is still refused as an output (#2092).
#[test]
fn a_take_folder_is_read_as_its_recording() {
    let dir = Dir(std::env::temp_dir().join(format!("tg-outputs-take-{}", std::process::id())));
    let take = dir.0.join("take");
    std::fs::create_dir_all(&take).unwrap();
    let wav = take.join("take_part01-001.wav");
    let made = tg(&[
        "synth".as_ref(),
        wav.as_os_str(),
        "--duration=40".as_ref(),
        "--bph=18000".as_ref(),
    ]);
    assert!(made.status.success(), "{made:?}");
    std::fs::write(
        take.join("session.toml"),
        "calibre = \"ETA 2824-2\"\nlift = 40\nbph = 18000\n\n[[recording]]\nfile = \".\"\nposition = \"DU\"\n",
    )
    .unwrap();
    let json = |command: &str, p: &std::path::Path| {
        let out = tg(&[command.as_ref(), p.as_os_str(), "--json".as_ref()]);
        assert!(out.status.success(), "{out:?}");
        let mut doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        doc.as_object_mut().unwrap().remove("software");
        doc
    };
    let file = json("analyze", &wav);
    assert_eq!(file["lift_deg"], 40.0, "the take's lift is read: {file}");
    assert_eq!(file["bph"], 18000, "{file}");
    assert!(file["take"].is_object(), "{file}");
    assert_eq!(json("analyze", &take), file);
    assert_eq!(json("shape", &take), json("shape", &wav));
    assert_eq!(json("profile", &take), json("profile", &wav));
    let before = std::fs::read(&wav).unwrap();
    let arg = format!("--beats={}", wav.display());
    let out = tg(&["analyze".as_ref(), take.as_os_str(), arg.as_ref()]);
    assert!(!out.status.success(), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("recording being read"));
    assert_eq!(std::fs::read(&wav).unwrap(), before);
    std::fs::copy(&wav, take.join("take_part01-002.wav")).unwrap();
    let out = tg(&["analyze".as_ref(), take.as_os_str()]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("a take folder of 2 recordings"), "{err}");
    assert!(!err.contains("extension"), "{err}");
}
