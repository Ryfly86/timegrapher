//! The Positions Test's position line (#1729, Hugh's design approved by
//! Peter 2026-11-14): the value is the axis, and each position is a dot
//! where its figure falls, on a lane of its own. The lanes sit in two
//! groups, Horizontal then Vertical, each in the table's order. The three
//! averages (every position, the vertical ones and the horizontal ones)
//! are named reference lines. On a later state of wind, each position's
//! full-wind figure is given beside it, drawn hollow with an arrow to this
//! test's.
//!
//! What to draw and what it says, never how: the app, the CLI's report and
//! the MCP view each draw this one model. No position is ever joined to
//! another (Norm's chart rule 4), and the averages are reference lines, not
//! a fit.

use serde::Serialize;

use crate::chart_scale;
use crate::session::{group_mean, PositionGroup, PositionValue, StateIndices};
use crate::steadiness::SeriesKind;

/// The key's words for a comparison: this test's dots are solid, the
/// full-wind test's hollow.
pub const THIS_TEST: &str = "This test";
pub const FULL_WIND_TEST: &str = "Full wind test";

/// The name of the line at the mean of every position.
pub const AVERAGE: &str = "Average";

/// One position's lane.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Lane {
    /// The position's common code: "DU", "CU", "1H30".
    pub position: &'static str,
    pub group: PositionGroup,
    /// Its name as the table writes it: "Crown Up (CU, 3H)".
    pub name: String,
    /// This test's figure, `None` where it has none.
    pub value: Option<f64>,
    /// The full-wind test's figure at this position, on a later state.
    pub full_wind: Option<f64>,
    /// `value` and `full_wind` as written beside a chart's title and in a
    /// table, with the unit: "+10.2 s/d".
    pub text: Option<String>,
    pub full_wind_text: Option<String>,
    /// The hover: the name and this test's figure with its unit,
    /// "Crown Up (CU, 3H) +10.2 s/d", and the full-wind figure after it
    /// where there is one.
    pub hover: String,
}

/// Which of the three averages a line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AverageOf {
    All,
    Vertical,
    Horizontal,
}

/// A grey dashed reference line at one of this test's averages.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AverageLine {
    pub of: AverageOf,
    /// Its name above the plot: "Average", "Vertical", "Horizontal".
    pub name: &'static str,
    pub value: f64,
    /// `value` with its unit: "+6.6 s/d".
    pub text: String,
    /// "Vertical +6.6 s/d".
    pub hover: String,
}

/// One measure's position line for one state of wind.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PositionLine {
    pub series: SeriesKind,
    /// The chart's name: "Rate".
    pub title: &'static str,
    /// The unit under the axis: "s/d", "°", "ms".
    pub unit: &'static str,
    /// Horizontal lanes first, then vertical, each in the table's order.
    pub lanes: Vec<Lane>,
    /// The averages that exist, in the order Average, Vertical,
    /// Horizontal.
    pub averages: Vec<AverageLine>,
    /// The key, on a later state only: what the solid and the hollow dots
    /// are.
    pub key: Option<[&'static str; 2]>,
    /// The least and greatest value drawn (dots of both tests and the
    /// averages), for the axis to hold; `None` with nothing to draw.
    pub extent: Option<[f64; 2]>,
    /// The value axis every surface draws: the extent padded
    /// ([`chart_scale::data_range`]), a beat error's from no lower than 0,
    /// and snapped to round ends ([`chart_scale::round_ends`]), as
    /// `[lo, hi, step]`.
    pub axis: Option<[f64; 3]>,
}

/// The figure a position's lane shows for `kind`.
fn figure(v: &PositionValue, kind: SeriesKind) -> Option<f64> {
    match kind {
        SeriesKind::Rate => v.rate_s_per_day,
        SeriesKind::Amplitude => v.amplitude_deg,
        SeriesKind::BeatError => v.beat_error_ms,
    }
    .filter(|x| x.is_finite())
}

/// The unit as it follows the axis: degrees as "°".
fn unit_of(kind: SeriesKind) -> &'static str {
    match kind {
        SeriesKind::Amplitude => "°",
        k => k.unit(),
    }
}

