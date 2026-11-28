//! The takes' own readings the Watches list shows, worked out by core's
//! `library::reading` on threads of their own, each asked again when the
//! library's [`ReadingsRevision`] changes (#2410).

use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use timegrapher_core::library::{Reading, ReadingsRevision, Take};

/// Where the Watches list is with a take's own reading (core
/// `library::reading`).
#[derive(Debug, Clone, PartialEq)]
pub enum TakeReading {
    /// Being worked out.
    Reading,
    /// Worked out, or why it couldn't be.
    Read(Result<Reading, String>),
}

/// The takes' own readings the Watches list shows, each worked out once
/// by core's `library::reading`, as `timegrapher watches` gives them. They
/// are worked out one take at a time on a thread of their own, as the
/// first read of a long take reads its whole sound (about a minute for
/// 9 h); a reading already kept comes back at once.
#[derive(Default)]
pub struct TakeReadings {
    /// Each take asked for, by its folder, with the library's
    /// [`ReadingsRevision`] when asked: at a new one (the library read
    /// again, a clock correction or calibre changed) it is asked again,
    /// and core says whether what its reading reads has changed.
    known: HashMap<PathBuf, (ReadingsRevision, TakeReading)>,
    /// The takes asked for whose answer to the last ask hasn't come in:
    /// one asked again keeps its old figures on show meanwhile, so it
    /// isn't told by its [`TakeReading`] alone.
    pending: HashSet<PathBuf>,
    work: Option<(mpsc::Sender<Job>, mpsc::Receiver<ReadDone>)>,
    /// The take being read now, whose sound is open: it can't be moved,
    /// renamed or deleted until it is done (Windows refuses to).
    now: std::sync::Arc<std::sync::Mutex<Option<PathBuf>>>,
}

/// A take's folder to read, with the revision it was asked at.
type Job = (PathBuf, ReadingsRevision);

/// A take's folder with the revision it was asked at, and its reading or
/// why it couldn't be worked out.
type ReadDone = (PathBuf, ReadingsRevision, Result<Reading, String>);

