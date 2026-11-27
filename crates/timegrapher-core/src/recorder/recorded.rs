//! The watch a recording was heard with, read back from its own
//! `session.json` ([`take::RECORD`]): what a take whose sound was never
//! saved has instead of a `session.toml` (#2376).

use super::*;
use crate::escapement::Escapement;
use crate::input_domain;
use crate::session::{self, RecordingEntry};
use serde::Deserialize;

/// The start of the text of a note [`Recorder::note`] keeps for a move
/// of the watch, followed by the position's code ("position DD").
pub const MOVED_TO: &str = "position ";

/// The watch settings [`SessionInfo`] wrote into `session.json` as the
/// recording went, each left out when the file's value can't stand.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedSetup {
    /// The watch's name; empty for a Quick Listen.
    pub watch: String,
    /// The position code it was heard in last, as written (DU, CL, 1H30…).
    pub position: Option<String>,
    /// The lift angle it was read with, when it is in
    /// [`input_domain::LIFT_RANGE_DEG`].
    pub lift_deg: Option<f64>,
    /// The beat rate set by hand; `None` when it was guessed (Auto) or is
    /// outside [`input_domain::BPH_RANGE`].
    pub bph: Option<u32>,
    /// Recorded on Auto, and read so: its `bph` written null (a missing
    /// `bph` never says Auto), and no watch it was moved into set a beat
    /// rate since ([`crate::set_rate::WATCH_RATE_KEY`]).
    pub on_auto: bool,
    /// The calibre it was heard with; `None` for none, or a file older
    /// than #2396, which wrote none.
    pub calibre: Option<String>,
    /// The take's own escapement, as [`SessionInfo::escapement`] wrote it;
    /// `None` for none, or a value Timegrapher doesn't read for.
    pub escapement: Option<Escapement>,
    /// The position it started in, as written; `None` for a file older
    /// than #2397, which wrote none.
    pub first_position: Option<String>,
    /// Each move of the watch noted as it recorded ([`MOVED_TO`]), oldest
    /// first: the second of sound it was moved at and the position's code
    /// as written, a code that names no position left out.
    pub moves: Vec<(f64, String)>,
}

impl RecordedSetup {
    /// The take's positions as the entries the app's saved session file
    /// would have listed (#2397), starting in `first`, the watch settling
    /// `settle_s` after each move, in a sound `length_s` long (infinite
    /// where not known): one entry per stretch (file `"."`), each from
    /// where it settled to the next move, the last to the sound's end, and
    /// `settle_s = 0` so that nothing is skipped twice. The first reads
    /// from the start, as no move settles it, unless it is the watch being
    /// handled ([`take::handled`]); a stretch moved on before it settled
    /// reads nothing ([`take::reads_nothing`]). Empty while the watch
    /// stayed in one position.
    pub fn stretches(&self, first: &str, settle_s: f64, length_s: f64) -> Vec<RecordingEntry> {
        let parse = session::Position::parse;
        if parse(first).is_none() {
            return Vec::new();
        }
        // As the app's live stretches go: a move at the same second as
        // the one before replaces it, and a move to where the watch is
        // already is none.
        let mut at: Vec<(f64, &str)> = vec![(0.0, first)];
        for (s, code) in &self.moves {
            if at.last().is_some_and(|l| l.0 >= *s) {
                at.pop();
            }
            if at.last().is_none_or(|l| parse(l.1) != parse(code)) {
                at.push((*s, code));
            }
        }
        if at.len() < 2 {
            return Vec::new();
        }
        (0..at.len())
            .map(|i| {
                let end_s = at.get(i + 1).map(|n| n.0);
                let start = match (i, end_s) {
                    (0, Some(e)) if take::handled(true, e, settle_s) => e,
                    (0, _) => 0.0,
                    _ => at[i].0 + settle_s,
                };
                RecordingEntry {
                    file: ".".into(),
                    position: Some(at[i].1.to_string()),
                    start_s: Some(start.min(end_s.unwrap_or(length_s))),
                    end_s,
                    settle_s: Some(0.0),
                    ..Default::default()
                }
            })
            .collect()
    }
}

/// A note as [`Recorder::note`] wrote it, read loosely.
#[derive(Deserialize)]
struct WrittenNote {
    audio_s: f64,
    text: String,
}

