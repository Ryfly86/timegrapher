//! The guided Positions, Poise, Power Reserve and Complication tests for one bench.

use super::*;

/// The guided tests' state. One per bench.
pub(super) struct GuidedTest {
    /// The Power Reserve Test, while it runs and once ended, until a new
    /// test or a new session.
    pub(super) reserve: Option<reserve::Test>,
    /// The keeping picked for the next Power Reserve Test. It runs in the
    /// Current Position card's position: there is no second setting.
    pub(super) reserve_keep_sound: bool,
    /// When the running test last read the beats, audio seconds.
    pub(super) reserve_read_s: f64,
    /// When the running Power Reserve Test's hours were last measured,
    /// audio seconds.
    pub(super) reserve_hours_s: f64,
    /// An opened take's Complication Test, its three stretches from the
    /// session file, read again once the take's beats are in (#1620).
    pub(super) complication_saved: Option<guided::Test>,
    /// A test ended with no watch picked: where it goes, asked.
    pub(super) test_keep: Option<TestKeep>,
    /// This take ran a Positions Test.
    pub(super) positions_take: bool,
    /// The position card's title as last drawn, for the words naming it.
    pub(super) position_card_title: &'static str,
    /// The Positions Test, while it runs and once finished, until a new
    /// test or a new session.
    pub(super) guided: Option<guided::Test>,
    /// The time the Positions Test didn't use, marked on the strip; kept
    /// after the test ends, until a new session.
    pub(super) guided_unused: Vec<(f64, f64, &'static str)>,
    /// The stretches the Positions Test measured, the one under way
    /// included, kept like `guided_unused`.
    pub(super) guided_measured: Vec<(f64, f64)>,
    /// The full-wind test's results the next test is set against as its
    /// 24 hour half: picked from `guided_choices`, or loaded from a file.
    pub(super) guided_earlier: Option<guided::FullWind>,
    /// An opened 24 hour half's saved results, which the pane shows again.
    pub(super) guided_saved: Option<guided::FullWind>,
    /// For a watch set up for 24 Hours After, its full-wind tests from 12
    /// to 36 hours ago, newest first, for Compare With to pick from.
    pub(super) guided_choices: Vec<guided::FullWind>,
    /// Whether `guided_earlier` was loaded from a file, so it stays when
    /// the watch's takes are searched again.
    pub(super) guided_earlier_file: bool,
    /// A take that is no saved Positions Test, being read to compare
    /// with (#1851).
    pub(super) compare_take: Option<compare_take::Reading>,
    /// What `guided_choices` was found for: the watch, the Wind setting,
    /// the minute and the watch's takes folder. Any of them otherwise, it
    /// is searched for again, so a test passing the 12 hour mark, or a
    /// take added or removed, is seen.
    pub(super) guided_earlier_for: Option<FullWindKey>,
    /// For a watch, the next Positions Test is its 24 hours after half,
    /// set against the watch's own full-wind test (Wind on its toolbar).
    pub(super) guided_after: bool,
    /// A test set up on a clear Bench, waiting for Start Test on the
    /// Listen card: the Positions Test or the Power Reserve Test.
    pub(super) test_ready: Option<NewTake>,
    /// Stop Test… pressed on the Listen card, its question open.
    pub(super) stop_test_ask: bool,
    /// The Current Position card offers ten positions, the four 45°
    /// stops too, rather than six (its 6 | 10 switch, kept).
    pub(super) ten_positions: bool,
    /// Seconds of settling and of measuring in each position, for the
    /// next test.
    pub(super) guided_settle_s: f64,
    pub(super) guided_measure_s: f64,
    /// The guided test to begin once the ask before listening is
    /// answered, and for the Positions Test whether just after winding
    /// (`None`) or the 24 hour half.
    pub(super) guided_after_ask: Option<(guided::Kind, Option<guided::FullWind>)>,
}

impl Default for GuidedTest {
    fn default() -> Self {
        Self {
            reserve: None,
            reserve_keep_sound: true,
            reserve_read_s: 0.0,
            reserve_hours_s: f64::NEG_INFINITY,
            complication_saved: None,
            test_keep: None,
            positions_take: false,
            position_card_title: "Current Position",
            guided: None,
            guided_unused: Vec::new(),
            guided_measured: Vec::new(),
            guided_earlier: None,
            guided_saved: None,
            guided_choices: Vec::new(),
            guided_earlier_file: false,
            compare_take: None,
            guided_earlier_for: None,
            guided_after: false,
            test_ready: None,
            stop_test_ask: false,
            ten_positions: false,
            guided_settle_s: guided::SETTLE_S,
            guided_measure_s: guided::MEASURE_S,
            guided_after_ask: None,
        }
    }
}
