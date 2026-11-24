//! Compare With from an ordinary take (#1851): a take that holds the six
//! positions, though not saved as a Positions Test, read as the test it
//! is set against. Each position is read as that take's own table reads
//! it, on the thread of its own a take's reading needs.

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
/// the take began: each of the six positions its session file lists, in
/// the test's order, read with `settings` by the one reading of a take's
/// positions its own This Take table shows when it is opened
/// ([`table_of_take`]): the same settled stretches, a position
/// listed twice read as one, the same figures to the bit (Norm on #1853).
pub(super) fn full_wind_of_take(
    dir: &Path,
    m: &Manifest,
    settings: take::EntrySettings,
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
    let (bph, lift) = (
        settings.stream.analysis.bph,
        settings.stream.analysis.amplitude.lift_deg,
    );
    let table = table_of_take(dir, m, settings)?;
    let positions = kept_rows(&table);
    if positions.is_empty() {
        return Err("it holds none of the six positions".into());
    }
    Ok(guided::FullWind {
        format: guided::RESULTS_FORMAT.into(),
        began_utc,
        began_unix,
        watch: m.watch.clone().unwrap_or_default(),
        calibre: m.calibre.clone().unwrap_or_default(),
        bph,
        lift_angle_deg: Some(lift),
        settle_s: m.settle_s,
        measure_s: m.measure_s,
        positions,
        wind_h: None,
        against: None,
        listened: false,
    })
}

/// The take in `base` read for its This Take table, as the window opens
/// and reads it, off the window's thread: core's `TakeContext::open`
/// with its masks kept, its engine readied by [`super::take_io::ready_engine`] on the
/// positions, settling and masks its session file gives, and the table
/// read by [`take_reads::read_table`] over each position's settled
/// stretches ([`take_reads::stretches_in`]). For Compare With reading
/// another take (#1851), so its rows are the bits that take's own table
/// shows when it is opened.
pub(super) fn table_of_take(
    base: &Path,
    m: &Manifest,
    settings: timegrapher_core::take::EntrySettings,
) -> Result<guided::FreeTable, String> {
    use timegrapher_core::take;
    let analysis = settings.stream.analysis.clone();
    let (o, log) = timegrapher_core::take_context::TakeContext::open(
        base,
        m,
        settings,
        timegrapher_core::take_context::Masks::Kept,
        take::usb,
        |first| take::correction_for(m, first, None, None),
        |_, _| {},
    )?;
    let (changes, settles) = o.positions(m);
    let stretches: Vec<(f64, Position)> = changes
        .iter()
        .filter_map(|&(a, p)| Some((a, Position::of_core(p)?)))
        .collect();
    let inputs = super::take_io::OpenInputs {
        bph: analysis.bph,
        lift_deg: analysis.amplitude.lift_deg,
        lift_known: analysis.lift_known,
        average_s: AVERAGES[0],
        masked: o.masked.clone(),
        changes: stretches.iter().map(|s| s.0).collect(),
        settles: settles.clone(),
    };
    let live = super::take_io::ready_engine(
        log,
        &inputs,
        &o.breaks,
        TimegrapherApp::rate_clock(&o.clock),
    );
    // The positions as the window loads them, and the watch in the first
    // until they say (`apply_manifest_placed`).
    let mut session = crate::session::Session::default();
    let now = super::take_io::load_positions(
        &mut session,
        &stretches,
        &settles,
        super::take_io::recorded_of(m),
        stretches.first().map_or(Position::DialUp, |s| s.1),
    );
    let end = live
        .beats()
        .last()
        .map_or(live.duration_s(), |b| b.time)
        .max(live.duration_s());
    let spans = take_reads::clipped(take_reads::stretches_in(&session, now), end);
    let log = steady::log_of(&live).ok_or("its beat rate isn't known")?;
    let clock = reading_clock(o.clock.as_ref(), true, None, log.duration_s);
    let breaks = steady::reading_breaks(o.breaks, &log);
    // The masks as the window keeps them (`put_masked`) and the table
    // reads them.
    let masked = timegrapher_core::session::masked_spans(
        &o.masked.iter().map(|&(a, b)| [a, b]).collect::<Vec<_>>(),
    );
    Ok(take_reads::read_table(
        &log,
        clock.as_ref(),
        &breaks,
        &masked,
        &spans,
        live.config().analysis.bph,
    ))
}

/// The six positions of a take's table that were read, in the test's
/// order, as a saved test keeps them.
pub(super) fn kept_rows(table: &guided::FreeTable) -> Vec<guided::KeptRow> {
    guided::Kind::Positions
        .order()
        .iter()
        .filter_map(|p| {
            let want = timegrapher_core::session::Position::parse(p.code())?;
            let r = table.six.0.iter().find(|r| r.position == want)?;
            let m = &r.measurement;
            Some(guided::KeptRow {
                position: p.code().to_string(),
                rate_s_per_day: m.rate_s_per_day,
                amplitude_deg: m.amplitude_deg,
                beat_error_ms: m.beat_error(),
                beat_error_source: m.beat_error_source,
            })
        })
        .collect()
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
            let _ = tx.send(full_wind_of_take(&dir, &m, settings));
        });
        self.test.compare_take = Some(Reading { name, rx });
    }

    /// The take being read to compare with, once read: set against, or
    /// said why not.
    pub(super) fn poll_compare_take(&mut self) {
        let Some(r) = &self.test.compare_take else {
            return;
        };
        let read = match r.rx.try_recv() {
            Ok(read) => read,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("its reading stopped".into()),
        };
        let name = r.name.clone();
        self.test.compare_take = None;
        match read {
            Ok(full) => {
                self.test.guided_earlier = Some(full);
                self.test.guided_earlier_file = true;
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
        if let Some(r) = &self.test.compare_take {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
            return format!("Reading {}…", r.name);
        }
        match &self.test.guided_earlier {
            Some(e) if self.test.guided_earlier_file => {
                format!("{} (from a file)", guided::choice_text(e))
            }
            Some(e) => guided::choice_text(e),
            None if self.test.guided_choices.is_empty() => {
                "None from 12 to 36 hours ago".to_string()
            }
            None => "Pick a full-wind test".to_string(),
        }
    }
}
