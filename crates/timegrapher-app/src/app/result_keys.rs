//! What each result the bench keeps was read from (#1988 N1): the take
//! it belongs to, what its readings rest on, and where it was read from.
//! A result is drawn only while its key matches the take on show now; one
//! from an earlier take, or read on an earlier revision, is dropped and
//! read again, never drawn.
//!
//! The take is named by [`Revision::opens`], which moves for every take
//! opened and every session begun ([`Revision::new_take`]). A key that
//! holds the whole [`Revision`] holds it too.

use super::*;

/// What [`super::results::Results::whole_take`] was read from: the beats
/// heard, the stretches, the masks, what the readings rest on, and where
/// it was read from: the stopped take read as its saved take opens
/// (#2151), or the engine on show.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct WholeKey {
    pub(super) beats: usize,
    pub(super) spans: Vec<(f64, f64)>,
    pub(super) masked: Vec<(f64, f64)>,
    pub(super) rev: Revision,
    pub(super) stopped: bool,
}

/// What [`super::results::Results::free_table`] was read from: the beats
/// heard, each position's settled stretches, the masks and what the
/// readings rest on.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FreeKey {
    pub(super) beats: usize,
    pub(super) stretches: Vec<(core_session::Position, f64, f64)>,
    pub(super) masked: Vec<(f64, f64)>,
    pub(super) rev: Revision,
}

/// What [`super::results::Results::trend`] was read from: what the
/// readings rest on and Average Over (its bits).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TrendKey {
    pub(super) rev: Revision,
    pub(super) average: u64,
}

impl Default for TrendKey {
    /// Nothing read yet: the first revision, at the 10 s Average Over.
    fn default() -> Self {
        Self {
            rev: Revision::default(),
            average: 10.0f64.to_bits(),
        }
    }
}

/// What the strip's own readings were worked out for: the window in
/// steps, the averaging time (ms), the spacing (its bits), how much sound
/// there was (in whole steps while listening) and what the readings rest
/// on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StripKey {
    pub(super) from: i64,
    pub(super) to: i64,
    pub(super) average: u64,
    pub(super) step: u64,
    pub(super) sound: u64,
    pub(super) rev: Revision,
}

/// What core's readings for the Distributions pane were read for: what
/// the readings rest on, the position on show and Average Over (its
/// bits).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DistKey {
    pub(super) rev: Revision,
    pub(super) position: Position,
    pub(super) average: u64,
}

/// A beat's trace on the Scope: the take it is from, the beat's index,
/// its time's bits, and the zoom and beat (its bits) it was drawn at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ScopeKey {
    pub(super) take: u64,
    pub(super) index: i64,
    pub(super) time: u64,
    pub(super) zoom: scope::Zoom,
    pub(super) beat: u64,
}

/// A stretch of a recording read for the Scope: the take it was read
/// from, the start and end asked for, s, and the sound read (shorter at
/// the end of the recording), or why it couldn't be.
pub(super) struct ScopeRead {
    pub(super) take: u64,
    pub(super) from: f64,
    pub(super) to: f64,
    pub(super) got: Result<crate::span::Span, String>,
}

impl ScopeRead {
    /// Whether this is the take on show now, `rev`, and its stretch
    /// covers `lo` to `hi` s.
    pub(super) fn covers(&self, rev: Revision, lo: f64, hi: f64) -> bool {
        self.take == rev.opens && self.from <= lo && self.to >= hi
    }
}

/// What the Tick Tock Profile worked out from the recording was read
/// for: the moment, s, the take, and the settings revision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SoundKey {
    pub(super) at: f64,
    pub(super) take: u64,
    pub(super) settings: u64,
}

impl SoundKey {
    /// The moment `at` in the take on show now, on its settings now.
    pub(super) fn now(at: f64, rev: Revision) -> Self {
        Self {
            at,
            take: rev.opens,
            settings: rev.settings,
        }
    }

    /// Whether this was read for the take on show and its settings now,
    /// `rev`.
    pub(super) fn holds(&self, rev: Revision) -> bool {
        self.take == rev.opens && self.settings == rev.settings
    }
}

impl TimegrapherApp {
    /// What a run of readings read now rests on: the take's [`Revision`]
    /// and Average Over.
    pub(super) fn trend_now(&self) -> TrendKey {
        TrendKey {
            rev: self.bench.take.rev,
            average: self.bench.take.average_s.to_bits(),
        }
    }
}
