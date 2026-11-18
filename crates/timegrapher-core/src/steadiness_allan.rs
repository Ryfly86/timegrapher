//! A series' Allan deviation, and which of its points rest on enough
//! differences to be judged: kept beside `steadiness.rs`, under its
//! length cap.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Allan {
    /// Averaging time, seconds; the Allan deviation there (the series'
    /// units); what independent readings would give; differences used.
    pub tau_s: Vec<f64>,
    pub deviation: Vec<f64>,
    pub white: Vec<f64>,
    pub pairs: Vec<usize>,
    /// Log-log slope over the short end (−0.5 for independent readings,
    /// 0 for flicker, +0.5 for a random walk, +1 for drift).
    pub slope_short: Option<f64>,
    /// The averaging time with the smallest deviation, and that value:
    /// how long to average for the most repeatable reading.
    pub best_tau_s: Option<f64>,
    pub best_deviation: Option<f64>,
    /// Largest deviation over the white line among points with at least
    /// [`ALLAN_JUDGED_DIFFERENCES`]: 1 for independent readings.
    pub max_excess: f64,
    /// [`ALLAN_JUDGED_DIFFERENCES`], so a view drawing the points on fewer
    /// hollow takes core's number rather than its own (#1985).
    pub judged_differences: usize,
}

/// The fewest differences an Allan deviation rests on for it to be
/// judged: picked as the least scatter, or weighed against independent
/// readings. With 3, a σ estimate is good to roughly ±40% (Norm, #1985).
pub const ALLAN_JUDGED_DIFFERENCES: usize = 8;

impl Allan {
    /// Whether each averaging time rests on enough differences to be
    /// judged ([`ALLAN_JUDGED_DIFFERENCES`]); the plot draws the others
    /// hollow.
    pub fn judged(&self) -> Vec<bool> {
        self.pairs
            .iter()
            .map(|&n| n >= ALLAN_JUDGED_DIFFERENCES)
            .collect()
    }
}
