//! A finished guided test read again from its take's sound as saved,
//! exactly as `timegrapher session` reads that take, so its table and its
//! saved results are the figures a reopen and the command line give
//! (#1958). One read of the saved sound, by core's own reading of each
//! entry ([`take_context::measure_entries`]). And its marks kept with its take
//! ([`test_marks::FILE`]), brought back when the take is opened, so a
//! reopened test shows its Settling, Before Test and Not Used as it did
//! while it ran (#1957).

use super::*;
use timegrapher_core::session::Measurement;
use timegrapher_core::take;
use timegrapher_core::test_marks::{self, Mark, Marked, TestMarks};

/// What a test's read again gives: for each position measured, in order,
/// its seconds measured, beat rate and measurement; or why it failed.
type Saved = Result<Vec<(f64, u32, Measurement)>, String>;

/// The status bar once a finished test's saved take is analysed again
/// (Hugh 5 on #2067).
pub(super) const SAVED_READ_DONE: &str =
    "Analysed the saved take again: the test's figures are updated.";

/// A finished test's read again from its saved take, under way off the
/// window's thread.
pub(super) struct TestRead {
    /// When the test it reads began, so its figures go to no other.
    began_unix: f64,
    /// The take it reads, where its results are written again.
    dir: PathBuf,
    rx: std::sync::mpsc::Receiver<Saved>,
}

/// Each entry of the take saved in `dir`, read as `timegrapher session`
/// reads it on `settings`.
fn read_saved(dir: &Path, m: &Manifest, settings: &take::EntrySettings) -> Saved {
    timegrapher_core::take_context::measure_entries(
        dir,
        m,
        settings,
        take::usb,
        |first| take::correction_for(m, first, None, None),
        |_, _| {},
    )
    .into_iter()
    .map(|r| {
        let r = r.map_err(|e| e.to_string())?;
        Ok((r.stop_s - r.start_s, r.log.bph, r.measurement))
    })
    .collect()
}

impl TimegrapherApp {
    /// Once a take is saved into `dir`, not streamed there: a finished
    /// test whose figures are still the listening's is read again from
    /// that sound as saved, in the background. A save again while it is
    /// read starts the reading over on what that save wrote (Vernon on
    /// #2067), and a full save after Save Analysis Only reads it from the
    /// sound that one keeps (Hugh 5 on #2080).
    pub(super) fn read_test_again(&mut self, dir: &Path, streamed: super::Streamed) {
        if streamed != super::Streamed::No {
            return;
        }
        let Ok(Some((m, _))) = read_manifest(dir) else {
            return;
        };
        let settings = self.entry_settings(&m);
        let Some(t) = self.test.guided.as_mut().filter(|t| {
            matches!(
                t.saved_read,
                guided::SavedRead::Pending
                    | guided::SavedRead::Reading
                    | guided::SavedRead::NoSound
            )
        }) else {
            return;
        };
        t.saved_read = guided::SavedRead::Reading;
        let (tx, rx) = std::sync::mpsc::channel();
        let read_dir = dir.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("test read again".into())
            .spawn(move || {
                let _ = tx.send(read_saved(&read_dir, &m, &settings));
            });
        if let Err(e) = spawned {
            t.saved_read = guided::SavedRead::Failed(e.to_string());
            return;
        }
        self.bg.test_read = Some(TestRead {
            began_unix: t.began_unix,
            dir: dir.to_path_buf(),
            rx,
        });
    }

    /// The test read again, once it is in: its figures replace the
    /// listening's, and its results are written into its take again.
    /// Where it failed, the table and the results say the figures are
    /// the listening's.
    pub(super) fn poll_test_read(&mut self) {
        use std::sync::mpsc::TryRecvError;
        // Nothing lands while a save is written: one of the same take
        // starts the reading over once it is done.
        let Some(job) = self.bg.test_read.as_ref().filter(|_| self.saving.is_none()) else {
            return;
        };
        let saved = match job.rx.try_recv() {
            Ok(s) => s,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("it stopped part way".to_string()),
        };
        let Some(TestRead {
            began_unix, dir, ..
        }) = self.bg.test_read.take()
        else {
            return;
        };
        let Some(t) =
            self.test.guided.as_mut().filter(|t| {
                t.began_unix == began_unix && t.saved_read == guided::SavedRead::Reading
            })
        else {
            return;
        };
        let read = saved.and_then(|s| {
            t.finish_saved(s).then_some(()).ok_or_else(|| {
                "its session file lists other stretches than the test measured".into()
            })
        });
        match read {
            Ok(()) => self.info_msg(SAVED_READ_DONE.into()),
            Err(why) => t.saved_read = guided::SavedRead::Failed(why),
        }
        let dir = self.results_dir(dir);
        self.write_results_into(&dir);
    }

    /// A finished test's take kept by Save Analysis Only: with no sound to
    /// read it again from, its table says its figures are the listening's
    /// (#2070). One read again already, or under way, is left as it is.
    pub(super) fn test_kept_without_sound(&mut self) {
        if let Some(t) = self
            .test
            .guided
            .as_mut()
            .filter(|t| t.saved_read == guided::SavedRead::Pending)
        {
            t.saved_read = guided::SavedRead::NoSound;
        }
    }

