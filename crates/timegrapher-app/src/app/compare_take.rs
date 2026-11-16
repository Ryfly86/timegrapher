//! Compare With from an ordinary take (#1851): a take that holds the six
//! positions, though not saved as a Positions Test, read as the test it
//! is set against. Each position is measured as an opened take's table
//! measures it, on the thread of its own a take's reading needs.

use super::*;
use std::sync::mpsc::{Receiver, TryRecvError};
use timegrapher_core::session::Manifest;
use timegrapher_core::take;

/// A take being read to compare with, and its folder's name.
pub(super) struct Reading {
    pub(super) name: String,
    rx: Receiver<Result<guided::FullWind, String>>,
}

/// The take folder `path` names: the folder it is the session file or
/// another file of, when that holds a session file. None for a file
/// outside a take, such as a saved Positions Test's results on their own.
pub(super) fn take_dir_of(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    dir.join(take::MANIFEST).is_file().then_some(dir)
}

/// The take in `dir` read as the results of a Positions Test begun when
/// the take began: each of the six positions its session file lists,
/// in the test's order, measured with `settings` as an opened take's
/// table measures it. A position listed twice is read where it is first.
pub(super) fn full_wind_of_take(
    dir: &Path,
    m: &Manifest,
    settings: &take::EntrySettings,
) -> Result<guided::FullWind, String> {
    let about: Option<serde_json::Value> = std::fs::read_to_string(dir.join("session.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let began_utc = about
        .as_ref()
        .and_then(|a| a.get("started_utc"))
        .and_then(|v| v.as_str())
        .ok_or("its session.json doesn't say when it began")?
        .to_string();
    let began_unix = timegrapher_core::full_wind::unix_of_utc(&began_utc)
        .ok_or_else(|| format!("its start time {began_utc} can't be read"))?;
    let mut positions = Vec::new();
    for p in guided::Kind::Positions.order() {
        let want = timegrapher_core::session::Position::parse(p.code()).map(|q| q.common());
        let Some(e) = m.recordings.iter().find(|e| {
            e.position
                .as_deref()
                .and_then(timegrapher_core::session::Position::parse)
                .map(|q| q.common())
                == want
        }) else {
            continue;
        };
        let read = take::measure_entry(dir, m, e, settings, take::usb, |first| {
            take::correction_for(m, first, None, None)
        })
        .map_err(|e| e.to_string())?
        .measurement;
        positions.push(guided::KeptRow {
            position: p.code().to_string(),
            rate_s_per_day: read.rate_s_per_day,
            amplitude_deg: read.amplitude_deg,
            beat_error_ms: read.beat_error(),
            beat_error_source: read.beat_error_source,
        });
    }
    if positions.is_empty() {
        return Err("it holds none of the six positions".into());
    }
    Ok(guided::FullWind {
        format: guided::RESULTS_FORMAT.into(),
        began_utc,
        began_unix,
        watch: m.watch.clone().unwrap_or_default(),
        calibre: m.calibre.clone().unwrap_or_default(),
        bph: settings.stream.analysis.bph,
        lift_angle_deg: Some(settings.stream.analysis.amplitude.lift_deg),
        settle_s: m.settle_s,
        measure_s: m.measure_s,
        positions,
        wind_h: None,
        against: None,
    })
}

impl TimegrapherApp {
    /// Read the take in `dir` to compare with, in the background, as the
    /// take on show is read: its beat rate and lift.
    pub(super) fn read_compare_take(&mut self, dir: PathBuf) {
        let file = dir.join(take::MANIFEST);
        let m: Manifest = match std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|t| toml::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(m) => m,
            Err(e) => return self.error(format!("Couldn't load {}: {e}", file.display())),
        };
        let settings = self.entry_settings(&m);
        let name = dir
            .file_name()
            .map_or_else(|| dir.display().to_string(), |n| n.to_string_lossy().into());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(full_wind_of_take(&dir, &m, &settings));
        });
        self.compare_take = Some(Reading { name, rx });
    }

    /// The take being read to compare with, once read: set against, or
    /// said why not.
    pub(super) fn poll_compare_take(&mut self) {
        let Some(r) = &self.compare_take else {
            return;
        };
        let read = match r.rx.try_recv() {
            Ok(read) => read,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("its reading stopped".into()),
        };
        let name = r.name.clone();
        self.compare_take = None;
        match read {
            Ok(full) => {
                self.guided_earlier = Some(full);
                self.guided_earlier_file = true;
            }
            Err(e) => self.error(format!(
                "Couldn't read the take {name} to compare with: {e}"
            )),
        }
    }

    /// What Compare With shows: the take being read, else the test it
    /// is set against, from the watch's own or a file, else none.
    pub(super) fn compare_with_text(&mut self, ui: &egui::Ui) -> String {
        self.poll_compare_take();
        if let Some(r) = &self.compare_take {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
            return format!("Reading {}…", r.name);
        }
        match &self.guided_earlier {
            Some(e) if self.guided_earlier_file => {
                format!("{} (from a file)", guided::choice_text(e))
            }
            Some(e) => guided::choice_text(e),
            None if self.guided_choices.is_empty() => "None from 12 to 36 hours ago".to_string(),
            None => "Pick a full-wind test".to_string(),
        }
    }
}
