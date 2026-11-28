//! The readings worked out for the take on show and the runs in the
//! background that read them, for one bench.

use super::*;

/// What the bench has read from the take on show, and what it is reading.
pub(super) struct Results {
    /// Readings worked out afresh over the strip's window when the run kept
    /// for the whole take is too sparse there, with what they were for.
    pub(super) strip_points: Option<(StripKey, Vec<TrendPoint>)>,
    /// Core's readings the Distributions pane counts (#1144), what they were
    /// read for and up to, and the run in the background that replaces them.
    pub(super) dist_readings: Option<result_keys::DistRead<DistReadings>>,
    pub(super) dist_job: Option<result_keys::DistRead<std::sync::mpsc::Receiver<DistReadings>>>,
    /// Sound read from the recording for the Scope once stopped: the
    /// take and stretch asked for (seconds) and what was read of it.
    pub(super) scope_audio: Option<ScopeRead>,
    /// Each beat's trace once drawn, so a beat stays on the Scope (fading
    /// as newer ones come) after the live engine has let its sound go.
    pub(super) scope_traces: std::collections::VecDeque<(ScopeKey, Vec<f32>)>,
    pub(super) scope_job: Option<std::sync::mpsc::Receiver<ScopeRead>>,
    /// The steadiness tests' latest answer over the session, and the run
    /// in the background that will replace it.
    pub(super) steady: Option<(Revision, Steadiness)>,
    pub(super) steady_job: Option<SteadyJob>,
    /// The Average Over the steadiness answers and the run under way were
    /// read at: which stretches hear no watch, so what the tests leave
    /// out, rests on it, so they are read again when it changes (Norm on
    /// #2451). None until the panes first look.
    pub(super) steady_average_s: Option<f64>,
    /// The step the steadiness run under way is on, and when it started.
    pub(super) steady_work: (
        std::sync::Arc<std::sync::atomic::AtomicU8>,
        std::time::Instant,
    ),
    pub(super) trend: Vec<TrendPoint>,
    /// What the readings in [`Self::trend`] were read on: the revision
    /// and Average Over.
    pub(super) trend_key: TrendKey,
    /// The last [`TimegrapherApp::whole_take`] and what it was read from.
    pub(super) whole_take: std::cell::RefCell<Option<(WholeKey, Option<WholeTake>)>>,
    /// The last stopped card's own span read, and what it was read from
    /// (`TimegrapherApp::own_span_read`).
    pub(super) own_span: std::cell::RefCell<Option<(WholeKey, Option<WholeTake>)>>,
    /// The last [`TimegrapherApp::free_table`] and what it was read from.
    pub(super) free_table: std::cell::RefCell<Option<(FreeKey, Option<guided::FreeTable>)>>,
    /// The next table while listening, read in the background, and what
    /// it is read from.
    pub(super) free_job: std::cell::RefCell<Option<FreeJob>>,
    /// The whole take being read in the background for the window.
    pub(super) whole_job: std::cell::RefCell<Option<take_reads::Job<WholeKey, Option<WholeTake>>>>,
    /// The charts' run and a long take's run being read in the background.
    pub(super) bg: take_reads::Background,
    /// The tick and tock sound every few seconds of this session, and one
    /// worked out from the file for a moment that history doesn't cover.
    pub(super) sound_history: Vec<(f64, scope_panes::Kept)>,
    /// The profiles worked out from the recording at a moment, with the
    /// take and settings revision they were read for.
    pub(super) sound_at: Option<(SoundKey, Profiles)>,
    pub(super) sound_job: Option<(SoundKey, std::sync::mpsc::Receiver<Profiles>)>,
}

impl Default for Results {
    fn default() -> Self {
        Self {
            strip_points: None,
            dist_readings: None,
            dist_job: None,
            scope_audio: None,
            scope_traces: std::collections::VecDeque::new(),
            scope_job: None,
            steady: None,
            steady_job: None,
            steady_average_s: None,
            steady_work: (Default::default(), std::time::Instant::now()),
            trend: Vec::new(),
            trend_key: TrendKey::default(),
            whole_take: Default::default(),
            own_span: Default::default(),
            free_table: Default::default(),
            free_job: Default::default(),
            whole_job: Default::default(),
            bg: Default::default(),
            sound_history: Vec::new(),
            sound_at: None,
            sound_job: None,
        }
    }
}

/// The steadiness run in the background: the [`Revision`] it reads, and
/// where its answer comes.
pub(super) struct SteadyJob {
    pub(super) rev: Revision,
    pub(super) rx: std::sync::mpsc::Receiver<steady::Answer>,
}
