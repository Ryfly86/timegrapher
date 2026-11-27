//! The watch a recording was heard with, read back from its own
//! `session.json` ([`take::RECORD`]): what a take whose sound was never
//! saved has instead of a `session.toml` (#2376).

use super::*;
use crate::input_domain;
use serde::Deserialize;

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
}

#[derive(Deserialize)]
struct Written {
    #[serde(default)]
    watch: String,
    position: Option<String>,
    lift_deg: Option<f64>,
    bph: Option<u64>,
}

/// What the `session.json` in `dir` says of the watch, or `None` when
/// there is none, it can't be read, or it holds no lift angle (so is not
/// one the recorder wrote).
pub fn recorded_setup(dir: &Path) -> Option<RecordedSetup> {
    let text = read_settled(&dir.join(take::RECORD)).ok()?;
    let w: Written = serde_json::from_str(&text).ok()?;
    let lift = w.lift_deg?;
    Some(RecordedSetup {
        watch: w.watch,
        position: w.position.filter(|p| !p.trim().is_empty()),
        lift_deg: input_domain::lift_deg(lift).ok(),
        bph: w.bph.and_then(|b| input_domain::bph(b).ok()),
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
            }
        );
        drop(r);
        let _ = fs::remove_dir_all(&parent);
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
}
