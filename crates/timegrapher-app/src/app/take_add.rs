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
        let Some(mut a) = self.take_add.take() else {
            return;
        };
        let stopped = a.cancel.load(Ordering::Relaxed) && copied.is_err();
        // A move's original is deleted only when nothing uses it now: the
        // take on show may be played or read while it is copied (Desmond
        // on #2020).
        let in_use = !a.plan.keep_original && self.take_busy(&a.plan.from, false).is_some();
        a.plan.keep_original |= in_use;
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
        if in_use {
            return self.added_in_use(a.watch, done);
        }
        self.take_added(&a.plan.from, a.watch, done);
    }

    /// A move copied in whose original was in use when the copy ended:
    /// the watch shown, and the original said to be left where it was.
    /// What is on show stays the original, in use.
    fn added_in_use(&mut self, id: String, done: Result<(PathBuf, Option<String>), String>) {
        match done {
            Ok((to, _)) => {
                self.watch_open = Some(id);
                self.unsorted_open = false;
                self.info_msg(in_use_words(&file_name(&to)));
            }
            Err(e) => self.error(format!("The take couldn't be added: {e}")),
        }
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
    "A take is being copied into a watch. Wait for it to finish, or press Cancel in that watch.";

/// A move finished as a copy, its original in use.
pub(super) fn in_use_words(name: &str) -> String {
    format!(
        "Copied \"{name}\" into the watch. The original was in use, so it was left where it \
         is: delete it there once you're done with it."
    )
}

/// A copy's progress in words: "Copying "name": 4.2 GB of 11.2 GB".
pub(super) fn adding_words(name: &str, done: u64, all: u64) -> String {
    format!(
        "Copying \"{name}\": {} of {}",
        size_text(done),
        size_text(all)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app with a library holding the watch "Pepsi", and a take folder
    /// outside it, in `base`.
    fn with_take(tag: &str) -> (TimegrapherApp, PathBuf, String, PathBuf) {
        let base = std::env::temp_dir().join(format!("tg-app-add-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("Timegrapher Data");
        let mut lib = library::Library::create(&root).unwrap();
        lib.add(&library::Watch::new("Pepsi")).unwrap();
        let id = lib.find("Pepsi").unwrap().watch.id.clone();
        drop(lib);
        let from = base.join("2026-10-22 10.00 Dial Up");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::write(
            from.join("session.toml"),
            "watch = \"Pepsi\"\nbph = 28800\n\n[[recording]]\n\
             file = \"take_part01-001.flac\"\nposition = \"DU\"\n",
        )
        .unwrap();
        std::fs::write(from.join("take_part01-001.flac"), vec![7u8; 3 << 20]).unwrap();
        let mut app = TimegrapherApp::with_devices(Vec::new(), None);
        app.use_library(Some(root));
        (app, base, id, from)
    }

    fn lib(app: &mut TimegrapherApp) -> &mut library::Library {
        match &mut app.library {
            Ok(Some(l)) => l,
            _ => panic!("no library"),
        }
    }

    /// The copy's end, as each frame polls for it.
    fn wait(app: &mut TimegrapherApp) {
        for _ in 0..1000 {
            app.poll_take_add();
            if app.take_add.is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the copy never ended");
    }

    /// A copy the poll is told has ended with `copied`, as its thread
    /// would tell it.
    fn ended(
        app: &mut TimegrapherApp,
        plan: library::AddPlan,
        id: &str,
        copied: Result<PathBuf, String>,
        stop: bool,
    ) {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(copied).unwrap();
        app.take_add = Some(TakeAdd {
            plan,
            watch: id.to_string(),
            rx,
            done: Arc::new(AtomicU64::new(0)),
            cancel: Arc::new(AtomicBool::new(stop)),
        });
        app.poll_take_add();
        assert!(app.take_add.is_none());
    }

    /// Keep in a Watch… copies on a thread of its own: a second add waits
    /// for it, and so does the take being copied; it ends in the watch.
    /// Stopped, nothing is kept. A move whose original is in use when the
    /// copy ends keeps the original, and says so (Desmond on #2020).
    #[test]
    fn a_take_is_copied_in_on_a_thread_of_its_own() {
        let (mut app, base, id, from) = with_take("thread");
        app.start_take_add(from.clone(), id.clone(), true);
        assert!(app.take_add.is_some(), "{:?}", app.message);
        assert_eq!(app.take_busy(&from, true).as_deref(), Some(ADDING_ALREADY));
        app.start_take_add(from.clone(), id.clone(), true);
        assert_eq!(app.message.clone().unwrap().0, ADDING_ALREADY);
        wait(&mut app);
        let m = app.message.clone().unwrap().0;
        assert!(m.starts_with("Added \""), "{m}");
        assert_eq!(app.watch_open.as_deref(), Some(id.as_str()));
        assert_eq!(lib(&mut app).find("Pepsi").unwrap().takes.len(), 1);
        assert!(from.is_dir());

        // Stopped: nothing kept, the original as it was.
        let plan = lib(&mut app).plan_add(&from, &id, true).unwrap();
        ended(&mut app, plan, &id, Err("Copying: stopped".into()), true);
        let m = app.message.clone().unwrap().0;
        assert!(m.starts_with("Stopped adding \""), "{m}");
        assert_eq!(lib(&mut app).find("Pepsi").unwrap().takes.len(), 1);
        assert!(from.is_dir());

        // A move copied to another disk, its original read for the Watches
        // list as the copy ends: kept, and said.
        let never = AtomicBool::new(false);
        let mut plan = lib(&mut app).plan_add(&from, &id, true).unwrap();
        plan.keep_original = false;
        let copied = library::copy_add(&plan, &mut |_| {}, &never);
        app.take_readings.set_now(Some(from.clone()));
        ended(&mut app, plan.clone(), &id, copied, false);
        let m = app.message.clone().unwrap().0;
        assert!(
            m.starts_with("Copied \"") && m.contains("left where it is"),
            "{m}"
        );
        assert!(from.is_dir(), "the original in use is kept");
        assert_eq!(lib(&mut app).find("Pepsi").unwrap().takes.len(), 2);

        // Not in use, a move's original goes.
        app.take_readings.set_now(None);
        let mut plan = lib(&mut app).plan_add(&from, &id, true).unwrap();
        plan.keep_original = false;
        let copied = library::copy_add(&plan, &mut |_| {}, &never);
        ended(&mut app, plan, &id, copied, false);
        assert!(!from.exists());
        assert_eq!(lib(&mut app).find("Pepsi").unwrap().takes.len(), 3);
        let _ = std::fs::remove_dir_all(&base);
    }
}