/// `state`'s position line for `kind`, with `full_wind`'s figures beside
/// it when it is a later state of wind compared with the full-wind test.
pub fn position_line(
    state: &StateIndices,
    full_wind: Option<&StateIndices>,
    kind: SeriesKind,
) -> PositionLine {
    let mut lanes = Vec::new();
    for group in [PositionGroup::Horizontal, PositionGroup::Vertical] {
        for v in state
            .positions
            .iter()
            .filter(|v| PositionGroup::of(v.position) == group)
        {
            let value = figure(v, kind);
            let earlier = full_wind.and_then(|f| {
                f.positions
                    .iter()
                    .find(|w| w.position == v.position)
                    .and_then(|w| figure(w, kind))
            });
            let name = v.position.display_label();
            let text = value.map(|x| kind.fmt(x));
            let full_wind_text = earlier.map(|e| kind.fmt(e));
            let mut hover = match &text {
                Some(t) => format!("{name} {t}"),
                None => name.clone(),
            };
            if let Some(e) = &full_wind_text {
                hover += &format!(" ({FULL_WIND_TEST} {e})");
            }
            lanes.push(Lane {
                position: v.position.common(),
                group,
                name,
                value,
                full_wind: earlier,
                text,
                full_wind_text,
                hover,
            });
        }
    }
    let averages: Vec<AverageLine> = [
        (AverageOf::All, AVERAGE, None),
        (
            AverageOf::Vertical,
            PositionGroup::Vertical.name(),
            Some(PositionGroup::Vertical),
        ),
        (
            AverageOf::Horizontal,
            PositionGroup::Horizontal.name(),
            Some(PositionGroup::Horizontal),
        ),
    ]
    .into_iter()
    .filter_map(|(of, name, group)| {
        let value = group_mean(&state.positions, kind, group).filter(|x| x.is_finite())?;
        let text = kind.fmt(value);
        Some(AverageLine {
            of,
            name,
            value,
            hover: format!("{name} {text}"),
            text,
        })
    })
    .collect();
    let drawn: Vec<f64> = lanes
        .iter()
        .flat_map(|l| [l.value, l.full_wind])
        .flatten()
        .chain(averages.iter().map(|a| a.value))
        .collect();
    let extent = (!drawn.is_empty()).then(|| {
        [
            drawn.iter().copied().fold(f64::INFINITY, f64::min),
            drawn.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        ]
    });
    let axis = extent
        .and_then(|[lo, hi]| chart_scale::data_range(&[lo, hi]))
        // A beat error is a size: its axis starts no lower than 0 (Walter
        // on #1742).
        .map(|(lo, hi)| match kind {
            SeriesKind::BeatError => (lo.max(0.0), hi),
            _ => (lo, hi),
        })
        .map(|(lo, hi)| {
            let (lo, hi, step) = chart_scale::round_ends(lo, hi);
            [lo, hi, step]
        });
    let compared = lanes.iter().any(|l| l.full_wind.is_some());
    PositionLine {
        series: kind,
        title: kind.title(),
        unit: unit_of(kind),
        lanes,
        averages,
        key: compared.then_some([THIS_TEST, FULL_WIND_TEST]),
        extent,
        axis,
    }
}

/// One state of wind's three position lines.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateLines {
    pub wind_h: Option<f64>,
    /// The state's heading: "Full wind", "24 h after winding", "Wind not
    /// given" ([`crate::reading_text::wind_state_title`]).
    pub name: String,
    pub rate: PositionLine,
    pub amplitude: PositionLine,
    pub beat_error: PositionLine,
}

