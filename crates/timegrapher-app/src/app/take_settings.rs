//! What the take on show is read with, for one bench: the watch and
//! calibre, beat rate, lift and position, the wind, the masks and
//! positions painted on the strip, its clock and breaks, and the
//! revision its readings rest on.

use super::*;

/// The settings the take on show is read with.
pub(super) struct TakeSettings {
    pub(super) bph: Option<u32>,
    pub(super) lift_deg: f64,
    /// (value, supplied): restored/resolved context applies at this value;
    /// numeric edits away from it retain the existing default-angle inference.
    pub(super) lift_context: (f64, bool),
    pub(super) position: Position,
    /// The beat rate the live engine had locked on to, to log a change.
    pub(super) locked_bph: Option<u32>,
    /// When the watch was last fully wound, for a Quick Listen (#1665).
    pub(super) wind: wind::Wind,
    pub(super) average_s: f64,
    pub(super) watch: String,
    /// The calibre picked on the Calibre card: a name in the table or one of the
    /// user's own, or empty when not set.
    pub(super) calibre: String,
    /// The escapement the take on show's session file sets (its
    /// `escapement` key), which outranks its calibre's (core's
    /// `Escapement::setting`, #2313); None for a microphone session and a
    /// file that sets none.
    pub(super) escapement_set: Option<timegrapher_core::escapement::Escapement>,
    /// Stretches masked out of the readings on the strip, start and end in
    /// the session's seconds, sorted and apart. Saved with the take.
    pub(super) masked: Vec<(f64, f64)>,
    /// For a recording opened from disk: the stretches its session file
    /// masked when it was opened or last saved to, so a change can be
    /// asked about before it is thrown away. None for a microphone session.
    pub(super) opened_masks: Option<Vec<(f64, f64)>>,
    /// The strip's Mask tool is on: dragging along it masks a stretch, and
    /// a click on a masked one brings it back.
    pub(super) masking: bool,
    /// For a recording opened from disk: the positions and settling its
    /// session file gave when it was opened or last saved to, so a change
    /// made with the Position Brush can be asked about before it is thrown
    /// away. None for a microphone session.
    pub(super) opened_positions: Option<Layers>,
    /// Read again at a new beat rate: the positions as they were, and as
    /// the session file gave them, to put back once it is in.
    pub(super) carried_positions: Option<(Layers, Option<Layers>)>,
    /// The positions as recorded under the painting, from the session file
    /// of the recording being opened, to load with the positions shown.
    pub(super) pending_recorded: Option<Positions>,
    /// The sound card's clock for the take on show, as core's
    /// `take::open_entry` reads it for every surface: from its clock log,
    /// USB lines or the correction stored for the input it was recorded
    /// on. The readings, tables and True Clock use it in place of the
    /// clock kept for the input.
    pub(super) take_clock: Option<timegrapher_core::clock::ClockFit>,
    /// Where [`Self::take_clock`] came from, in core's words
    /// (`OpenedEntry::clock_source`), so the Rate's hover names it.
    pub(super) take_clock_source: Option<String>,
    /// The take on show's breaks, as core reads them with its beats: the
    /// pauses it lists, its clock's steps and the jumps in its beats.
    /// `None` for a session heard here, whose readings go by the clock
    /// kept for the input.
    pub(super) take_breaks: Option<Vec<timegrapher_core::breaks::Break>>,
    /// Why those breaks are not all the take's own list's, where they
    /// aren't (core's `breaks::Unlisted`), for the caveat that says so.
    pub(super) take_unlisted: Option<timegrapher_core::breaks::Unlisted>,
    /// The strip's Position Brush is on, with this position: dragging
    /// along the strip sets that stretch to it.
    pub(super) brush: Option<Position>,
    /// Edit ▸ Remove All Masks asked, waiting on its yes.
    pub(super) remove_masks_asked: bool,
    /// The watch set up for the microphone, kept while a recording with
    /// its own (or no) settings is open, and put back on listening again.
    pub(super) mic_setup: Option<WatchSetup>,
    /// The settings a take on show is read again with, for a new beat
    /// rate, in place of what the take saved.
    pub(super) reread_with: Option<WatchSetup>,
    /// What the take on show's readings rest on.
    pub(super) rev: Revision,
    /// Masks on the recording on show let go by the answer to the ask
    /// before listening, so starting doesn't ask again.
    pub(super) masks_let_go: bool,
}

impl Default for TakeSettings {
    fn default() -> Self {
        Self {
            bph: None,
            lift_deg: DEFAULT_LIFT_DEG,
            lift_context: (DEFAULT_LIFT_DEG, false),
            position: Position::DialUp,
            locked_bph: None,
            wind: Default::default(),
            average_s: timegrapher_core::live::DEFAULT_AVERAGE_S,
            watch: String::new(),
            calibre: String::new(),
            escapement_set: None,
            masked: Vec::new(),
            opened_masks: None,
            masking: false,
            opened_positions: None,
            carried_positions: None,
            pending_recorded: None,
            take_clock: None,
            take_clock_source: None,
            take_breaks: None,
            take_unlisted: None,
            brush: None,
            remove_masks_asked: false,
            mic_setup: None,
            reread_with: None,
            rev: Revision::default(),
            masks_let_go: false,
        }
    }
}

/// The Watch settings a take is read with.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct WatchSetup {
    pub(super) watch: String,
    pub(super) calibre: String,
    pub(super) escapement_set: Option<timegrapher_core::escapement::Escapement>,
    pub(super) lift_deg: f64,
    pub(super) lift_context: (f64, bool),
    pub(super) bph: Option<u32>,
    pub(super) position: Position,
}

impl Default for WatchSetup {
    fn default() -> Self {
        WatchSetup {
            watch: String::new(),
            calibre: String::new(),
            escapement_set: None,
            lift_deg: DEFAULT_LIFT_DEG,
            lift_context: (DEFAULT_LIFT_DEG, false),
            bph: None,
            position: Position::DialUp,
        }
    }
}
