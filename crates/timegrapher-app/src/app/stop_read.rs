//! A take just stopped, read as opening its saved take reads it (#2151,
//! Norm's ruling B): at Stop, the sound kept so far is read whole on a
//! thread of its own, with the session file a save would write, by core's
//! `TakeContext::open` as an open reads it. Until it is read, the reading
//! cards show the open's own Analysing wait (Hugh); then the cards, their
//! sparklines and the whole take read that reading, and nothing else on
//! show is read again (Gordon). Listening again, or anything that ends
//! the paused take, drops it.

use super::*;
use crate::app::take_io::{manifest_positions, open_in_background, OpenInputs, Opened};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Arc;

/// The take just stopped being read whole, then read.
pub(in crate::app) struct StopRead {
    rx: std::sync::mpsc::Receiver<Result<Option<Opened>, String>>,
    /// Seconds of sound read so far.
    pub(in crate::app) progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
    /// The sound kept at the Stop, seconds: what it reads.
    pub(in crate::app) duration_s: f64,
    pub(in crate::app) started: Instant,
    /// What the wait calls the take.
    pub(in crate::app) label: String,
    /// The take read, once it is.
    read: Option<Box<StoppedTake>>,
}

impl Drop for StopRead {
    fn drop(&mut self) {
        self.cancel.store(true, Relaxed);
    }
}

/// A stopped take as opening its saved take reads it: its engine on the
/// saved take's clock and breaks, in the saved sound's own time.
pub(in crate::app) struct StoppedTake {
    pub(in crate::app) live: LiveAnalyzer,
    inputs: OpenInputs,
    pub(in crate::app) clock: Option<timegrapher_core::clock::ClockFit>,
    pub(in crate::app) breaks: Vec<timegrapher_core::breaks::Break>,
    pub(in crate::app) unlisted: Option<timegrapher_core::breaks::Unlisted>,
}

impl TimegrapherApp {
    /// Read the take just stopped as its saved take opens (see the
    /// module): its sound kept so far, with the session file a save would
    /// write now. None where it keeps no sound, or core can't read it so;
    /// the cards then read the live engine as before.
    pub(in crate::app) fn start_stop_read(&mut self) {
        self.bg.stop_read = None;
        if !self.session.paused() {
            return;
        }
        let Some(r) = self.session.recorder.as_ref().filter(|r| r.keeps_sound()) else {
            return;
        };
        let (dir, duration_s) = (r.dir().to_path_buf(), r.duration_s());
        let m = self.take_manifest();
        let entries = m.recordings.clone();
        let opening = match take_entry(&dir, Some(&(m, entries))) {
            Ok(o) => o,
            Err(e) => return self.stop_read_failed(&e.to_string()),
        };
        let inputs = ready_engine_inputs(self.open_inputs(&[], &[]), &opening.1, duration_s);
        let settings = self.entry_settings(&opening.1);
        let (tx, rx) = std::sync::mpsc::channel();
        let progress = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        open_in_background(
            opening,
            settings,
            tx,
            progress.clone(),
            cancel.clone(),
            None,
            inputs,
        );
        let label = match self.session.saved_to() {
            Some(p) => capture_label(p),
            None => "the take".to_string(),
        };
        self.bg.stop_read = Some(StopRead {
            rx,
            progress,
            cancel,
            duration_s,
            started: Instant::now(),
            label,
            read: None,
        });
    }

    /// Whether the read of the take just stopped still holds: the take
    /// paused with the sound it read.
    fn stop_read_holds(&self) -> bool {
        self.session.paused()
            && self.bg.stop_read.as_ref().is_some_and(|s| {
                self.session
                    .recorder
                    .as_ref()
                    .is_some_and(|r| r.duration_s() == s.duration_s)
            })
    }

