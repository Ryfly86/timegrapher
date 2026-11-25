//! Keeping and saving the take on show, for one bench: sound kept
//! from earlier sessions, Keep in a Watch…, saves and analysis-only
//! keeps under way, the library move, opening a recording, and the
//! crash note from an earlier run.

use super::*;

/// What is being kept, saved or opened, and what was asked about it.
pub(super) struct Keeping {
    /// A crash record from an earlier run not yet looked at, and whether
    /// its text is showing.
    pub(super) crash: Option<PathBuf>,
    pub(super) crash_shown: bool,
    /// Sound kept from earlier sessions, offered back one at a time.
    pub(super) kept: Vec<KeptSound>,
    /// The kept sound Delete… is asking about.
    pub(super) kept_delete_ask: Option<PathBuf>,
    /// The form was opened from Keep in a Watch…: Done keeps what is on
    /// show in the new watch.
    pub(super) keep_after_form: bool,
    /// The watch (or Unsorted) picked before Keep in a Watch… showed the
    /// one it keeps into, put back if its question is cancelled.
    pub(super) keep_back: Option<watches_view::Picked>,
    pub(super) take_keep: Option<(PathBuf, AnalysisSave)>,
    /// A take being copied into a watch by Keep in a Watch… (#1970).
    pub(super) take_add: Option<take_add::TakeAdd>,
    /// Saved hours still WAVs being kept as FLAC, from start-up (#503).
    pub(super) wav_keep: Option<library_folder::WavKeep>,
    /// Where Move Library… is asking to move it to.
    pub(super) library_move_ask: Option<PathBuf>,
    /// A move under way: where to, and its outcome when done.
    pub(super) library_moving: Option<(PathBuf, LibraryMove)>,
    /// Where the last Save put the session, if it has been saved.
    /// The folder Save offers first.
    pub(super) save_dir: String,
    /// Open Recording… or Open Folder… asking where the recording goes.
    pub(super) open_ask: Option<OpenAsk>,
    /// The recording last opened, which the path box goes back to when
    /// another can't be opened.
    pub(super) opened_path: String,
    /// The take picked in the Watches view's list.
    pub(super) take_picked: Option<PathBuf>,
    /// Said with the result once the recording opening is analysed.
    pub(super) open_note: Option<String>,
    /// Keeping the session as its analysis alone, while that is worked out.
    pub(super) analysis_save: Option<AnalysisSave>,
    /// Saving the session's sound, while the copy is written.
    pub(super) saving: Option<Saving>,
    /// When a save of a take recording into its folder last failed, until
    /// the next is tried.
    pub(super) stream_wait: Option<Instant>,
    /// Quitting once the save that ends a take recording into its folder
    /// is written.
    pub(super) close_after_save: bool,
    /// What was asked while a take's save into its folder was written.
    pub(super) after_save: Vec<take_io::AfterSave>,
    /// Kept as its analysis alone: the 2 s profiles in its sound's place.
    pub(super) no_sound: Option<Vec<analysis_take::StretchProfiles>>,
    /// Where this session's analysis was last kept without its sound, and
    /// the seconds of sound it covers. The sound itself stays unsaved.
    pub(super) analysis_kept_to: Option<(PathBuf, f64)>,
}

impl Default for Keeping {
    fn default() -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            crash: None,
            crash_shown: false,
            kept: Vec::new(),
            kept_delete_ask: None,
            keep_after_form: false,
            keep_back: None,
            take_keep: None,
            take_add: None,
            wav_keep: None,
            library_move_ask: None,
            library_moving: None,
            save_dir: home.join("Timegrapher recordings").display().to_string(),
            open_ask: None,
            opened_path: String::new(),
            take_picked: None,
            open_note: None,
            analysis_save: None,
            saving: None,
            stream_wait: None,
            close_after_save: false,
            after_save: Vec::new(),
            no_sound: None,
            analysis_kept_to: None,
        }
    }
}