impl TakeReadings {
    /// Ask for the reading of the take in `dir` at the library's
    /// revision `at`, unless it is already asked for at that one; `ctx`
    /// is drawn again as each comes in.
    pub fn want(&mut self, dir: &Path, at: &ReadingsRevision, ctx: &egui::Context) {
        if self.known.get(dir).is_some_and(|(r, _)| r == at) {
            return;
        }
        let (tx, _) = self.work.get_or_insert_with(|| {
            let (tx, jobs) = mpsc::channel::<Job>();
            let (done, rx) = mpsc::channel();
            // A take whose reading is kept is answered on a thread of its
            // own, at once, not left blank behind a take still being read.
            let (to_read, reads) = mpsc::channel::<Job>();
            let (kept_done, kept_ctx) = (done.clone(), ctx.clone());
            std::thread::spawn(move || {
                for (dir, at) in jobs {
                    let sent = match timegrapher_core::library::kept_reading(&dir) {
                        Some(r) => kept_done.send((dir, at, Ok(r))).is_ok(),
                        None => to_read.send((dir, at)).is_ok(),
                    };
                    if !sent {
                        break;
                    }
                    kept_ctx.request_repaint();
                }
            });
            let ctx = ctx.clone();
            let now = self.now.clone();
            std::thread::spawn(move || {
                for (dir, at) in reads {
                    *now.lock().unwrap() = Some(dir.clone());
                    let r = timegrapher_core::library::reading(&dir);
                    *now.lock().unwrap() = None;
                    if done.send((dir, at, r)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
            (tx, rx)
        });
        if tx.send((dir.to_path_buf(), at.clone())).is_ok() {
            // A take read before keeps its figures on show until it is read
            // again (at once, when what the reading reads hasn't changed).
            let was = self.known.remove(dir).map(|(_, r)| r);
            let now = match was {
                Some(r @ TakeReading::Read(Ok(_))) => r,
                _ => TakeReading::Reading,
            };
            self.known.insert(dir.to_path_buf(), (at.clone(), now));
            self.pending.insert(dir.to_path_buf());
        } else {
            // The thread has gone: a new one next time.
            self.work = None;
        }
    }

    /// Take in the readings worked out since last time.
    pub fn poll(&mut self) {
        let Some((_, rx)) = &self.work else { return };
        let done: Vec<ReadDone> = rx.try_iter().collect();
        for (dir, at, r) in done {
            self.take_in(&dir, &at, r);
        }
    }

    /// Take in the reading of the take in `dir` asked for at `at`: kept
    /// only while the take is still asked for at that revision. One asked
    /// for before the take was asked again (its session file edited
    /// meanwhile, say) is dropped, as the newer answer follows it.
    fn take_in(&mut self, dir: &Path, at: &ReadingsRevision, r: Result<Reading, String>) {
        if let Some((asked, s)) = self.known.get_mut(dir) {
            if asked == at {
                *s = TakeReading::Read(r);
                self.pending.remove(dir);
            }
        }
    }

    /// Where the reading of the take in `dir` is, if it was asked for.
    pub fn of(&self, dir: &Path) -> Option<&TakeReading> {
        self.known.get(dir).map(|(_, r)| r)
    }

    /// The reading of the take in `dir`, once worked out.
    pub fn reading(&self, dir: &Path) -> Option<&Reading> {
        match self.of(dir) {
            Some(TakeReading::Read(Ok(r))) => Some(r),
            _ => None,
        }
    }

    /// Whether the take in `dir`, or one inside it, is being read now.
    pub fn being_read(&self, dir: &Path) -> bool {
        self.now
            .lock()
            .unwrap()
            .as_deref()
            .is_some_and(|p| p.starts_with(dir))
    }

    /// Say the take in `dir` is being read now, or none.
    #[cfg(test)]
    pub fn set_now(&self, dir: Option<PathBuf>) {
        *self.now.lock().unwrap() = dir;
    }

    /// Wait until the answer to every ask is in, a take asked again
    /// included.
    #[cfg(test)]
    pub fn wait(&mut self) {
        for _ in 0..1200 {
            self.poll();
            if self.pending.is_empty() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("the takes' readings never came in");
    }

    /// Whether the take's figures come from its own reading: not a test's,
    /// whose figures are its Mean row or hour by hour.
    pub fn read_for(t: &Take, dir: &Path) -> bool {
        t.power_reserve.is_none() && timegrapher_core::library::take_test(dir).is_none()
    }

    #[cfg(test)]
    pub fn known(takes: impl IntoIterator<Item = (PathBuf, TakeReading)>) -> TakeReadings {
        TakeReadings {
            known: takes
                .into_iter()
                .map(|(d, r)| (d, (ReadingsRevision::default(), r)))
                .collect(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timegrapher_core::library::Library;

    /// A library in a folder of its own, for its readings revisions.
    fn library(name: &str) -> (Library, PathBuf) {
        let base = std::env::temp_dir().join(format!("tg-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let lib = Library::create(&base.join("Timegrapher Data")).unwrap();
        (lib, base)
    }
    /// A take's reading is worked out on its own thread: asked for, it is
    /// being read, then read, or why not; asked again at the same
    /// revision it isn't read again, and after the library is read again
    /// it is.
    #[test]
    fn a_takes_reading_is_worked_out_on_its_own_thread() {
        let (mut lib, base) = library("no-take");
        let dir = base.join("no take");
        let one = lib.readings_revision();
        assert_eq!(lib.readings_revision(), one, "nothing changed");
        lib.scan();
        let two = lib.readings_revision();
        assert_ne!(two, one, "read again");
        let ctx = egui::Context::default();
        let mut x = TakeReadings::default();
        assert_eq!(x.of(&dir), None);
        x.want(&dir, &one, &ctx);
        assert_eq!(x.of(&dir), Some(&TakeReading::Reading));
        let read = |x: &mut TakeReadings| {
            for _ in 0..500 {
                x.poll();
                if let Some(TakeReading::Read(r)) = x.of(&dir) {
                    return r.clone();
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("never read");
        };
        // No take there: why not, in core's words.
        let why = read(&mut x).unwrap_err();
        assert!(why.contains("session.toml"), "{why}");
        assert_eq!(x.reading(&dir), None);
        x.want(&dir, &one, &ctx);
        assert!(matches!(x.of(&dir), Some(TakeReading::Read(Err(_)))));
        x.want(&dir, &two, &ctx);
        assert_eq!(x.of(&dir), Some(&TakeReading::Reading));
        read(&mut x).unwrap_err();
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A take whose reading is kept comes back while another take is still
    /// being read, not after it.
    #[test]
    fn a_kept_reading_comes_back_while_another_take_is_read() {
        use timegrapher_core::synth::{generate, SynthConfig};
        let base = std::env::temp_dir().join(format!("tg-app-kept-first-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let kept = base.join("kept");
        std::fs::create_dir_all(&kept).unwrap();
        std::fs::write(
            kept.join(timegrapher_core::take::MANIFEST),
            "bph = 28800\n\n[[recording]]\nfile = \".\"\nposition = \"DU\"\n",
        )
        .unwrap();
        let audio = generate(
            &SynthConfig {
                duration_s: 20.0,
                ..Default::default()
            },
            |_| 270.0,
            |_| 0.0,
        );
        timegrapher_core::audio::write_wav(&kept.join("audio-001.wav"), &audio).unwrap();
        let want = timegrapher_core::library::reading(&kept).unwrap();
        let at = ReadingsRevision::default();
        let ctx = egui::Context::default();
        let mut x = TakeReadings::default();
        // Hold the reading thread at the start of its first take.
        let held = x.now.clone();
        let guard = held.lock().unwrap();
        let slow = base.join("slow");
        x.want(&slow, &at, &ctx);
        // Kept again just before it is asked for, in case another test's
        // reading rewrote the readings file meanwhile.
        assert_eq!(timegrapher_core::library::reading(&kept).unwrap(), want);
        x.want(&kept, &at, &ctx);
        for _ in 0..500 {
            x.poll();
            if x.reading(&kept).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(x.reading(&kept), Some(&want));
        assert_eq!(x.of(&slow), Some(&TakeReading::Reading));
        drop(guard);
        x.wait();
        assert!(matches!(x.of(&slow), Some(TakeReading::Read(Err(_)))));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A reading asked for at an earlier revision, coming in after the
    /// take was asked again, is dropped: the row waits for the answer
    /// to the newer ask, then shows it (#2410).
    #[test]
    fn a_reading_asked_for_before_the_take_was_asked_again_is_dropped() {
        let (mut lib, base) = library("superseded");
        let dir = base.join("take");
        let old = lib.readings_revision();
        lib.scan();
        let new = lib.readings_revision();
        let at = |deg| Reading {
            amplitude_deg: Some(deg),
            ..Default::default()
        };
        let mut x = TakeReadings::known([(dir.clone(), TakeReading::Reading)]);
        x.known.get_mut(&dir).unwrap().0 = new.clone();
        x.take_in(&dir, &old, Ok(at(271.98)));
        assert_eq!(x.of(&dir), Some(&TakeReading::Reading));
        x.take_in(&dir, &new, Ok(at(257.15)));
        assert_eq!(x.reading(&dir), Some(&at(257.15)));
        // Nor is a take never asked for given one.
        x.take_in(&base.join("other"), &new, Ok(at(250.0)));
        assert_eq!(x.of(&base.join("other")), None);
        let _ = std::fs::remove_dir_all(&base);
    }
}
