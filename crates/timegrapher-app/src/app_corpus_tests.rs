//! Each corpus take opened as the app opens it, headless, reads what the
//! command line reads for it (#514): each recording the take's session.toml
//! lists (the whole take, ".", or one file of it), opened in the app,
//! reads on the cards what `timegrapher session` reads for that entry, to
//! the last bit. Both read through core, so any difference is logic
//! written twice.
//!
//! Each take opened whole reads in its panes what the command line
//! reads for it too: the Distributions pane's distributions are `series`'s,
//! and the Power Reserve pane's run from a long take is `session`'s.
//! Gaps still open are listed, each with its issue.
//!
//! The same holds for core's synthetic edge takes, written afresh.
//!
//! Needs the corpus (`TIMEGRAPHER_CORPUS`, its audio fetched) and a built
//! `timegrapher` (`TIMEGRAPHER_CLI`); the edge takes need only the latter.
//! With neither set it skips, saying so; with `TIMEGRAPHER_REQUIRE_CORPUS`
//! set, as in CI, a missing one fails.

use super::*;
use std::time::Instant;

/// The longest the app may take to open one take: the overnight take is
/// eight hours of audio.
const OPEN_LIMIT_S: u64 = 1200;

/// The corpus root and the command line, or none where the test skips.
fn corpus_and_cli() -> Option<(PathBuf, PathBuf)> {
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let root = std::env::var_os("TIMEGRAPHER_CORPUS").map(PathBuf::from);
    let cli = std::env::var_os("TIMEGRAPHER_CLI").map(PathBuf::from);
    match (root, cli) {
        (Some(root), Some(cli)) => {
            assert!(
                root.join("corpus.json").is_file(),
                "no corpus.json in {root:?}"
            );
            assert!(cli.is_file(), "no command line at {cli:?}");
            Some((root, cli))
        }
        _ if required => panic!("TIMEGRAPHER_CORPUS and TIMEGRAPHER_CLI are both needed"),
        _ => {
            eprintln!("SKIP: set TIMEGRAPHER_CORPUS and TIMEGRAPHER_CLI to compare the app with the command line");
            None
        }
    }
}

/// The takes the corpus lists that have a session.toml, by id, but for its
/// holdout takes: their audio is fetched only for `regress --holdout`, and
/// their readings stay sealed (docs/developer/regress.md, "Holdout takes").
fn takes(root: &Path) -> Vec<String> {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("corpus.json")).unwrap()).unwrap();
    json["takes"]
        .as_array()
        .expect("corpus.json lists its takes")
        .iter()
        .filter(|t| t["tier"].as_str() != Some("holdout"))
        .filter_map(|t| t["id"].as_str().map(str::to_string))
        .filter(|id| root.join("takes").join(id).join("session.toml").is_file())
        .collect()
}

/// The take opened as File > Open Recording opens it, waited for.
fn opened(dir: &Path) -> TimegrapherApp {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    // At 10 s, so edge_45s (25 s measured) guards against the whole take
    // needing a multiple of Average Over again.
    app.set_average(10.0);
    app.start_batch(dir);
    let t = Instant::now();
    while app.batch.is_some() && t.elapsed().as_secs() < OPEN_LIMIT_S {
        app.poll();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        app.batch.is_none(),
        "{dir:?} still opening after {OPEN_LIMIT_S} s"
    );
    app
}

