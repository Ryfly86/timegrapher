//! The input's level meter, level control and automatic gain switch, as
//! last read: one owner for what Settings > Microphone and the meter show
//! (#1988).

use std::time::Instant;
use timegrapher_core::capture::Level;
use timegrapher_core::mixer::{GainState, InputGain};

/// The input's level meter, level control and automatic gain switch.
pub(super) struct InputMeter {
    pub(super) level: Level,
    pub(super) level_at: Instant,
    /// When the input last clipped.
    pub(super) clipped_at: Option<Instant>,
    /// The selected input's level control, and its state when last read.
    pub(super) gain: Option<InputGain>,
    pub(super) gain_state: Option<GainState>,
    pub(super) gain_read_at: Instant,
    /// The device the level control was read for.
    pub(super) gain_device: Option<String>,
    pub(super) gain_error: Option<String>,
    /// The selected input's automatic gain switch as last read back, when
    /// it has one.
    pub(super) agc_read: Option<bool>,
}

impl Default for InputMeter {
    fn default() -> Self {
        Self {
            level: Level::default(),
            level_at: Instant::now(),
            clipped_at: None,
            gain: None,
            gain_state: None,
            gain_read_at: Instant::now(),
            gain_device: None,
            gain_error: None,
            agc_read: None,
        }
    }
}