#[derive(Deserialize)]
struct Written {
    #[serde(default)]
    watch: String,
    position: Option<String>,
    lift_deg: Option<f64>,
    /// `None` when there is no `bph` key, `Some(None)` when it is null.
    #[serde(default, deserialize_with = "written")]
    bph: Option<Option<u64>>,
    /// [`crate::set_rate::WATCH_RATE_KEY`]; only `true` marks it.
    #[serde(default)]
    bph_from_watch: serde_json::Value,
    /// Kept loose, so a value that can't stand is left out on its own
    /// rather than refusing the rest of the file.
    #[serde(default)]
    calibre: serde_json::Value,
    #[serde(default)]
    escapement: serde_json::Value,
    first_position: Option<String>,
    /// Kept loose, as `calibre` is: a note that can't stand is left out.
    #[serde(default)]
    notes: Vec<serde_json::Value>,
}

/// A key that is there, null or not.
fn written<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<u64>>, D::Error> {
    Option::<u64>::deserialize(d).map(Some)
}

/// What the `session.json` in `dir` says of the watch, or `None` when
/// there is none, it can't be read, or it holds no lift angle (so is not
/// one the recorder wrote). The one reader of the recorder's info in a
/// take's `session.json`.
pub fn recorded_setup(dir: &Path) -> Option<RecordedSetup> {
    let text = read_settled(&dir.join(take::RECORD)).ok()?;
    let w: Written = serde_json::from_str(&text).ok()?;
    let lift = w.lift_deg?;
    let marked = w.bph_from_watch == serde_json::Value::Bool(true);
    Some(RecordedSetup {
        watch: w.watch,
        position: w.position.filter(|p| !p.trim().is_empty()),
        lift_deg: input_domain::lift_deg(lift).ok(),
        bph: w.bph.flatten().and_then(|b| input_domain::bph(b).ok()),
        on_auto: w.bph == Some(None) && !marked,
        calibre: w
            .calibre
            .as_str()
            .filter(|c| !c.trim().is_empty())
            .map(String::from),
        escapement: serde_json::from_value(w.escapement).ok().flatten(),
        first_position: w.first_position.filter(|p| !p.trim().is_empty()),
        moves: w
            .notes
            .into_iter()
            .filter_map(|n| serde_json::from_value::<WrittenNote>(n).ok())
            .filter(|n| n.audio_s.is_finite())
            .filter_map(|n| Some((n.audio_s, n.text.strip_prefix(MOVED_TO)?.to_string())))
            .filter(|(_, code)| session::Position::parse(code).is_some())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tg-recorded-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// What a recorder writes reads back as it was written.
    #[test]
    fn a_recorders_session_file_reads_back_its_watch() {
        let parent = dir("round-trip");
        let info = SessionInfo {
            watch: String::new(),
            position: "CL".into(),
            lift_deg: 55.0,
            bph: Some(28800),
            device: "test".into(),
            sample_rate: 8000,
            bits: 16,
            software: "test".into(),
            calibre: "Omega 8500".into(),
            escapement: Some(Escapement::SwissLever),
        };
        let r = Recorder::start(&parent, info).unwrap();
        let got = recorded_setup(r.dir()).unwrap();
        assert_eq!(
            got,
            RecordedSetup {
                watch: String::new(),
                position: Some("CL".into()),
                lift_deg: Some(55.0),
                bph: Some(28800),
                on_auto: false,
                calibre: Some("Omega 8500".into()),
                escapement: Some(Escapement::SwissLever),
                first_position: Some("CL".into()),
                moves: Vec::new(),
            }
        );
        drop(r);
        let _ = fs::remove_dir_all(&parent);
    }

    /// Only a `bph` written null says Auto, and a watch's mark
    /// ([`crate::set_rate::WATCH_RATE_KEY`] `true`) undoes it (#2465).
    #[test]
    fn only_a_bph_written_null_and_unmarked_is_on_auto() {
        let d = dir("on-auto");
        let on_auto = |s: &str| {
            fs::write(d.join(take::RECORD), s).unwrap();
            recorded_setup(&d).map(|r| r.on_auto)
        };
        assert_eq!(on_auto(r#"{"lift_deg":52.0,"bph":null}"#), Some(true));
        assert_eq!(on_auto(r#"{"lift_deg":52.0}"#), Some(false));
        assert_eq!(on_auto(r#"{"lift_deg":52.0,"bph":18000}"#), Some(false));
        assert_eq!(on_auto(r#"{"lift_deg":52.0,"bph":5}"#), Some(false));
        // The serde field is named for the key mark_watch_rate writes.
        let key = crate::set_rate::WATCH_RATE_KEY;
        let marked = format!(r#"{{"{key}":true,"lift_deg":52.0,"bph":null}}"#);
        assert_eq!(on_auto(&marked), Some(false));
        for not_marked in ["false", "null", "1", "\"true\""] {
            let s = format!(r#"{{"{key}":{not_marked},"lift_deg":52.0,"bph":null}}"#);
            assert_eq!(on_auto(&s), Some(true), "{s}");
        }
        assert_eq!(on_auto(r#"{"bph":null}"#), None);
        let _ = fs::remove_dir_all(&d);
    }

    /// A guessed beat rate is written `null` and reads as Auto; a value
    /// that can't stand is left out, not used.
    #[test]
    fn a_value_that_cant_stand_is_left_out() {
        let d = dir("bad");
        let write = |s: &str| fs::write(d.join(take::RECORD), s).unwrap();
        write(r#"{"position":"DU","lift_deg":55.0,"bph":null}"#);
        assert_eq!(recorded_setup(&d).unwrap().bph, None);
        write(r#"{"position":"","lift_deg":300.0,"bph":5}"#);
        let got = recorded_setup(&d).unwrap();
        assert_eq!((got.position, got.lift_deg, got.bph), (None, None, None));
        // Not the recorder's: no lift angle, or not JSON.
        write(r#"{"started_utc":"2026-11-26T10:00:00Z"}"#);
        assert_eq!(recorded_setup(&d), None);
        write("not json");
        assert_eq!(recorded_setup(&d), None);
        fs::remove_file(d.join(take::RECORD)).unwrap();
        assert_eq!(recorded_setup(&d), None);
        let _ = fs::remove_dir_all(&d);
    }

    /// A calibre or escapement that can't stand is left out on its own,
    /// the rest read as written; a file older than #2396 has neither
    /// (#2396).
    #[test]
    fn a_calibre_or_escapement_that_cant_stand_is_left_out_alone() {
        let d = dir("calibre");
        let read = |s: &str| {
            fs::write(d.join(take::RECORD), s).unwrap();
            let r = recorded_setup(&d).unwrap();
            (r.calibre, r.escapement, r.position)
        };
        let both =
            r#"{"position":"DU","lift_deg":52.0,"calibre":"Omega 3861","escapement":"co-axial"}"#;
        let co_axial = Some(Escapement::CoAxial);
        assert_eq!(
            read(both),
            (Some("Omega 3861".into()), co_axial, Some("DU".into()))
        );
        let older = r#"{"position":"DU","lift_deg":52.0}"#;
        assert_eq!(read(older), (None, None, Some("DU".into())));
        for bad in [
            r#""calibre":5,"escapement":"detent""#,
            r#""calibre":"  ","escapement":1"#,
        ] {
            let s = format!(r#"{{"position":"DU","lift_deg":52.0,{bad}}}"#);
            assert_eq!(read(&s), (None, None, Some("DU".into())), "{s}");
        }
        let _ = fs::remove_dir_all(&d);
    }

    fn second(r: &mut Recorder) {
        let block = crate::capture::Block {
            samples: vec![0.0; 8000],
            at: std::time::SystemTime::now(),
        };
        r.write(&block).unwrap();
    }

    /// Each move noted as it recorded reads back at its second, and the
    /// position it started in with it; other notes, and a move to a code
    /// that names no position, are left out (#2397).
    #[test]
    fn a_recorders_moves_read_back_with_where_it_started() {
        let parent = dir("moves");
        let info = SessionInfo {
            watch: String::new(),
            position: "DU".into(),
            lift_deg: 52.0,
            bph: None,
            device: "test".into(),
            sample_rate: 8000,
            bits: 16,
            software: "test".into(),
            calibre: String::new(),
            escapement: None,
        };
        let mut r = Recorder::start(&parent, info.clone()).unwrap();
        for _ in 0..30 {
            second(&mut r);
        }
        r.note(&format!("{MOVED_TO}DD")).unwrap();
        r.set_info(SessionInfo {
            position: "DD".into(),
            ..info
        })
        .unwrap();
        for _ in 0..30 {
            second(&mut r);
        }
        r.note("turned to CU").unwrap();
        r.note(&format!("{MOVED_TO}XX")).unwrap();
        r.note(&format!("{MOVED_TO}1H30")).unwrap();
        let got = recorded_setup(r.dir()).unwrap();
        assert_eq!(got.position.as_deref(), Some("DD"));
        assert_eq!(got.first_position.as_deref(), Some("DU"));
        assert_eq!(
            got.moves,
            [(30.0, "DD".to_string()), (60.0, "1H30".to_string())]
        );
        drop(r);
        let _ = fs::remove_dir_all(&parent);
        // A file older than #2397 names no first position; a note that
        // can't stand is left out on its own.
        let d = dir("old-notes");
        let notes = r#"[{"audio_s":"x","text":"position DD"},{"audio_s":5.0,"text":7},
            {"audio_s":9.0,"text":"position CU"}]"#;
        let s = format!(r#"{{"position":"CU","lift_deg":52.0,"notes":{notes}}}"#);
        fs::write(d.join(take::RECORD), s).unwrap();
        let got = recorded_setup(&d).unwrap();
        assert_eq!(got.first_position, None);
        assert_eq!(got.moves, [(9.0, "CU".to_string())]);
        let _ = fs::remove_dir_all(&d);
    }

    fn moved(moves: &[(f64, &str)]) -> RecordedSetup {
        RecordedSetup {
            watch: String::new(),
            position: None,
            lift_deg: None,
            bph: None,
            on_auto: false,
            calibre: None,
            escapement: None,
            first_position: None,
            moves: moves.iter().map(|&(s, c)| (s, c.to_string())).collect(),
        }
    }

    /// What `stretches` lists: (position, start, end), every entry file
    /// "." with `settle_s = 0`, as the app saves a take.
    fn listed(r: &RecordedSetup, first: &str, length_s: f64) -> Vec<(String, f64, Option<f64>)> {
        r.stretches(first, 20.0, length_s)
            .into_iter()
            .map(|e| {
                assert_eq!((e.file.as_str(), e.settle_s), (".", Some(0.0)));
                (e.position.unwrap(), e.start_s.unwrap(), e.end_s)
            })
            .collect()
    }

    /// The stretches of a take moved as it recorded are the entries the
    /// app's saved session file lists: the first from the start, each
    /// later one once it settled, to the next move; a move back at the
    /// same second undoes one, and a move to where it is is none (#2397).
    #[test]
    fn a_moved_takes_stretches_are_its_saved_entries() {
        let s = |p: &str| p.to_string();
        let r = moved(&[(30.0, "DD"), (60.0, "CU"), (60.0, "DD"), (90.0, "CL")]);
        assert_eq!(
            listed(&r, "DU", 200.0),
            [
                (s("DU"), 0.0, Some(30.0)),
                (s("DD"), 50.0, Some(90.0)),
                (s("CL"), 110.0, None)
            ]
        );
        let r = moved(&[(30.0, "DD"), (60.0, "DD"), (90.0, "CU")]);
        assert_eq!(
            listed(&r, "DU", 200.0),
            [
                (s("DU"), 0.0, Some(30.0)),
                (s("DD"), 50.0, Some(90.0)),
                (s("CU"), 110.0, None)
            ]
        );
        // Moved on before it settled: it reads nothing. The last settling
        // past the sound's end reads nothing too.
        let r = moved(&[(30.0, "DD"), (45.0, "CU")]);
        assert_eq!(
            listed(&r, "DU", 50.0),
            [
                (s("DU"), 0.0, Some(30.0)),
                (s("DD"), 45.0, Some(45.0)),
                (s("CU"), 50.0, None)
            ]
        );
        // An opening stretch no longer than the settle is the watch being
        // handled (take::handled): it reads nothing.
        for (at, opening) in [(20.0, 20.0), (21.0, 0.0)] {
            let r = moved(&[(at, "DD")]);
            assert_eq!(listed(&r, "DU", 200.0)[0], (s("DU"), opening, Some(at)));
        }
        // Never moved, or moved only to where it was: one stretch, the
        // take read whole as before.
        assert!(moved(&[]).stretches("DU", 20.0, 200.0).is_empty());
        assert!(moved(&[(30.0, "DU")])
            .stretches("DU", 20.0, 200.0)
            .is_empty());
        assert!(moved(&[(30.0, "DD")])
            .stretches("XX", 20.0, 200.0)
            .is_empty());
    }

    /// Read as a saved take's entries are, the stretches give the changes
    /// of position and the settling after each (#2397).
    #[test]
    fn a_moved_takes_stretches_read_as_a_saved_takes() {
        use crate::session::{Manifest, Position};
        let r = moved(&[(30.0, "DD"), (90.0, "CU")]);
        let entries = r.stretches("DU", 20.0, 200.0);
        let listed = take::listed_positions(&Manifest::default(), &entries, 200.0);
        let (changes, settles) = take::positions_of(listed.into_iter());
        let du = Position::parse("DU").unwrap();
        let dd = Position::parse("DD").unwrap();
        let cu = Position::parse("CU").unwrap();
        assert_eq!(changes, [(0.0, du), (30.0, dd), (90.0, cu)]);
        assert_eq!(settles, [(30.0, 50.0), (90.0, 110.0)]);
    }
}