/// `timegrapher session --json` for the take: its readings, by label.
fn session_readings(cli: &Path, dir: &Path) -> Vec<serde_json::Value> {
    session_json(cli, dir)["readings"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// `timegrapher session --json` for the take.
fn session_json(cli: &Path, dir: &Path) -> serde_json::Value {
    let out = std::env::temp_dir().join(format!(
        "tg-app-corpus-{}-{}",
        dir.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&out);
    let run = std::process::Command::new(cli)
        .arg("session")
        .arg(dir)
        .arg("--json")
        .arg("--out")
        .arg(&out)
        .output()
        .expect("the command line runs");
    let _ = std::fs::remove_dir_all(&out);
    assert!(
        run.status.success(),
        "session {dir:?}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    serde_json::from_slice(&run.stdout).expect("session JSON")
}

/// Where the app's whole-take reading and the command line's differ, in
/// words, figures and caveats alike; none where they are the same.
fn differences(id: &str, app: &Option<WholeTake>, cli: &serde_json::Value) -> Vec<String> {
    let m = &cli["measurement"];
    let Some(w) = app else {
        return vec![format!(
            "{id}: the app has no whole-take reading; session has {m}"
        )];
    };
    let mut out = Vec::new();
    let mut same = |what: &str, a: Option<f64>, key: &str| {
        let b = m[key].as_f64();
        if a != b {
            out.push(format!("{id}: {what} {a:?} in the app, {b:?} from session"));
        }
    };
    same("rate", w.m.rate_s_per_day, "rate_s_per_day");
    same("amplitude", w.m.amplitude_deg, "amplitude_deg");
    same("beat error", w.m.beat_error_ms, "beat_error_ms");
    same("start", Some(w.m.start_s), "start_s");
    same("end", Some(w.m.end_s), "end_s");
    same("beats", Some(w.m.beats as f64), "beats");
    // The caveats each names, in any order.
    let mut app_codes = timegrapher_core::edge_takes::caveat_codes(&w.m);
    let mut cli_codes: Vec<String> = m["caveats"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["code"].as_str().map(str::to_string))
        .collect();
    app_codes.sort();
    cli_codes.sort();
    if app_codes != cli_codes {
        out.push(format!(
            "{id}: caveats {app_codes:?} in the app, {cli_codes:?} from session"
        ));
    }
    out
}

/// Corpus entries the app still reads differently from session, and why.
/// Each must still differ, so the list only shrinks as the app is fixed. A
/// file the session file lists more than once is named with its entry's
/// place among them: `file entry 1`.
const KNOWN: &[(&str, &str)] = &[
    (
        "ym42_CR_long/take_part01-003.flac entry 1",
        "by design (#1197): one position split in two entries of one file; the app reads \
         a position's stretches together, as a Positions Test does",
    ),
    (
        "ym42_CR_long/take_part01-003.flac entry 2",
        "by design (#1197): one position split in two entries of one file; the app reads \
         a position's stretches together, as a Positions Test does",
    ),
];

/// Each recording the takes under `root` list, read on the app's cards and
/// by `timegrapher session`, compared: what differs and is not on `known`,
/// or is on it and no longer differs. Takes, and then recordings, are read
/// side by side, one to a core, so the whole corpus fits CI's time.
fn compare(root: &Path, cli: &Path, known: &[(&str, &str)]) -> Vec<String> {
    let ids = takes(root);
    let readings = timegrapher_core::cores::on_every_core(&ids, |id| {
        session_readings(cli, &root.join("takes").join(id))
    });
    let entries: Vec<(String, serde_json::Value)> = ids
        .iter()
        .zip(readings)
        .flat_map(|(id, rs)| rs.into_iter().map(move |r| (id.clone(), r)))
        .collect();
    let label = |r: &serde_json::Value| r["label"].as_str().expect("a reading's label").to_string();
    // Each reading's name, with its place among the entries for its file
    // when there are several.
    let names: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(i, (id, r))| {
            let file =
                |(other, o): &(String, serde_json::Value)| other == id && label(o) == label(r);
            let name = format!("{id}/{}", label(r));
            if entries.iter().filter(|e| file(e)).count() > 1 {
                let place = entries[..i].iter().filter(|e| file(e)).count() + 1;
                format!("{name} entry {place}")
            } else {
                name
            }
        })
        .collect();
    let named: Vec<_> = entries.iter().zip(names).collect();
    let found = timegrapher_core::cores::on_every_core(&named, |((id, reading), name)| {
        let label = label(reading);
        let name = name.clone();
        let app = opened(&root.join("takes").join(id).join(label));
        let found = differences(&name, &app.whole_take(), reading);
        match known.iter().find(|(k, _)| *k == name) {
            Some((_, why)) if !found.is_empty() => {
                eprintln!("{name}: differs, as known ({why})");
                Vec::new()
            }
            Some(_) => vec![format!("{name}: now the same; take it off its known list")],
            None => {
                eprintln!(
                    "{name}: {}",
                    if found.is_empty() { "same" } else { "DIFFERS" }
                );
                found
            }
        }
    });
    eprintln!("{} recordings compared", entries.len());
    assert!(!entries.is_empty(), "no recording to compare in {root:?}");
    found.into_iter().flatten().collect()
}

/// Edge takes the app still reads differently from session, and why. Each
/// must still differ, so the list only shrinks as the app is fixed.
const EDGE_KNOWN: &[(&str, &str)] = &[
    (
        "edge_sleep_two_entries/. (0–90 s)",
        "one of two readings of one file: the app opens a file whole, not a stretch of it",
    ),
    (
        "edge_sleep_two_entries/. (90–180 s)",
        "one of two readings of one file: the app opens a file whole, not a stretch of it",
    ),
    (
        "edge_reads_nothing/. (0–90 s)",
        "a stretch of a file: the app opens a file whole, not a stretch of it",
    ),
];

/// Every recording each corpus take lists reads the same on the app's
/// cards as from `timegrapher session`.
#[test]
fn the_app_reads_each_corpus_take_as_the_command_line_does() {
    let Some((root, cli)) = corpus_and_cli() else {
        return;
    };
    let wrong = compare(&root, &cli, KNOWN);
    assert!(
        wrong.is_empty(),
        "the app and session differ:\n{}",
        wrong.join("\n")
    );
}

/// The empty stand opened: no watch heard, so the beat rate shows
/// "Auto" with no number, not the rate the engine last listened at
/// (#1972, "Auto 21600"). Needs only `TIMEGRAPHER_CORPUS`.
#[test]
fn the_empty_stand_finds_no_beat_rate() {
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let Some(root) = std::env::var_os("TIMEGRAPHER_CORPUS").map(PathBuf::from) else {
        assert!(!required, "TIMEGRAPHER_CORPUS is needed");
        eprintln!("SKIP: set TIMEGRAPHER_CORPUS to open the empty stand");
        return;
    };
    let app = opened(&root.join("takes").join("silence_empty_stand"));
    assert_eq!(app.bph_heard(), None);
    assert_eq!(app.watch_facts().1, "Auto");
}

/// Each synthetic edge take (core's `edge_takes`: short, paused, slept,
/// silent, unlisted breaks, a drifting card) reads the same on the app's
/// cards as from `timegrapher session`. Needs only `TIMEGRAPHER_CLI`.
#[test]
fn the_app_reads_each_edge_take_as_the_command_line_does() {
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let Some(cli) = std::env::var_os("TIMEGRAPHER_CLI").map(PathBuf::from) else {
        assert!(!required, "TIMEGRAPHER_CLI is needed");
        eprintln!("SKIP: set TIMEGRAPHER_CLI to compare the app with the command line");
        return;
    };
    let root = std::env::temp_dir().join(format!("tg-app-edge-takes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    timegrapher_core::edge_takes::write_all(&root).unwrap();
    let wrong = compare(&root, &cli, EDGE_KNOWN);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        wrong.is_empty(),
        "the app and session differ:\n{}",
        wrong.join("\n")
    );
}

/// Split entries retain their clock even without readable recorder metadata.
fn split_entries_clock_probe(metadata: &str, folder: bool) {
    use timegrapher_core::{audio, synth};
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let Some(cli) = std::env::var_os("TIMEGRAPHER_CLI").map(PathBuf::from) else {
        assert!(!required, "TIMEGRAPHER_CLI is needed");
        eprintln!("SKIP: set TIMEGRAPHER_CLI for the duplicate-entry probe");
        return;
    };
    let root = std::env::temp_dir().join(format!(
        "tg-app-duplicate-clock-{metadata}-{folder}-{}",
        std::process::id()
    ));
    let dir = root.join(format!("takes/duplicate_clock_{metadata}_{folder}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&dir).unwrap();
    let sound = synth::generate(
        &synth::SynthConfig {
            duration_s: 300.0,
            rate_s_per_day: -7.0,
            ..Default::default()
        },
        |_| 270.0,
        |_| 0.0,
    );
    let fs = sound.sample_rate;
    audio::write_wav(&dir.join("take.wav"), &sound).unwrap();
    let mut clock = String::from("unix_s,frames\n");
    let mut frames: Vec<u32> = (0..=30).map(|i| i * 10 * fs).collect();
    frames.push(100 * fs - fs / 10);
    frames.sort_unstable();
    for frame in frames {
        let wall = 1_700_000_000.0
            + frame as f64 / fs as f64 * 1.00002
            + if frame >= 100 * fs { 60.0 } else { 0.0 };
        clock.push_str(&format!("{wall:.6},{frame}\n"));
    }
    std::fs::write(dir.join("clock.csv"), clock).unwrap();
    let record = serde_json::json!({"segments": ["take.wav"], "breaks": [
        {"frame": 100 * fs, "audio_s": 100.0, "gap_s": 60.0}
    ]})
    .to_string();
    match metadata {
        "valid" => std::fs::write(dir.join("session.json"), &record).unwrap(),
        "truncated" => {
            std::fs::write(dir.join("session.json"), &record[..record.len() / 2]).unwrap()
        }
        "missing" => {}
        _ => unreachable!(),
    }
    std::fs::write(
        dir.join("session.toml"),
        "bph = 28800\nlift = 52\n\
         [[recording]]\nfile = \"take.wav\"\nposition = \"DU\"\nend_s = 160\nclock = \"clock.csv\"\n\
         [[recording]]\nfile = \"take.wav\"\nposition = \"DD\"\nstart_s = 160\nclock = \"clock.csv\"\n",
    )
    .unwrap();
    let readings = session_readings(&cli, &dir);
    assert_eq!(readings.len(), 2);
    for reading in &readings {
        let rate = reading["measurement"]["rate_s_per_day"].as_f64().unwrap();
        assert!((rate + 8.728).abs() < 0.2, "corrected rate: {rate}");
    }
    let path = if folder {
        dir.clone()
    } else {
        dir.join("take.wav")
    };
    let mut app = opened(&path);
    let clock = app.take_clock.as_ref().expect("the app retains the clock");
    assert!((clock.ppm + 20.0).abs() < 0.1);
    assert_eq!(clock.breaks.len(), 1);
    assert!((clock.breaks[0].gap_s - 60.0).abs() < 0.01);
    for (position, reading) in [Position::DialUp, Position::DialDown]
        .into_iter()
        .zip(&readings)
    {
        app.position = position;
        let wrong = differences(metadata, &app.whole_take(), reading);
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn split_entries_clock_valid_metadata() {
    split_entries_clock_probe("valid", false);
}
#[test]
fn split_entries_clock_truncated_metadata() {
    split_entries_clock_probe("truncated", false);
}
#[test]
fn split_entries_clock_missing_metadata() {
    split_entries_clock_probe("missing", false);
}

#[test]
fn split_entries_clock_folder_valid_metadata() {
    split_entries_clock_probe("valid", true);
}

#[test]
fn split_entries_clock_folder_truncated_metadata() {
    split_entries_clock_probe("truncated", true);
}

#[test]
fn split_entries_clock_folder_missing_metadata() {
    split_entries_clock_probe("missing", true);
}

/// `timegrapher series --json` for the take.
fn series_json(cli: &Path, dir: &Path) -> serde_json::Value {
    let run = std::process::Command::new(cli)
        .arg("series")
        .arg(dir)
        .arg("--json")
        .output()
        .expect("the command line runs");
    assert!(
        run.status.success(),
        "series {dir:?}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    serde_json::from_slice(&run.stdout).expect("series JSON")
}

/// The figures of a distribution that the app draws and `series` writes:
/// its count, quantiles, bins and peaks (not its step, which `series`
/// gives on the corrected clock).
fn dist_figures(d: &serde_json::Value) -> serde_json::Value {
    let keys = [
        "n", "median", "p10", "p90", "edges", "counts", "below", "above", "peaks",
    ];
    keys.iter()
        .map(|k| ((*k).to_string(), d[k].clone()))
        .collect()
}

/// The figures of two distributions that differ, the app's then
/// `series`'s, in words.
fn figures_apart(ours: Option<&serde_json::Value>, theirs: Option<&serde_json::Value>) -> String {
    let (Some(a), Some(b)) = (ours, theirs) else {
        return format!("{ours:?} in the app, {theirs:?} from series");
    };
    let short = |v: &serde_json::Value| v.to_string().chars().take(80).collect::<String>();
    a.as_object()
        .into_iter()
        .flatten()
        .filter(|(k, v)| b.get(k.as_str()) != Some(v))
        .map(|(k, v)| {
            format!(
                "{k} {} in the app, {} from series",
                short(v),
                short(&b[k.as_str()])
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Where the app's panes for the whole take, opened as one recording,
/// differ from the command line's: each distribution the Distributions
/// pane draws, against `series`, and the run the Power Reserve pane reads
/// from a long take, against `session`. Each as `(what, how)`; `what` is
/// `id`, then the series and its values ("readings" or "2 s"), or
/// "power reserve". With them, whether either surface read a run, and
/// how many seconds of the take the app read.
fn pane_differences(id: &str, root: &Path, cli: &Path) -> (Vec<(String, String)>, bool, f64) {
    use timegrapher_core::distribution as dist;
    let dir = root.join("takes").join(id);
    let mut app = opened(&dir);
    app.update_dist_readings(None);
    let series = series_json(cli, &dir);
    let mut out = Vec::new();
    let theirs = series["distributions"]["overall"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (k, short) in [
        (SeriesKind::Rate, false),
        (SeriesKind::Amplitude, false),
        (SeriesKind::Amplitude, true),
        (SeriesKind::BeatError, false),
        (SeriesKind::BeatError, true),
    ] {
        let (values, kind, _, label, _) = app.hist_values(k, short, &theme::DARK);
        let step = if short {
            dist::STRETCH_S
        } else {
            app.average_s
        };
        let ours = dist::distribution(&values, kind, label, step)
            .map(|d| dist_figures(&serde_json::to_value(d).unwrap()));
        let other = theirs
            .iter()
            .find(|d| d["series"] == kind.series && d["values"] == label)
            .map(dist_figures);
        if ours != other {
            out.push((
                format!("{id} {} {label}", kind.series),
                figures_apart(ours.as_ref(), other.as_ref()),
            ));
        }
    }
    app.reserve_from_long_take();
    let ours = app
        .reserve
        .as_ref()
        .and_then(|t| t.results.as_ref())
        .map(|r| serde_json::to_value(r).unwrap())
        .unwrap_or(serde_json::Value::Null);
    let theirs = session_json(cli, &dir)["power_reserve"].clone();
    let run = !ours.is_null() || !theirs.is_null();
    if ours != theirs {
        out.push((
            format!("{id} power reserve"),
            format!("{ours} in the app, {theirs} from session"),
        ));
    }
    let length = app.live.as_ref().map_or(0.0, |l| l.duration_s());
    (out, run, length)
}

/// Pane figures the app still reads differently from the command line,
/// with the issue that tracks each. A name starting `* ` stands for every
/// take; each entry must still differ on at least one take, so the list
/// only shrinks as the app is fixed. A take's "power reserve" entry is
/// held to that only where a surface reads a run: a pull request's corpus
/// cut is too short for one. Likewise an entry is held only where the take
/// read reaches the seconds it gives, where its difference is: a pull
/// request's cut of three files may stop short of it.
const PANES_KNOWN: &[(&str, &str, f64)] = &[
    ("ym42_CR_long amplitude 2 s", BREAK_EDGE, 5400.0),
    ("ym42_CR_long beat_error 2 s", BREAK_EDGE, 5400.0),
];

/// #636, in [`PANES_KNOWN`]: at ym42_CR_long's 5,400 s break (3.883 s
/// gap) the app keeps the 2 s window ending at the break and leaves out
/// the next, cut by a jump its engine finds 0.05 s after the break;
/// `series` reads the break on true time with its gap and keeps the
/// other. One window of about 3,700.
const BREAK_EDGE: &str = "#636: the app and series pick a different 2 s window at a break \
                          with a gap";

/// Whether the difference `what` is the known gap `known`.
fn is_known(known: &str, what: &str) -> bool {
    known
        .strip_prefix("* ")
        .map_or(known == what, |rest| what.ends_with(&format!(" {rest}")))
}

/// For each corpus take, opened whole in the app as File > Open Recording
/// opens it, the Distributions pane's distributions are `series`'s, and
/// the Power Reserve pane's run from a long take is `session`'s (#514).
/// Both read through core, so any difference is logic written twice.
#[test]
fn the_apps_panes_read_each_corpus_take_as_the_command_line_does() {
    let Some((root, cli)) = corpus_and_cli() else {
        return;
    };
    let ids = takes(&root);
    assert!(!ids.is_empty(), "no take to compare in {root:?}");
    let read = timegrapher_core::cores::on_every_core(&ids, |id| pane_differences(id, &root, &cli));
    let with_run: Vec<&str> = ids
        .iter()
        .zip(&read)
        .filter(|(_, (_, run, _))| *run)
        .map(|(id, _)| id.as_str())
        .collect();
    // Each take's id with the seconds of it the app read.
    let lengths: Vec<(&str, f64)> = ids
        .iter()
        .zip(&read)
        .map(|(id, (_, _, length))| (id.as_str(), *length))
        .collect();
    let found: Vec<(String, String)> = read.into_iter().flat_map(|(d, _, _)| d).collect();
    let mut wrong = Vec::new();
    for (what, how) in &found {
        match PANES_KNOWN.iter().find(|(k, _, _)| is_known(k, what)) {
            Some((_, why, _)) => eprintln!("{what}: differs, as known ({why}): {how}"),
            None => wrong.push(format!("{what}: {how}")),
        }
    }
    for &(known, _, reaches_s) in PANES_KNOWN {
        let held = known
            .strip_suffix(" power reserve")
            .is_none_or(|id| with_run.contains(&id))
            && lengths.iter().any(|&(id, length)| {
                let names = known.starts_with("* ") || known.starts_with(&format!("{id} "));
                names && length >= reaches_s
            });
        if held && !found.iter().any(|(what, _)| is_known(known, what)) {
            wrong.push(format!("{known}: now the same; take it off PANES_KNOWN"));
        }
    }
    eprintln!("{} takes compared", ids.len());
    assert!(
        wrong.is_empty(),
        "the app's panes and the command line differ:\n{}",
        wrong.join("\n")
    );
}

/// `secs` of the corpus recording `flac` from `from_s` in, listened to as
/// from a microphone whose blocks are stamped `ppm` fast against the
/// computer's clock, stopped and saved: the saved take's folder, under
/// `base`. The input stalls for `pause` (audio seconds in, and how long by
/// the computer's clock) where one is given, so the take holds a break
/// there, and `silent_s` of silence follows the sound, as when the watch
/// is taken off the stand before Stop.
fn listened_and_saved(
    base: &Path,
    flac: &Path,
    (from_s, secs): (f64, f64),
    ppm: f64,
    pause: Option<(f64, f64)>,
    silent_s: f64,
) -> PathBuf {
    std::fs::create_dir_all(base).unwrap();
    let audio = timegrapher_core::audio::load(flac).unwrap();
    let fs = audio.sample_rate as usize;
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.reset_session(audio.sample_rate, "test".into());
    app.session.listen();
    let info = app.info("test", audio.sample_rate, 16);
    app.session.recorder = Some(Recorder::start(&base.join("live"), info).unwrap());
    let tx = super::tests::queued_capture(&mut app, base);
    let from = (from_s * fs as f64) as usize;
    let block = fs / 50;
    let mut sound = audio.samples[from..from + (secs * fs as f64) as usize].to_vec();
    // The tail is a quiet stand's hiss, not digital zero: a microphone
    // never hears nothing.
    let mut seed = 0x2073_u32;
    sound.extend((0..(silent_s * fs as f64) as usize).map(|_| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1u32 << 24) as f32 * 2e-3 - 1e-3
    }));
    for (k, samples) in sound.chunks(block).enumerate() {
        let heard = (k * block) as f64 / fs as f64;
        let stalled = pause.filter(|&(at, _)| heard >= at).map_or(0.0, |(_, s)| s);
        let us = 3e6 + (heard / (1.0 + ppm * 1e-6) + stalled) * 1e6;
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_micros(us as u64);
        let samples = samples.to_vec();
        tx.send(timegrapher_core::capture::Event::Audio(
            timegrapher_core::capture::Block { samples, at },
        ))
        .unwrap();
    }
    while app.poll_for(std::time::Duration::from_secs(60)) > 0 {}
    app.stop();
    app.save_into(&base.join("saved"));
    app.wait_for_save();
    app.session
        .saved_to()
        .expect("the take is saved")
        .to_path_buf()
}

/// The measurement behind the Watches row of the take saved in `dir`:
/// its last position read as `library::reading` reads it, through core's
/// `take::measure_entry`, with its ±s and caveats.
fn row_measurement(dir: &Path) -> timegrapher_core::session::Measurement {
    use timegrapher_core::take;
    let text = std::fs::read_to_string(dir.join(take::MANIFEST)).unwrap();
    let m: timegrapher_core::session::Manifest = toml::from_str(&text).unwrap();
    let e = m.recordings.last().expect("a saved take has a position");
    let settings = take::EntrySettings {
        stream: take::stream_settings(&m, None, take::Given::default()),
        card_clock: false,
        settle_s: None,
    };
    let read = take::measure_entry(dir, &m, e, &settings, take::usb, |first| {
        take::correction_for(&m, first, None, None)
    });
    read.unwrap_or_else(|e| panic!("the saved take reads: {e}"))
        .measurement
}

/// Where the Watches row of the take saved in `dir` and its cards once
/// opened, at its end with the Average Over over the whole take, differ,
/// in words: figures, ±s, why one has none, caveats, and the beats and
/// span they rest on. `paused` says the take should hold a break.
fn row_against_cards(name: &str, dir: &Path, paused: bool) -> Vec<String> {
    let mut wrong = Vec::new();
    let row = timegrapher_core::library::reading(dir).unwrap();
    let x = row_measurement(dir);
    let mut app = opened(dir);
    app.set_average(60.0);
    // As the window does: the whole take read for the cards' hover.
    let _ = app.whole_take();
    let card = app.reading().expect("the opened take reads at its end");
    let shown_be =
        timegrapher_core::session::beat_error_signed(card.beat_error_unlock_ms, card.beat_error_ms);
    let c = x.confidence;
    let pairs = [
        ("rate", row.rate_s_per_day, card.rate_s_per_day),
        ("amplitude", row.amplitude_deg, card.amplitude_deg),
        ("beat error", row.beat_error_ms, shown_be),
        (
            "rate ±",
            c.and_then(|c| c.rate.se),
            card.rate_error_s_per_day,
        ),
        (
            "amplitude ±",
            c.and_then(|c| c.amplitude.se),
            card.amplitude_error_deg,
        ),
        (
            "beat error ±",
            c.and_then(|c| c.beat_error.se),
            card.beat_error_unlock_error_ms,
        ),
    ];
    for (what, row, card) in pairs {
        eprintln!("{name}: {what} {row:?} in the row, {card:?} on the cards");
        if row.is_none() && what.ends_with('±') {
            continue;
        }
        if row.is_none() || row != card {
            wrong.push(format!(
                "{name}: {what} {row:?} in the row, {card:?} on the cards"
            ));
        }
    }
    // The beats and seconds the figures rest on: the fit's own (Norm).
    let span = x.end_s - x.start_s - x.masked_s;
    if (x.beats_fitted, span) != (card.beats_used, card.span_s) {
        wrong.push(format!(
            "{name}: {} beats fitted over {span} s in the row, {} over {} on the cards",
            x.beats_fitted, card.beats_used, card.span_s
        ));
    }
    if paused && x.breaks.is_empty() {
        wrong.push(format!("{name}: the pause left no break"));
    }
    // Across the pause, the Rate hover names the whole take's beats, both
    // sides of it (Hugh 5 on #2075).
    let said = app.rate_beats_behind(Some(&card));
    let first = said.split("\n\n").next().unwrap_or_default().to_string();
    let whole = format!(
        "{} beats in {} of 1 min: the beats the readings are fitted to, fewer seconds than \
         Average Over asks for. They are the whole take after settling, fitted apart at each \
         break, masked stretch or gap of more than 2 s with no beats, with any piece under 10 s \
         left out.",
        x.beats_fitted,
        crate::fields::duration(span.round())
    );
    if paused && first != whole {
        wrong.push(format!(
            "{name}: the Rate hover says {first:?}, not {whole:?}"
        ));
    }
    let why = (
        c.and_then(|c| c.rate.no_pm),
        c.and_then(|c| c.amplitude.no_pm),
    );
    if why != (card.rate_no_pm, card.amplitude_no_pm) || x.caveats != card.caveats {
        wrong.push(format!(
            "{name}: no-± reasons {why:?} and caveats {:?} in the row, {:?} and {:?} \
             on the cards",
            x.caveats,
            (card.rate_no_pm, card.amplitude_no_pm),
            card.caveats
        ));
    }
    wrong
}

/// A saved take's Watches row and its cards once opened, at its end with
/// the Average Over over the whole take, give one rate, amplitude and
/// beat error, with their ±s, why one has none and the caveats, to the
/// last bit (#2073, Norm's acceptance (a)): the row is
/// `library::reading`, `session`'s reading of the take, and the cards read
/// the same stretch on the same clock. Each take is listened to as from a
/// microphone, on the card's own clock (0 ppm) and on one 25 ppm fast, so
/// a 50 s take's clock log corrects and a 5 s one's is withheld. Three
/// corpus takes, one of them again with a 5 s pause, and 5 s of the first
/// at four places; the cards' beats and span are the row's too. Needs only
/// `TIMEGRAPHER_CORPUS`.
#[test]
fn a_saved_takes_row_and_its_cards_give_one_reading() {
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let Some(root) = std::env::var_os("TIMEGRAPHER_CORPUS").map(PathBuf::from) else {
        assert!(!required, "TIMEGRAPHER_CORPUS is needed");
        eprintln!("SKIP: set TIMEGRAPHER_CORPUS to read saved takes");
        return;
    };
    let file = |id: &str| root.join("takes").join(id).join("take_part01-001.flac");
    let first = file("eta2824_bare_DU_10min_g4");
    let mut cases = vec![
        (first.clone(), (60.0, 50.0), None),
        (
            file("pepsi3285_DU_10min_newstand_firsttest"),
            (60.0, 50.0),
            None,
        ),
        (
            file("peacock1258_DU_15min_lowwind_newstand"),
            (60.0, 50.0),
            None,
        ),
        // A take with a pause: the cards' beats are the whole stretch's,
        // not the latest unbroken run's (Norm on #2075).
        (first.clone(), (60.0, 50.0), Some((25.0, 5.0))),
    ];
    cases.extend([0.0, 30.0, 100.0, 300.0].map(|at| (first.clone(), (at, 5.0), None)));
    let mut wrong = Vec::new();
    for (k, (flac, stretch, pause)) in cases.iter().enumerate() {
        for ppm in [0.0, 25.0] {
            let name = format!(
                "{} {stretch:?} at {ppm} ppm, paused {pause:?}",
                flac.display()
            );
            let base = std::env::temp_dir()
                .join(format!("tg-one-reading-{k}-{ppm}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&base);
            let dir = listened_and_saved(&base, flac, *stretch, ppm, *pause, 0.0);
            wrong.extend(row_against_cards(&name, &dir, pause.is_some()));
            let _ = std::fs::remove_dir_all(&base);
        }
    }
    assert!(
        wrong.is_empty(),
        "the row and the cards differ:\n{}",
        wrong.join("\n")
    );
}

/// The app as the window runs it with a take's folder given at launch,
/// as `timegrapher-app <folder>` opens it: a fresh app, its settings at
/// their defaults, drawn frame by frame until the take is read, with the
/// text the frame painted.
fn window_opened(dir: &Path) -> (TimegrapherApp, egui::Context) {
    let mut app = TimegrapherApp::new(Some(dir.to_path_buf()), None);
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    let t = Instant::now();
    loop {
        let _ = window_frame(&mut app, &ctx);
        if app.live.is_some() && app.batch.is_none() {
            break;
        }
        assert!(
            t.elapsed().as_secs() < OPEN_LIMIT_S,
            "{dir:?} still opening after {OPEN_LIMIT_S} s"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    (app, ctx)
}

/// One frame of the window, 1920 by 1080, and the text it painted.
fn window_frame(app: &mut TimegrapherApp, ctx: &egui::Context) -> String {
    let mut frame = eframe::Frame::_new_kittest();
    let out = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1920.0, 1080.0),
            )),
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut frame),
    );
    super::tests::painted_text(&out)
}

/// A saved take whose watch was taken off the stand before Stop, opened
/// at launch as the window opens it: its cards at its end read the last
/// stretch with a watch heard, not "No Beats Heard" over the quiet, at
/// the default Average Over and at one the quiet outlasts (Vernon and
/// Walter 15 on #2075). Needs only `TIMEGRAPHER_CORPUS`.
#[test]
fn a_saved_takes_cards_read_past_a_silent_tail() {
    let required = std::env::var_os("TIMEGRAPHER_REQUIRE_CORPUS").is_some();
    let Some(root) = std::env::var_os("TIMEGRAPHER_CORPUS").map(PathBuf::from) else {
        assert!(!required, "TIMEGRAPHER_CORPUS is needed");
        eprintln!("SKIP: set TIMEGRAPHER_CORPUS to read saved takes");
        return;
    };
    // 30 s of a watch then a quiet stand: 9.7 s, as Walter 15's take,
    // just under Average Over, so the watch verdict still hears the watch
    // before it with no beat in the stretch; 10 s; 10 s with the capture
    // stalled 2 s at 31 s, so the quiet after that break still awaits its
    // verdict. And 70 s, longer than every Average Over. At 1 min that one
    // reads its last beat, which the hiss makes a stray one, as a take
    // just stopped does on main (#2081). Takes the PR corpus cut holds.
    let pepsi = "pepsi3285_DU_10min_newstand_firsttest";
    let eta = "eta2824_bare_DU_10min_g4";
    let both: &[f64] = &[10.0, 60.0];
    let takes = [
        (pepsi, 60.0, 9.7, None, both),
        (pepsi, 60.0, 10.0, None, both),
        (pepsi, 60.0, 10.0, Some((31.0, 2.0)), &[10.0][..]),
        (eta, 200.0, 10.0, None, both),
        (eta, 200.0, 70.0, None, &[10.0][..]),
    ];
    for (k, (take, from_s, silent_s, pause, averages)) in takes.into_iter().enumerate() {
        let flac = root.join(format!("takes/{take}/take_part01-001.flac"));
        let base = std::env::temp_dir().join(format!("tg-silent-tail-{k}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = listened_and_saved(&base, &flac, (from_s, 30.0), 0.0, pause, silent_s);
        let (mut app, ctx) = window_opened(&dir);
        for &average in averages {
            app.set_average(average);
            // As the window has it once its background reads are in.
            let _ = app.whole_take();
            let painted = window_frame(&mut app, &ctx);
            let card = app.card_reading().expect("the opened take reads");
            let what =
                format!("{take}, {silent_s} s quiet, pause {pause:?}, Average Over {average} s");
            eprintln!("{what}: {:?} s/d at {} s", card.rate_s_per_day, card.time_s);
            assert!(
                !painted.contains("No Beats Heard"),
                "{what}: the cards say No Beats Heard"
            );
            assert!(
                card.watch_heard && card.listening.is_none() && card.rate_s_per_day.is_some(),
                "{what}: no figures on the cards"
            );
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