    /// Take in the read of the take just stopped once it is done; drop it
    /// once it no longer holds. On the window's frame.
    pub(in crate::app) fn poll_stop_read(&mut self) {
        if self.bg.stop_read.is_none() {
            return;
        }
        if !self.stop_read_holds() {
            self.bg.stop_read = None;
            return;
        }
        let length_s = self.bg.stop_read.as_ref().map_or(0.0, |s| s.duration_s);
        let now = ready_engine_inputs(self.open_inputs(&[], &[]), &self.take_manifest(), length_s);
        let Some(s) = self.bg.stop_read.as_mut() else {
            return;
        };
        if let Some(t) = s.read.as_mut() {
            // A setting, a mask or a position changed while stopped, as an
            // opened take's engine takes it.
            t.retune(now);
            return;
        }
        let done = match s.rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("it stopped part way".into()),
        };
        match done {
            Ok(Some(o)) => s.read = Some(Box::new(StoppedTake::of(o))),
            Ok(None) => self.bg.stop_read = None,
            Err(e) => {
                self.bg.stop_read = None;
                self.stop_read_failed(&e);
            }
        }
    }

    /// The take just stopped, read as its saved take opens, once read.
    pub(in crate::app) fn stopped_take(&self) -> Option<&StoppedTake> {
        self.stop_read_holds()
            .then(|| self.bg.stop_read.as_ref()?.read.as_deref())
            .flatten()
    }

    /// The take just stopped couldn't be read whole, for `e`: the cards
    /// read the live engine, as before, and say so (Hugh on #2164).
    fn stop_read_failed(&mut self, e: &str) {
        self.error(format!(
            "The take stopped couldn't be analysed whole\nThe cards show the figures heard \
             while listening ({e}). Saved and opened again, the take may read slightly \
             differently."
        ));
    }

    /// A Stop read that stops part way, as one that fails does.
    #[cfg(test)]
    pub(in crate::app) fn break_stop_read(&mut self) {
        if let Some(s) = self.bg.stop_read.as_mut() {
            s.rx = std::sync::mpsc::channel().1;
        }
    }

    /// The live engine, unless the take just stopped is still being read:
    /// the cards wait for that rather than read live's figures.
    pub(in crate::app) fn live_unless_stop_read_waits(&self) -> Option<&LiveAnalyzer> {
        self.live
            .as_ref()
            .filter(|_| self.stop_read_waits().is_none())
    }

    /// The take just stopped, still being read: the cards wait for it.
    pub(in crate::app) fn stop_read_waits(&self) -> Option<&StopRead> {
        self.bg
            .stop_read
            .as_ref()
            .filter(|s| s.read.is_none() && self.stop_read_holds())
    }
}

impl StoppedTake {
    fn of(o: Opened) -> Self {
        let Opened {
            live,
            inputs,
            clock,
            breaks,
            unlisted,
            ..
        } = o;
        StoppedTake {
            live,
            inputs,
            clock,
            breaks,
            unlisted,
        }
    }

    /// What is masked, in the saved sound's time.
    pub(in crate::app) fn masked(&self) -> &[(f64, f64)] {
        &self.inputs.masked
    }

    /// Bring the engine to `now`, as an opened take's takes a change.
    fn retune(&mut self, now: OpenInputs) {
        if now != self.inputs {
            crate::app::take_io::retune(&mut self.live, &self.inputs, &now, &self.breaks);
            self.inputs = now;
        }
    }
}

/// `window`'s settings with the positions, settles and masks the session
/// file `m` gives a take `length_s` long, as opening it reads them.
fn ready_engine_inputs(window: OpenInputs, m: &Manifest, length_s: f64) -> OpenInputs {
    let entries = &m.recordings;
    let masked =
        timegrapher_core::take_context::masks_in_take(entries, entries.iter().map(|_| 0.0));
    let (stretches, settles) = manifest_positions(m, entries, Some(length_s), None);
    OpenInputs {
        masked: timegrapher_core::session::masked_spans(&masked),
        changes: stretches.iter().map(|s| s.0).collect(),
        settles,
        ..window
    }
}
