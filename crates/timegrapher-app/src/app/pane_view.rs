//! How the panes look: the Paper Strip's view, follow and fit, its
//! overlays and rate line, the charts drawn and their fit and follow, the
//! shared time cursor, and the test, Steadiness, Scope and Distributions
//! panes' series and Info switches (#1988). One per bench, once benches come.

use super::*;

/// The strip and chart switches and views.
pub(super) struct PaneView {
    /// A take started or opened: the time panes' gutter fits its labels
    /// afresh (Hugh on #1239).
    pub(super) gutter_fresh: bool,
    pub(super) strip: StripView,
    /// Keep the newest beats on the centre line, sliding the rest.
    pub(super) follow: bool,
    /// Draw the rate reading as a line over the strip.
    pub(super) rate_line: bool,
    /// Faint parallels to the rate line across the strip.
    pub(super) rate_guides: bool,
    /// Amplitude, rate and beat error drawn over the strip.
    pub(super) overlays: [bool; 3],
    /// The readings the Charts pane draws: rate, amplitude, beat error.
    pub(super) chart_series: [bool; 3],
    /// The charts drawn last frame: those turned on that had room. The
    /// key names only these.
    pub(super) charts_drawn: [bool; 3],
    /// The time under the pointer on the strip or a chart, drawn as a
    /// cursor on all of them: last frame's, and this frame's so far.
    pub(super) cursor_t: Option<f64>,
    pub(super) cursor_next: Option<f64>,
    /// Asked to put the charts back on the whole session, following new
    /// beats: set by the button, applied by every chart on the next frame.
    pub(super) charts_to_live: bool,
    pub(super) charts_to_live_now: bool,
    /// Whether the charts show the whole session, as the pane's Fit
    /// switch shows (#1063).
    pub(super) charts_fit: bool,
    /// The time under the pointer over any chart last frame, which every
    /// chart's lane reads its values at (#1063).
    pub(super) charts_over: Option<f64>,
    /// The charts, panned or zoomed, end at the newest reading and follow
    /// new ones at the length they show (Hugh, #1070).
    pub(super) charts_follow: bool,
    /// The averaging time each chart last drew with: a new one rescales
    /// its values, which spread wider or narrower with it.
    pub(super) chart_average_drawn: std::collections::HashMap<&'static str, f64>,
    /// `None` follows the newest beat; otherwise the time at the newest
    /// edge of the strip. The strip, the chart marker for it and the sound
    /// panes (the Tick Tock Profile and the Scope) follow this; the
    /// readings show the newest.
    pub(super) strip_end: Option<f64>,
    /// The strip's view and Rate Line and Parallel Guides before an opened
    /// take was fitted, and the fitted view: Listen brings them back.
    pub(super) live_strip: Option<(StripView, StripView, [bool; 2])>,
    /// Fit, on the Paper Strip: while on, the strip's length is the whole
    /// session so far, growing as beats arrive. Any zoom or pan of the
    /// strip's time turns it off.
    pub(super) strip_fit: bool,
    /// The unrounded length a run of strip zooms is heading for.
    pub(super) strip_zoom_s: Option<f64>,
    /// Whether Fit was on before an opened take was fitted.
    pub(super) live_fit: bool,
    /// The Dynamic Poise pane's Plot and Dial on or off, and its Info.
    pub(super) poise_views: [bool; 2],
    pub(super) poise_info: bool,
    /// Whether the Complication pane's Info panel shows.
    pub(super) complication_info: bool,
    /// The readings the Distributions pane draws.
    pub(super) dist_series: [bool; 3],
    /// The charts the Power Reserve, Positions and Complication panes draw (#1729).
    pub(super) reserve_series: [bool; 3],
    pub(super) positions_series: [bool; 3],
    pub(super) complication_series: [bool; 3],
    /// Show the cumulative share instead of the probability plot.
    pub(super) hist_cumulative: bool,
    /// The Power Reserve and Positions Test panes' Info panels open (#1528).
    pub(super) reserve_info: bool,
    pub(super) positions_info: bool,
    /// The histograms count the readings rather than each 2 seconds of beats.
    pub(super) hist_readings: bool,
    pub(super) iso_view: isochronism::View,
    /// The rate over each 2 seconds of beats, for the rate's distribution.
    pub(super) dist: distributions::DistState,
    /// The series each Steadiness pane shows: rate, amplitude, beat error.
    pub(super) steady_series: std::collections::BTreeMap<steady::View, [bool; 3]>,
    /// The Periods pane's scores in dB.
    pub(super) periods_db: bool,
    /// The FFT pane's rate as the beat timing's spectrum, in ms.
    pub(super) fft_timing: bool,
    /// Each STFT and Wavelet Map pane's colours in dB.
    pub(super) maps_db: steady::PerPane,
    /// The calibre's wheels and meshes marked on each pane's period axes.
    pub(super) show_marks: steady::MarkSwitches,
    /// Each Steadiness pane's Info panel open.
    pub(super) steady_info: steady::PerPane,
    pub(super) scope_view: scope::View,
    /// Watches view groups folded down to their caption.
    pub(super) folded: std::collections::BTreeSet<String>,
    /// The rate, amplitude and beat error figures above the panes.
    pub(super) show_readings: bool,
    /// Those cards pinned open in the layout, else a strip that opens them.
    pub(super) readings_pinned: bool,
}

impl Default for PaneView {
    fn default() -> Self {
        Self {
            gutter_fresh: false,
            strip: Settings::default().strip,
            follow: false,
            rate_line: true,
            rate_guides: true,
            overlays: [true, false, false],
            chart_series: RATE_ONLY,
            charts_drawn: [true; 3],
            cursor_t: None,
            cursor_next: None,
            charts_to_live: false,
            charts_to_live_now: false,
            charts_fit: true,
            charts_over: None,
            charts_follow: false,
            chart_average_drawn: Default::default(),
            strip_end: None,
            live_strip: None,
            strip_fit: false,
            strip_zoom_s: None,
            live_fit: false,
            poise_views: POISE_VIEWS,
            poise_info: false,
            complication_info: false,
            dist_series: RATE_ONLY,
            reserve_series: [true; 3],
            positions_series: [true; 3],
            complication_series: [true; 3],
            hist_cumulative: false,
            reserve_info: false,
            positions_info: false,
            hist_readings: true,
            iso_view: Default::default(),
            dist: distributions::DistState::default(),
            steady_series: Default::default(),
            periods_db: false,
            fft_timing: false,
            maps_db: steady::PerPane::new(Vec::new(), false),
            show_marks: steady::MarkSwitches::new(Default::default()),
            steady_info: steady::PerPane::new(Vec::new(), false),
            scope_view: scope::View::default(),
            folded: Default::default(),
            show_readings: true,
            readings_pinned: true,
        }
    }
}
