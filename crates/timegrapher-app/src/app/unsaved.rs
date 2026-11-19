//! What a session left unsaved: its sound, or for a Power Reserve Test
//! kept as its analysis alone, its analysis kept in parts as it went
//! (2c of #1514), offered back at the next start.

use super::*;

/// A session that was never saved or thrown away: the app quit without
/// asking (on macOS, Cmd+Q and Quit in the app menu or the Dock), or was
/// stopped (a crash, a logout, a shutdown).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct KeptSound {
    pub(super) dir: PathBuf,
    /// "2026-10-21 14.32", from its session.json, in this computer's time.
    pub(super) started: Option<String>,
    pub(super) duration_s: Option<f64>,
    pub(super) bytes: u64,
    /// The take it was recording into as it went, which holds all of it
    /// but its last few seconds.
    pub(super) streamed_to: Option<PathBuf>,
    /// A Power Reserve Test kept as its analysis alone: no sound, its
    /// analysis kept in parts up to `duration_s` ([`checkpoint::only`]).
    pub(super) parts: bool,
    /// Only its last part is here: hours its session file lists were kept
    /// in the take it was recording into alone (#2040), and that take has
    /// since been moved or deleted. `duration_s` is then not given.
    pub(super) partial: bool,
}

impl KeptSound {
    /// What it is, as the notice and its dialogs name it: "sound", or a
    /// test's "analysis".
    pub(super) fn what(&self) -> &'static str {
        if self.parts {
            "analysis"
        } else {
            "sound"
        }
    }

    /// What the start notice says of it: when it started, how much it
    /// holds, and what can be done with it.
    pub(super) fn notice(&self) -> String {
        let started = self
            .started
            .as_ref()
            .map_or(String::new(), |s| format!(": started {s}"));
        let size = size_text(self.bytes);
        if self.parts {
            // A test kept as its analysis alone: how much it recorded,
            // never its run (2c of #1514, #1806).
            let recorded = self
                .duration_s
                .map_or(String::new(), |d| format!(", {}", recorded_text(d)));
            format!(
                "An unsaved Power Reserve Test was found (Analysis Only){started}{recorded}, \
                 {size}. Open it, move it to a folder of your own, or delete it?"
            )
        } else if self.partial {
            format!(
                "Sound from an earlier session wasn't saved{started}, {size}. Only its last part \
                 is here: the take it was recording into held the rest, and has since been \
                 moved or deleted. Open it, move it into a folder of your own, or delete it?"
            )
        } else {
            let long = self
                .duration_s
                .map_or(String::new(), |d| format!(", {} long", length_text(d)));
            format!(
                "Sound from an earlier session wasn't saved{started}{long}, {size}. Open it, \
                 move it into a folder of your own, or delete it?"
            )
        }
    }

    /// [`Self::what`] in a title's capitals.
    pub(super) fn what_title(&self) -> &'static str {
        if self.parts {
            "Analysis"
        } else {
            "Sound"
        }
    }
}

impl TimegrapherApp {
    /// Let go of the session's own copy of each hour its take holds
    /// whole, once that copy is kept as FLAC (#2040): listening does it as
    /// it writes, this does it after a Stop.
    pub(super) fn tidy_recording(&mut self) {
        let Some(r) = self.session.recorder.as_mut().filter(|r| r.releasing()) else {
            return;
        };
        if let Err(e) = r.tidy() {
            applog::warn(&format!("couldn't tidy the session's folder: {e}"));
        }
    }

    /// Refuse a save, saying why, when hours the session kept in its take
    /// alone are gone with it (Desmond on #2046): it can't be saved whole
    /// anywhere, and a save would fail part way. The first line is short
    /// enough for the status bar to show whole (Hugh 4 on #2055).
    pub(super) fn refuse_hours_gone(&mut self) -> bool {
        if self.saved_hours_reachable() {
            return false;
        }
        self.error(
            "This sound can't be saved\nIts earlier part was kept only in its take, and the \
             take no longer holds it."
                .into(),
        );
        true
    }

    /// Keep in a Watch > New Watch…: the form, and the keep once it is
    /// done, unless the keep would be refused, which is said before the
    /// form is filled in for nothing (Walter 11 on #2057, #2058).
    pub(super) fn keep_in_new_watch(&mut self) {
        if self.refuse_hours_gone() {
            return;
        }
        self.open_new_watch_form();
        self.keep_after_form = true;
    }

    /// Whether every hour the session keeps in its take alone is still
    /// there (#2040). One gone with its take (deleted, or its sound
    /// removed) means the take can't be carried on whole, so Listen
    /// starts a new one rather than resuming.
    pub(super) fn saved_hours_reachable(&self) -> bool {
        self.session
            .recorder
            .as_ref()
            .is_none_or(Recorder::hours_reachable)
    }
}

