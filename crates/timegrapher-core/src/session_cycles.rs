//! A recording's periodic changes as a session reports them: [`Cycle`],
//! its size and share in words, and the notes said once beside them.

use super::Reading;
use crate::longrun::LongReport;
use crate::periodicity::Wheel;
use serde::Serialize;

/// A periodic change found in one recording.
#[derive(Debug, Clone, Serialize)]
pub struct Cycle {
    /// "rate" or "amplitude".
    pub series: &'static str,
    pub period_s: f64,
    /// Peak to peak, in `size_unit`: degrees for amplitude; for rate, s/d,
    /// or the timing swing in ms for a cycle shorter than
    /// [`crate::longrun::TIMING_UNIT_BELOW_S`], which a rate reading cannot
    /// follow (see [`crate::longrun::RateComponent::size`]).
    pub size: f64,
    /// "s/d", "ms" or "deg".
    pub size_unit: &'static str,
    /// -log10 of the false-alarm probability.
    pub significance: f64,
    /// Past the simulated null's range ([`crate::longterm::beyond_simulated`]).
    pub beyond_simulated: bool,
    /// Share of the series' variance the cycle explains, 0..1. Not a size
    /// for a rate cycle, found in the beat timing (a walk): no text gives
    /// it there ([`Cycle::share_percent`], Norm on #2100).
    pub explained: f64,
    pub wheel: Option<String>,
    /// The nearest listed wheel and its offset in percent of its turn,
    /// only within [`crate::longterm::NEAREST_WHEEL_REACH`].
    pub nearest_wheel: Option<(String, f64)>,
    /// A wheel whose turn is a whole number of these periods, and the
    /// number: a short, sharp change once per turn also shows here.
    pub fraction_of: Option<(String, u32)>,
    /// A longer cycle found in the same series whose period is this one's
    /// times a whole number, as core's search marks it
    /// ([`crate::longterm::Component::harmonic_of`]): this one is a
    /// harmonic of that cycle, not a cycle of its own.
    pub harmonic_of: Option<crate::longterm::Harmonic>,
}

impl Cycle {
    pub fn unit(&self) -> &'static str {
        self.size_unit
    }

    /// The size with its unit, written one place finer than its figure as
    /// a difference within one take (math standards, section 11):
    /// "10.93 s/d", "0.052 ms" or "7.8 deg".
    pub fn size_text(&self) -> String {
        use crate::figure_text::{base_digits, difference_digits, fixed_text};
        let digits = difference_digits(match self.size_unit {
            "ms" => base_digits::BEAT_ERROR,
            "s/d" => base_digits::RATE,
            _ => base_digits::AMPLITUDE,
        });
        format!("{} {}", fixed_text(self.size, digits), self.size_unit)
    }

    /// The size named by its quantity, peak to peak: "timing swing 0.103 ms
    /// peak to peak" below [`crate::longrun::TIMING_UNIT_BELOW_S`], "rate
    /// swing 3.04 s/d peak to peak" above it, "7.8 deg peak to peak" for
    /// amplitude: a rate cycle is sized in two quantities, never one in two
    /// units (Norm on #2100).
    pub fn size_words(&self) -> String {
        let quantity = match self.size_unit {
            "ms" => "timing swing ",
            "s/d" => "rate swing ",
            _ => "",
        };
        format!("{quantity}{} peak to peak", self.size_text())
    }

    /// The share of the series' variation this cycle explains, in percent,
    /// where it means something: not for a rate cycle. A rate cycle is
    /// found in the beat timing, a walk whose power fills the band, so a
    /// share of it is no measure of a line (a real escape wheel line read
    /// "0%"; [`crate::longterm::MIN_RATE_SWING_S_PER_DAY`], Norm on #2100).
    pub fn share_percent(&self) -> Option<f64> {
        (self.series != "rate").then_some(self.explained * 100.0)
    }
}