    /// Where a test read again from the take saved in `read` writes its
    /// results: where the take is now, which may have moved while it was
    /// read (Desmond on #2067).
    pub(super) fn results_dir(&self, read: PathBuf) -> PathBuf {
        self.session.saved_to().map_or(read, Path::to_path_buf)
    }

    /// The test's results written into the take saved in `dir` again, each
    /// file whole or not at all.
    fn write_results_into(&mut self, dir: &Path) {
        let written = self.results_files().and_then(|files| {
            files.iter().try_for_each(|(name, bytes)| {
                timegrapher_core::library::write_atomic(&dir.join(name), bytes)
                    .map_err(|e| format!("Couldn't write {name}: {e}"))
            })
        });
        match written {
            // The take as saved holds these results now (#2106).
            Ok(()) if self.session.saved_to() == Some(dir) => {
                let kept = self.save_metadata().full_wind;
                self.session.results_saved(kept);
            }
            Ok(()) => {}
            Err(e) => self.error(e),
        }
    }
}

/// The guided test kind a take's marks name, and back.
fn test_of(kind: guided::Kind) -> test_marks::Test {
    match kind {
        guided::Kind::Positions => test_marks::Test::Positions,
        guided::Kind::Poise => test_marks::Test::DynamicPoise,
        guided::Kind::Complication => test_marks::Test::Complication,
    }
}

fn kind_of(test: test_marks::Test) -> guided::Kind {
    match test {
        test_marks::Test::Positions => guided::Kind::Positions,
        test_marks::Test::DynamicPoise => guided::Kind::Poise,
        test_marks::Test::Complication => guided::Kind::Complication,
    }
}

/// What a stretch the test marked was, from the words the strip gives it.
fn mark_of(words: &str) -> Mark {
    match words {
        guided::BEFORE_TEST => Mark::BeforeTest,
        guided::SETTLING => Mark::Settling,
        _ => Mark::NotUsed,
    }
}

/// The words the strip gives a stretch the test `kind` marked `mark`.
fn words_of(mark: Mark, kind: guided::Kind) -> &'static str {
    match mark {
        Mark::BeforeTest => guided::BEFORE_TEST,
        Mark::Settling => guided::SETTLING,
        Mark::NotUsed => kind.not_used(),
    }
}

/// The marks `marks` kept with a take, as the strip shows a test's.
fn unused_of(marks: &TestMarks) -> Vec<(f64, f64, &'static str)> {
    let kind = kind_of(marks.test);
    marks
        .marks
        .iter()
        .map(|m| (m.start_s, m.end_s, words_of(m.mark, kind)))
        .collect()
}

impl TimegrapherApp {
    /// The test marks file among a take's results files, so the test opens
    /// again as it ran (#1957).
    pub(super) fn test_marks_file(&self) -> Option<(&'static str, Vec<u8>)> {
        let m = self.test_marks()?;
        Some((timegrapher_core::test_marks::FILE, m.to_json().into_bytes()))
    }

    /// The guided test's marks for its take's files, in the saved sound's
    /// own seconds; none with no test, or nothing yet in the sound.
    pub(super) fn test_marks(&self) -> Option<TestMarks> {
        let t = self.test.guided.as_ref()?;
        let now = self.live.as_ref().map_or(0.0, |l| l.duration_s());
        let audio = |a: f64, b: f64| -> Option<(f64, f64)> {
            let (a, b) = (self.session.audio_of(a)?, self.session.audio_of(b)?);
            (b > a).then_some((a, b))
        };
        let marks: Vec<Marked> = t
            .unused(now)
            .into_iter()
            .filter_map(|(a, b, words)| {
                let (start_s, end_s) = audio(a, b)?;
                Some(Marked {
                    start_s,
                    end_s,
                    mark: mark_of(words),
                })
            })
            .collect();
        let measured: Vec<(f64, f64)> = t
            .measured(now)
            .into_iter()
            .filter_map(|(a, b)| audio(a, b))
            .collect();
        (!marks.is_empty() || !measured.is_empty())
            .then(|| TestMarks::new(test_of(t.kind), marks, measured))
    }

    /// A take opened from `dir`: its guided test's marks back on the
    /// strip, where it kept them. One whose file can't be read says so
    /// and opens as a take without them.
    pub(super) fn load_test_marks(&mut self, dir: &Path) {
        match TestMarks::read(dir) {
            Ok(Some(m)) => {
                self.test.guided_unused = unused_of(&m);
                self.test.guided_measured = m.measured;
                self.bg.opened_marks = Some(self.test.guided_unused.clone());
            }
            Ok(None) => {}
            Err(e) => self.info_msg(format!("The test's marks weren't read: {e}")),
        }
    }

    /// Whether the test marks on show came with a take opened, not from a
    /// test on the Bench: what each stretch was is then the marks' word,
    /// over the settling the session file's gaps give.
    pub(super) fn marks_opened(&self) -> bool {
        self.test.guided.is_none()
            && !self.test.guided_unused.is_empty()
            && self.bg.opened_marks.as_ref() == Some(&self.test.guided_unused)
    }
}