/// What was left in `scratch` from earlier sessions, oldest first: sound,
/// or a test's analysis kept in parts, leaving out any folder another
/// copy of the app is recording into now.
pub(super) fn kept_sounds(scratch: &Path) -> Vec<KeptSound> {
    let mut v: Vec<KeptSound> = std::fs::read_dir(scratch)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|d| !timegrapher_core::recorder::in_use(d))
        .filter_map(|dir| {
            let files: Vec<std::fs::DirEntry> = std::fs::read_dir(&dir).ok()?.flatten().collect();
            let has_sound = files.iter().any(|f| {
                let n = f.file_name().to_string_lossy().to_lowercase();
                n.ends_with(".wav") || n.ends_with(".flac")
            });
            let parts = !has_sound && checkpoint::only(&dir);
            let names: Vec<String> = files
                .iter()
                .map(|f| f.file_name().to_string_lossy().to_lowercase())
                .collect();
            if !has_sound && !parts {
                return None;
            }
            let about: Option<serde_json::Value> = std::fs::read(dir.join("session.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            let started = started_of(about.as_ref());
            let partial = !parts && hours_missing(about.as_ref(), &names);
            // Of a test kept in parts, as far as its parts reach; none
            // for one with hours missing, whose session file counts them.
            let duration_s = match (parts, partial) {
                (true, _) => checkpoint::length_s(&dir).ok(),
                (false, true) => None,
                (false, false) => about.as_ref().and_then(|a| a["duration_s"].as_f64()),
            };
            let streamed_to = take_io::streamed_to(&dir);
            Some(KeptSound {
                started,
                duration_s,
                bytes: tree_bytes(&dir),
                streamed_to,
                parts,
                partial,
                dir,
            })
        })
        .collect();
    v.sort_by(|a, b| a.dir.cmp(&b.dir));
    v
}

/// Whether a sound file the session file `about` lists is in none of
/// `names` (the folder's files, in lower case), as itself or as the FLAC
/// it was kept as: an hour kept in its take alone (#2040).
fn hours_missing(about: Option<&serde_json::Value>, names: &[String]) -> bool {
    let Some(listed) = about.and_then(|a| a["segments"].as_array()) else {
        return false;
    };
    listed.iter().filter_map(|s| s.as_str()).any(|s| {
        let s = s.to_lowercase();
        let flac = timegrapher_core::flac::kept_name(Path::new(&s));
        !names.contains(&s) && !names.iter().any(|n| Path::new(n) == flac)
    })
}

/// When a recording started, from its `session.json` (`about`), in this
/// computer's time, as takes are named: "2026-10-21 14.32"; as written,
/// marked UTC, when it can't be read.
pub(super) fn started_of(about: Option<&serde_json::Value>) -> Option<String> {
    let u = about?["started_utc"].as_str()?;
    Some(watches::local_minute(u).unwrap_or_else(|| {
        let at = u.get(..16).unwrap_or(u).replacen('T', " ", 1);
        format!("{} UTC", at.replace(':', "."))
    }))
}

/// How much a test kept in parts recorded, as the start notice and Open
/// say it (Hugh and Norm on #1806): the last part's end, never its run,
/// which the Power Reserve pane gives.
pub(super) fn recorded_text(s: f64) -> String {
    format!("recorded {}", power_reserve::run_text(s))
}

/// The bytes the files under `dir` take, its folders' included (a test's
/// parts are kept in one, Felix).
fn tree_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => tree_bytes(&e.path()),
            _ => e.metadata().map_or(0, |m| m.len()),
        })
        .sum()
}

/// Move the recording folder `dir` into `parent`, under its own name:
/// renamed, or onto another disk copied, its folders with it, each file
/// written to the disk, and only then removed. Returns where it went.
pub(super) fn move_kept(dir: &Path, parent: &Path) -> Result<PathBuf, String> {
    let name = dir.file_name().ok_or("The kept sound has no folder name")?;
    let to = library::unused_dir(parent, &name.to_string_lossy());
    timegrapher_core::recorder::remove_stale_lock(dir);
    if std::fs::rename(dir, &to).is_ok() {
        return Ok(to);
    }
    match copy_tree(dir, &to) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(dir);
            Ok(to)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&to);
            Err(format!("Couldn't move it to {}: {e}", parent.display()))
        }
    }
}

/// Copy the folder `from` to `to`, each file and folder written to the
/// disk (Felix).
pub(super) fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &to.join(e.file_name()))?;
            continue;
        }
        let mut r = std::fs::File::open(e.path())?;
        let mut w = std::fs::File::create(to.join(e.file_name()))?;
        std::io::copy(&mut r, &mut w)?;
        w.sync_all()?;
    }
    timegrapher_core::recorder::sync_folder(to)
}
