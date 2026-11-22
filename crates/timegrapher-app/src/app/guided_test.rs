//! The guided tests' own state, slice 1: the Power Reserve Test and an
//! opened take's Complication Test (#1988). The Positions Test and Poise
//! join it in the next step.

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
}

impl Default for GuidedTest {
    fn default() -> Self {
        Self {
            reserve: None,
            reserve_keep_sound: true,
            reserve_read_s: 0.0,
            reserve_hours_s: f64::NEG_INFINITY,
            complication_saved: None,
        }
    }
}
