//! Keep in a Watch…'s copy of a take into a watch, on a thread of its own
//! with its progress shown and a Cancel (#1970): core plans it, the room
//! for it checked first ([`library::Library::plan_add`]), copies it
//! ([`library::copy_add`]) and finishes it ([`library::Library::finish_add`]).

use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;

/// A take being copied into a watch.
pub(super) struct TakeAdd {
    plan: library::AddPlan,
    /// The watch it goes into, by id.
    pub(super) watch: String,
    rx: Receiver<Result<PathBuf, String>>,
    /// Bytes copied so far.
    done: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
}

impl TakeAdd {
    /// How far it is: bytes copied, and all of them.
    pub(super) fn progress(&self) -> (u64, u64) {
        (self.done.load(Ordering::Relaxed), self.plan.bytes)
    }

    /// The take's folder outside the library, being copied.
    pub(super) fn from(&self) -> &Path {
        &self.plan.from
    }
}

impl TimegrapherApp {
    /// Add the take `from` to the watch `id`: moved there at once on the
    /// same disk, else copied on a thread of its own, once core has found
    /// room for it.
    pub(super) fn start_take_add(&mut self, from: PathBuf, id: String, keep_original: bool) {
        if self.take_add.is_some() {
            return self.info_msg(ADDING_ALREADY.into());
        }
        let Ok(Some(lib)) = &mut self.library else {
            return self.error("There is no Watches folder.".into());
        };
        let plan = match lib.plan_add(&from, &id, keep_original) {
            Ok(p) => p,
            Err(e) => return self.error(format!("The take couldn't be added: {e}")),
        };
        if let Some(to) = plan.moved().map(Path::to_path_buf) {
            let done = lib.finish_add(&plan, Ok(to));
            return self.take_added(&plan.from, id, done);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let done = Arc::new(AtomicU64::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (d2, c2, p2) = (done.clone(), cancel.clone(), plan.clone());
        std::thread::spawn(move || {
            let copied = library::copy_add(&p2, &mut |n| d2.store(n, Ordering::Relaxed), &c2);
            let _ = tx.send(copied);
        });
        self.take_add = Some(TakeAdd {
            plan,
            watch: id,
            rx,
            done,
            cancel,
        });
    }

    /// Stop the copy under way: what was copied goes, and the original
    /// is as it was.
    pub(super) fn cancel_take_add(&mut self) {
        if let Some(a) = &self.take_add {
            a.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// The copy, once it ends, finished: in the watch, or why not.
    pub(super) fn poll_take_add(&mut self) {
        let Some(a) = &self.take_add else {
            return;
        };
        let copied = match a.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("it stopped part way".into()),
        };
        let Some(a) = self.take_add.take() else {
            return;
        };
        let stopped = a.cancel.load(Ordering::Relaxed) && copied.is_err();
        let done = match &mut self.library {
            Ok(Some(lib)) => lib.finish_add(&a.plan, copied),
            _ => copied.map(|to| (to, None)),
        };
        if stopped {
            return self.info_msg(format!(
                "Stopped adding \"{}\": nothing was kept, and the original is as it was.",
                file_name(&a.plan.from)
            ));
        }
        self.take_added(&a.plan.from, a.watch, done);
    }

    /// Say how adding `from` to the watch `id` went, and show the watch.
    fn take_added(
        &mut self,
        from: &Path,
        id: String,
        done: Result<(PathBuf, Option<String>), String>,
    ) {
        match done {
            Ok((to, left)) => {
                // What is on show is now the watch's take, copied or
                // moved.
                self.take_moved(from, &to);
                self.watch_open = Some(id);
                self.unsorted_open = false;
                match left {
                    Some(why) => self.error(why),
                    None => self.info_msg(format!(
                        "Added \"{}\" to the watch's takes.",
                        file_name(&to)
                    )),
                }
            }
            Err(e) => self.error(format!("The take couldn't be added: {e}")),
        }
    }

    /// The copy under way into the watch `id`, if any: its progress bar,
    /// how far it is in words, and Cancel.
    pub(super) fn take_add_row(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(a) = self.take_add.as_ref().filter(|a| a.watch == id) else {
            return;
        };
        let (done, all) = a.progress();
        let words = adding_words(&file_name(a.from()), done, all);
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::ProgressBar::new((done as f64 / all.max(1) as f64) as f32)
                    .desired_width(160.0),
            );
            ui.label(words);
            cancel = ui
                .button("Cancel")
                .on_hover_text("Stop copying: nothing is kept, and the original is as it was")
                .clicked();
        });
        ui.ctx().request_repaint_after(Duration::from_millis(250));
        if cancel {
            self.cancel_take_add();
        }
    }
}

/// Why another take can't be added while one is copied in.
pub(super) const ADDING_ALREADY: &str =
    "A take is being copied into a watch. Wait for it to finish, or press Cancel.";

/// A copy's progress in words: "Copying "name": 4.2 GB of 11.2 GB".
pub(super) fn adding_words(name: &str, done: u64, all: u64) -> String {
    format!(
        "Copying \"{name}\": {} of {}",
        size_text(done),
        size_text(all)
    )
}