/// Said once beside the findings or the cycles table when a cycle is sized
/// as a timing swing ([`Cycle::size_words`]), so no line has to say it.
pub fn timing_swing_text() -> String {
    format!(
        "A rate cycle shorter than {} s is sized as a timing swing in ms: a 10 s rate reading is too coarse to follow it.",
        crate::longrun::TIMING_UNIT_BELOW_S
    )
}

/// Said once under the cycles table: why a rate cycle has no share.
pub const RATE_SHARE_NOTE: &str = "A rate cycle is found in the beat timing, which wanders as a walk, so a share of its variation says nothing about the cycle and none is given (–).";

/// [`timing_swing_text`] when a finding sizes a cycle as a timing swing.
pub fn timing_swing_note(readings: &[Reading]) -> Option<String> {
    readings
        .iter()
        .filter(|r| r.measurement.watch_heard)
        .flat_map(|r| r.cycles.iter().map(move |c| (r, c)))
        .any(|(r, c)| c.size_unit == "ms" && folded_into(r, c).is_none())
        .then(timing_swing_text)
}

/// The cycle of `r` a cycle core marks as a harmonic of is reported with,
/// when it is: not a wheel's own line, and explaining no more of the
/// variation than that one does.
pub(super) fn folded_into<'a>(r: &'a Reading, c: &Cycle) -> Option<&'a Cycle> {
    harmonic_parent(r, c).filter(|w| c.wheel.is_none() && c.explained <= w.explained)
}

/// The longer cycle of `r` that core marks `c` a harmonic of, if listed.
pub(super) fn harmonic_parent<'a>(r: &'a Reading, c: &Cycle) -> Option<&'a Cycle> {
    let h = c.harmonic_of?;
    r.cycles
        .iter()
        .find(|w| w.series == c.series && w.period_s == h.period_s)
}

/// The wheel whose turn a component not tied to a wheel sits at a whole
/// fraction of (1/2 to 1/6), and the n: by core's one whole-fraction test,
/// within one resolution (1/span) in frequency of n turns (#775). Only
/// when exactly one wheel and n fit: a turn about as long as the span
/// lets neighbouring n both fit, and then the fraction is not told (#787).
pub(crate) fn wheel_fraction(
    c: &crate::longterm::Component,
    wheels: &[Wheel],
) -> Option<(String, u32)> {
    if c.wheel.is_some() {
        return None;
    }
    let mut fits = wheels.iter().flat_map(|w| {
        (2..=6u8)
            .filter(|&n| {
                crate::longterm::harmonic_offset(c.period_s, c.resolution_s, n, w.period_s) < 1.0
            })
            .map(|n| (w.name.clone(), u32::from(n)))
    });
    let one = fits.next()?;
    fits.next().is_none().then_some(one)
}

/// The periodic changes `long` found, without their long arrays.
/// `wheels` are the turn periods searched, used to mark a component at a
/// whole fraction of a wheel's turn.
pub fn cycles(r: &LongReport, wheels: &[Wheel]) -> Vec<Cycle> {
    let fraction = |c: &crate::longterm::Component| wheel_fraction(c, wheels);
    let rate = r.rate_components.iter().map(|c| Cycle {
        series: "rate",
        period_s: c.component.period_s,
        size: c.size().0,
        size_unit: c.size().1,
        significance: c.component.significance,
        beyond_simulated: c.component.beyond_simulated,
        explained: c.component.explained,
        wheel: c.component.wheel.clone(),
        nearest_wheel: c.component.nearest_wheel.clone(),
        fraction_of: fraction(&c.component),
        harmonic_of: c.component.harmonic_of,
    });
    let amp = r.amplitude.components.iter().map(|c| Cycle {
        series: "amplitude",
        period_s: c.period_s,
        size: c.peak_to_peak,
        size_unit: "deg",
        significance: c.significance,
        beyond_simulated: c.beyond_simulated,
        explained: c.explained,
        wheel: c.wheel.clone(),
        nearest_wheel: c.nearest_wheel.clone(),
        fraction_of: fraction(c),
        harmonic_of: c.harmonic_of,
    });
    rate.chain(amp).collect()
}
