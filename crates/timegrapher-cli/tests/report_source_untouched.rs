//! `timegrapher report` only reads the take it bundles: every file it
//! makes on the way goes to its own scratch folder, so the take's tree is
//! byte-identical afterwards, and a read-only take bundles (#2415). Each
//! run's zip has a name of its own, even three in one second (#2416).
use std::path::{Path, PathBuf};
use std::process::Command;

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        set_read_only(&self.0, false);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tg(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(args)
        .output()
        .unwrap()
}

/// A take folder made by `synth`, with a report.html of its own in it.
fn take(name: &str) -> (Dir, PathBuf) {
    let dir =
        Dir(std::env::temp_dir().join(format!("tg-report-src-{name}-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    let take = dir.0.join("take");
    std::fs::create_dir_all(&take).unwrap();
    let made = tg(&[
        "synth".as_ref(),
        take.join("audio-001.wav").as_os_str(),
        "--duration=20".as_ref(),
    ]);
    assert!(made.status.success(), "{made:?}");
    std::fs::write(take.join("report.html"), "the take's own").unwrap();
    (dir, take)
}

/// Every file and folder under `d`, with its bytes, in order.
fn tree(d: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut v = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for p in names {
        if p.is_dir() {
            v.push((p.clone(), None));
            v.extend(tree(&p));
        } else {
            let bytes = std::fs::read(&p).unwrap();
            v.push((p, Some(bytes)));
        }
    }
    v
}

fn set_read_only(d: &Path, on: bool) {
    let Ok(read) = std::fs::read_dir(d) else {
        return;
    };
    for e in read.flatten() {
        if e.path().is_dir() {
            set_read_only(&e.path(), on);
        } else if let Ok(m) = e.metadata() {
            let mut p = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(on);
            let _ = std::fs::set_permissions(e.path(), p);
        }
    }
    if let Ok(m) = std::fs::metadata(d) {
        let mut p = m.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(on);
        let _ = std::fs::set_permissions(d, p);
    }
}

fn zips(d: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "zip"))
        .collect();
    v.sort();
    v
}

fn report(take: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    let mut args: Vec<&std::ffi::OsStr> = vec!["report".as_ref(), take.as_os_str()];
    args.extend(extra.iter().map(std::ffi::OsStr::new));
    args.extend(["--out".as_ref(), out.as_os_str()]);
    tg(&args)
}

#[test]
fn a_report_leaves_the_take_as_it_was() {
    let (dir, take) = take("same");
    let before = tree(&take);
    let out = dir.0.join("bundles");
    let run = report(&take, &out, &["--no-audio"]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(zips(&out).len(), 1);
    let after = tree(&take);
    let names =
        |t: &[(PathBuf, Option<Vec<u8>>)]| t.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
    assert_eq!(names(&after), names(&before), "the take's files changed");
    assert!(after == before, "a file in the take changed");
}

#[test]
fn a_read_only_take_bundles() {
    let (dir, take) = take("read-only");
    let before = tree(&take);
    set_read_only(&take, true);
    let out = dir.0.join("bundles");
    let run = report(&take, &out, &["--at", "0", "--span", "10"]);
    set_read_only(&take, false);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(zips(&out).len(), 1);
    assert!(tree(&take) == before, "the take changed");
}

/// Three reports started together into one folder each leave a zip, and
/// each says the one it wrote.
#[test]
fn three_reports_at_once_leave_three_zips() {
    let (dir, _take) = take("at-once");
    let out = dir.0.join("bundles");
    let runs: Vec<_> = (0..3)
        .map(|i| {
            Command::new(env!("CARGO_BIN_EXE_timegrapher"))
                .args(["report", "--note", &format!("note {i}"), "--out"])
                .arg(&out)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    for mut r in runs {
        assert!(r.wait().unwrap().success());
    }
    let made = zips(&out);
    assert_eq!(made.len(), 3, "{made:?}");
    let mut notes: Vec<String> = made
        .iter()
        .map(|z| {
            let files = timegrapher_core::archive::read_zip(z).unwrap();
            let m: serde_json::Value = serde_json::from_slice(&files[0].1).unwrap();
            m["note"].as_str().unwrap().to_string()
        })
        .collect();
    notes.sort();
    assert_eq!(notes, ["note 0", "note 1", "note 2"]);
}