/// The position lines of each state of wind, fullest first as `states`
/// are: a later state whose wind was given is compared with the fullest
/// one, when that is the full-wind test (0 h).
pub fn state_lines(states: &[StateIndices]) -> Vec<StateLines> {
    let full = states.first().filter(|s| s.wind_h == Some(0.0));
    states
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let earlier = full.filter(|_| i > 0 && s.wind_h.is_some_and(|h| h > 0.0));
            StateLines {
                wind_h: s.wind_h,
                name: crate::reading_text::wind_state_title(s.wind_h),
                rate: position_line(s, earlier, SeriesKind::Rate),
                amplitude: position_line(s, earlier, SeriesKind::Amplitude),
                beat_error: position_line(s, earlier, SeriesKind::BeatError),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{sourced_indices, BeatErrorSource, Position, SourcedFigures};

    fn figures(rates: &[(Position, f64, f64)]) -> Vec<SourcedFigures> {
        rates
            .iter()
            .map(|&(p, r, a)| {
                (
                    p,
                    Some(r),
                    Some(a),
                    Some(0.2),
                    Some(BeatErrorSource::Unlock),
                )
            })
            .collect()
    }

    fn six(shift: f64) -> Vec<SourcedFigures> {
        figures(&[
            (Position::CH, 2.0 + shift, 300.0),
            (Position::CB, 4.0 + shift, 296.0),
            (Position::H3, 10.0 + shift, 270.0),
            (Position::H6, 6.0 + shift, 268.0),
            (Position::H9, 5.0 + shift, 266.0),
            (Position::H12, 7.0 + shift, 272.0),
        ])
    }

    /// Lanes run Horizontal then Vertical in the table's order; the three
    /// averages are the table's own; no key without a comparison.
    #[test]
    fn a_state_gives_its_lanes_and_averages() {
        let st = sourced_indices(Some(0.0), &six(0.0));
        let line = position_line(&st, None, SeriesKind::Rate);
        let order: Vec<&str> = line.lanes.iter().map(|l| l.position).collect();
        assert_eq!(order, ["DU", "DD", "CD", "CL", "CU", "CR"]);
        assert_eq!(line.lanes[2].group, PositionGroup::Vertical);
        assert_eq!(line.lanes[4].hover, "Crown Up (CU, 3H) +10.0 s/d");
        assert_eq!(line.lanes[4].text.as_deref(), Some("+10.0 s/d"));
        assert_eq!(line.averages[1].text, "+7.0 s/d");
        let avg: Vec<(&str, f64)> = line.averages.iter().map(|a| (a.name, a.value)).collect();
        assert_eq!(
            avg,
            [
                ("Average", 34.0 / 6.0),
                ("Vertical", 7.0),
                ("Horizontal", 3.0)
            ]
        );
        // The table's figures, from the one place averages are taken.
        assert_eq!(Some(avg[0].1), st.x);
        assert_eq!(Some(avg[1].1), st.xv);
        assert_eq!(Some(avg[2].1), st.xh);
        assert_eq!(line.key, None);
        assert_eq!(line.extent, Some([2.0, 10.0]));
        assert_eq!(line.axis, Some([0.0, 12.0, 2.0]));
        let amp = position_line(&st, None, SeriesKind::Amplitude);
        assert_eq!(amp.unit, "°");
        assert_eq!(Some(amp.averages[0].value), st.amplitude_mean);
        let be = position_line(&st, None, SeriesKind::BeatError);
        assert_eq!(Some(be.averages[0].value), st.beat_error_mean);
        // A beat error is a size: its padded axis stops at 0.
        let near: Vec<SourcedFigures> = [(Position::CH, 0.01), (Position::H3, 0.4)]
            .iter()
            .map(|&(p, b)| {
                (
                    p,
                    Some(1.0),
                    Some(270.0),
                    Some(b),
                    Some(BeatErrorSource::Unlock),
                )
            })
            .collect();
        let be = position_line(
            &sourced_indices(Some(0.0), &near),
            None,
            SeriesKind::BeatError,
        );
        assert_eq!(be.axis.map(|a| a[0]), Some(0.0), "{:?}", be.axis);
    }

    /// A 24 h state is set against the full-wind test: each lane carries
    /// its full-wind figure, the key names both, and the extent holds both.
    #[test]
    fn a_later_state_carries_the_full_wind_figures() {
        let states = vec![
            sourced_indices(Some(0.0), &six(0.0)),
            sourced_indices(Some(24.0), &six(-3.0)),
        ];
        let lines = state_lines(&states);
        assert_eq!(lines[0].rate.key, None);
        assert_eq!(lines[0].name, "Full wind");
        assert_eq!(lines[1].name, "24 h after winding");
        let later = &lines[1].rate;
        assert_eq!(later.key, Some([THIS_TEST, FULL_WIND_TEST]));
        assert_eq!(later.lanes[0].value, Some(-1.0));
        assert_eq!(later.lanes[0].full_wind, Some(2.0));
        assert_eq!(later.extent, Some([-1.0, 10.0]));
        assert!(later.lanes[0].hover.ends_with("(Full wind test +2.0 s/d)"));
        assert_eq!(later.lanes[0].full_wind_text.as_deref(), Some("+2.0 s/d"));
        // No full-wind test, no comparison.
        let alone = state_lines(&states[1..]);
        assert_eq!(alone[0].rate.key, None);
    }
}
