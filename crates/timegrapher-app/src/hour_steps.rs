//! A test pane's charts over hours, one per measure, stacked on one hour
//! axis with one hover line through them all (#1729, Hugh's layout,
//! Peter's approval of 14 Nov). Each hour's mean is a level step across
//! its own span, its ± a band where it has one, and a line breaks at
//! every hour not shown (Norm's ruling on #1729). An hour grey in the
//! pane's table is grey here too. Each chart's title row gives its value
//! at the hovered hour, in its colour, and the unit sits over its axis,
//! the ticks bare numbers. Only the bottom chart names the hours, and
//! under the hovered one says which it is.

use crate::steady::series_colour;
use crate::strength::{self, Grid};
use crate::theme;
use eframe::egui;
use timegrapher_core::chart_scale::axis_number;
use timegrapher_core::figure_text::{figure_with_pm, true_minus};
use timegrapher_core::steadiness::SeriesKind;

/// One hour's figure as drawn: its value, its ± (a standard error), and
/// whether it is grey, as its table cell is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub v: f64,
    pub se: Option<f64>,
    pub grey: bool,
}

/// One chart: its measure, a step for each hour (none where the hour's
/// figure isn't shown), and how its figures are written.
pub struct Chart {
    pub kind: SeriesKind,
    pub steps: Vec<Option<Step>>,
    pub unit: &'static str,
    /// Decimals ([`timegrapher_core::figure_text::base_digits`]).
    pub digits: usize,
    /// A sign written, and zero across drawn as a zero line: a rate.
    pub signed: bool,
    /// Its scale reaches zero: a rate, and a size such as a beat error,
    /// which a flat run would otherwise blow up into noise.
    pub to_zero: bool,
}

/// The hours' spans, `[from, to)` in hours since the run began, one for
/// each step of every chart; the last may be a part hour.
pub type Spans = [(f64, f64)];

/// The height of each chart's title row, points.
const TITLE_H: f32 = 18.0;

/// The height under the bottom chart for its hours, points.
const AXIS_H: f32 = 20.0;

/// The least height of a chart's plot, points.
pub const PLOT_MIN_H: f32 = 56.0;

/// The height a chart reads well at, its title row included, points:
/// its least, so three fit where the Power Reserve pane's two did.
pub const CHART_READ_H: f32 = TITLE_H + PLOT_MIN_H;

/// The steps, in hours, the hour axis goes up in.
const HOUR_STEPS: [f64; 7] = [1.0, 2.0, 3.0, 6.0, 12.0, 24.0, 48.0];

/// The least width of the charts' value axis, points.
const GUTTER: f32 = 48.0;

/// The hour axis's marks: none before the run, in the margin left of 0 h.
fn hour_marks(g: egui_plot::GridInput) -> Vec<egui_plot::GridMark> {
    // 80 points apart at least: egui_plot names hours only from 60 apart,
    // and fades them in up to 80.
    let mut marks = theme::even_grid(g, 80.0, &HOUR_STEPS);
    // Adding 0 turns the grid's -0 into 0, which is kept.
    marks.retain(|m| !(m.value + 0.0).is_sign_negative());
    marks
}

/// The height `n` charts need to read well, the hours under them
/// included.
pub fn read_h(n: usize) -> f32 {
    n as f32 * CHART_READ_H + AXIS_H
}

/// The hour of `spans` at `x` hours, if any.
fn hour_at(spans: &Spans, x: f64) -> Option<usize> {
    spans.iter().position(|&(a, b)| a <= x && x < b)
}

/// One width of value axis for every chart, so their plots start at the
/// same x: the least gutter, or the widest label any may draw, read from
/// each figure's whole-number ends with a decimal to spare.
fn shared_gutter(ui: &egui::Ui, charts: &[&Chart]) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut w = GUTTER;
    for c in charts {
        for s in c.steps.iter().flatten() {
            for end in [s.v.floor(), s.v.ceil()] {
                let t = true_minus(&format!("{end:.1}"));
                let wide = ui.fonts_mut(|f| {
                    f.layout_no_wrap(t, font.clone(), egui::Color32::WHITE)
                        .size()
                        .x
                });
                w = w.max(wide + 8.0);
            }
        }
    }
    w
}

/// The value range a chart shows: its steps and their bands.
fn y_range(c: &Chart) -> Option<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for s in c.steps.iter().flatten() {
        let e = s.se.unwrap_or(0.0);
        lo = lo.min(s.v - e);
        hi = hi.max(s.v + e);
    }
    if c.to_zero {
        lo = lo.min(0.0);
        hi = hi.max(0.0);
    }
    lo.is_finite().then_some((lo, hi))
}

/// The charts in `rect`, stacked, sharing its height, over `spans`
/// (the hours of every chart's steps), the hour `hover` marked by one
/// line through them all, its value in each title row and `axis_words`
/// for it under the hours. Gives the hour under the pointer, if any.
pub fn stack(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    charts: &[&Chart],
    spans: &Spans,
    hover: Option<usize>,
    axis_words: impl Fn(usize) -> String,
) -> Option<usize> {
    if charts.is_empty() {
        return None;
    }
    let end = spans.iter().map(|s| s.1).fold(1.0, f64::max);
    // A 5% margin each side, as egui's own, so 0 h clears the value axis.
    let x_range = (-end * 0.05, end * 1.05);
    let gutter = shared_gutter(ui, charts);
    // Never so short that one chart runs into the next: what doesn't fit
    // runs past the pane's foot, where it scrolls.
    let each = ((rect.height() - AXIS_H) / charts.len() as f32).max(TITLE_H + PLOT_MIN_H);
    let mut under = None;
    let mut bottom_frame = None;
    for (i, c) in charts.iter().enumerate() {
        let last = i + 1 == charts.len();
        let top = rect.top() + i as f32 * each;
        let band = egui::Rect::from_x_y_ranges(rect.x_range(), top..=top + TITLE_H);
        let plot_h = (each - TITLE_H + if last { AXIS_H } else { 0.0 }).max(PLOT_MIN_H);
        let plot_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), band.bottom()),
            egui::vec2(rect.width(), plot_h),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(plot_rect));
        let (frame, at) = one(&mut child, c, spans, (x_range, gutter, last), hover);
        under = under.or(at);
        title_row(ui, band, frame, c, hover);
        if last {
            bottom_frame = Some(frame);
        }
    }
    if let (Some(h), Some(frame)) = (hover, bottom_frame) {
        hover_words(ui, frame, x_range, spans[h], &axis_words(h));
    }
    under
}

/// One chart in `ui`: its bands, its steps, and the hover line; the hours
/// named under it when it is the `last`. Gives its plot's frame and the
/// hour under the pointer.
fn one(
    ui: &mut egui::Ui,
    c: &Chart,
    spans: &Spans,
    ((x_lo, x_hi), gutter, last): ((f64, f64), f32, bool),
    hover: Option<usize>,
) -> (egui::Rect, Option<usize>) {
    use egui_plot::{Line, Plot, Polygon, VLine};
    let pal = theme::pal(ui);
    let colour = series_colour(c.kind, pal);
    let mut grid = Grid::new(hour_marks, |g| theme::even_grid(g, 24.0, &[]));
    if c.signed {
        grid = grid.zero_y();
    }
    let mut plot = grid
        .on(Plot::new(("hour steps", c.kind.title())))
        .height(ui.available_height())
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_double_click_reset(false)
        .show_x(false)
        .show_y(false)
        .default_x_bounds(x_lo, x_hi)
        .y_axis_min_width(gutter)
        .show_axes([last, true])
        .x_axis_formatter(|m, _| format!("{} h", axis_number(m.value, m.step_size)))
        .y_axis_formatter(|m, _| true_minus(&axis_number(m.value, m.step_size)));
    if let Some((lo, hi)) = y_range(c) {
        plot = plot.include_y(lo).include_y(hi);
    }
    let resp = plot.show(ui, |p| {
        grid.draw(p, pal);
        for (s, &(a, b)) in c.steps.iter().zip(spans) {
            let Some(Step {
                v,
                se: Some(e),
                grey,
            }) = *s
            else {
                continue;
            };
            let ink = if grey { pal.text_tertiary } else { colour };
            p.polygon(
                Polygon::new("±", vec![[a, v - e], [b, v - e], [b, v + e], [a, v + e]])
                    .fill_color(ink.gamma_multiply(strength::BAND_FILL))
                    .stroke(egui::Stroke::NONE),
            );
        }
        for (pts, grey) in runs(&c.steps, spans) {
            let ink = if grey { pal.text_tertiary } else { colour };
            p.line(
                Line::new(c.kind.title(), pts)
                    .color(ink)
                    .width(strength::DATA_W),
            );
        }
        if let Some(&(a, b)) = hover.and_then(|h| spans.get(h)) {
            p.vline(
                VLine::new("hover", (a + b) / 2.0)
                    .color(pal.text_secondary)
                    .width(strength::FAINT_CALL_OUT_W),
            );
        }
    });
    let at = resp
        .response
        .hover_pos()
        .and_then(|pos| hour_at(spans, resp.transform.value_from_position(pos).x));
    (*resp.transform.frame(), at)
}

/// The steps as lines: one for each run of hours shown side by side with
/// the same ink, each hour level across its span and a rise where the next
/// begins; a run starts from the hour before's level where that hour is
/// shown, so the line is whole, and breaks where an hour isn't.
fn runs(steps: &[Option<Step>], spans: &Spans) -> Vec<(Vec<[f64; 2]>, bool)> {
    let mut out: Vec<(Vec<[f64; 2]>, bool)> = Vec::new();
    let mut prev: Option<(f64, f64)> = None;
    for (s, &(a, b)) in steps.iter().zip(spans) {
        let Some(s) = s else {
            prev = None;
            continue;
        };
        // Joined where the last hour ends as this one begins: a whole
        // hour's end is the next hour exactly.
        let joined = prev.filter(|&(_, end)| end >= a);
        match out.last_mut() {
            Some((pts, grey)) if joined.is_some() && *grey == s.grey => {
                pts.push([a, s.v]);
                pts.push([b, s.v]);
            }
            _ => {
                let mut pts = Vec::new();
                if let Some((v, _)) = joined {
                    pts.push([a, v]);
                }
                pts.push([a, s.v]);
                pts.push([b, s.v]);
                out.push((pts, s.grey));
            }
        }
        prev = Some((s.v, b));
    }
    out
}

/// A chart's title row in `band`: its name over the plot's left edge,
/// its value at the hovered hour after it in its colour, and its unit
/// over the value axis, which ends at `frame`'s left.
fn title_row(ui: &egui::Ui, band: egui::Rect, frame: egui::Rect, c: &Chart, hover: Option<usize>) {
    let pal = theme::pal(ui);
    let painter = ui.painter().with_clip_rect(band);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let y = band.center().y;
    painter.text(
        egui::pos2(frame.left() - 6.0, y),
        egui::Align2::RIGHT_CENTER,
        c.unit,
        font.clone(),
        pal.text_secondary,
    );
    let name = painter.text(
        egui::pos2(frame.left(), y),
        egui::Align2::LEFT_CENTER,
        c.kind.title(),
        font.clone(),
        pal.text_secondary,
    );
    if let Some(h) = hover {
        let words = match c.steps.get(h).copied().flatten() {
            // Degrees close up to the figure, as everywhere: "328°".
            Some(s) => {
                let gap = if c.unit == "°" { "" } else { " " };
                format!(
                    "{}{gap}{}",
                    figure_with_pm(s.v, s.se, c.digits, c.signed),
                    c.unit
                )
            }
            None => "–".to_string(),
        };
        let ink = match c.steps.get(h).copied().flatten() {
            Some(s) if !s.grey => series_colour(c.kind, pal),
            _ => pal.text_secondary,
        };
        painter.text(
            egui::pos2(name.right() + 8.0, y),
            egui::Align2::LEFT_CENTER,
            true_minus(&words),
            font,
            ink,
        );
    }
}

/// The hovered hour named under the bottom chart's `frame`, over its hour
/// labels, centred on the hover line.
fn hover_words(
    ui: &egui::Ui,
    frame: egui::Rect,
    (x_lo, x_hi): (f64, f64),
    (a, b): (f64, f64),
    words: &str,
) {
    let pal = theme::pal(ui);
    let t = ((a + b) / 2.0 - x_lo) / (x_hi - x_lo);
    let x = frame.left() + frame.width() * t as f32;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(words.to_string(), font, pal.text);
    let size = galley.size();
    // Kept inside the plot's width.
    let left = (x - size.x / 2.0)
        .max(frame.left())
        .min(frame.right() - size.x);
    let at = egui::pos2(left, frame.bottom() + 3.0);
    let back = egui::Rect::from_min_size(at, size).expand(2.0);
    ui.painter().rect_filled(back, 2.0, pal.card);
    ui.painter().galley(at, galley, pal.text);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(v: f64, grey: bool) -> Option<Step> {
        Some(Step { v, se: None, grey })
    }

    /// Hours side by side make one line, level across each and rising
    /// between; an hour not shown breaks it, and grey starts a new run
    /// from the level before.
    #[test]
    fn steps_join_level_and_break_at_gaps() {
        let spans = [(0.0, 1.0), (1.0, 2.0), (2.0, 3.0), (3.0, 4.0), (4.0, 4.5)];
        let steps = [
            step(1.0, false),
            step(2.0, false),
            None,
            step(3.0, false),
            step(4.0, true),
        ];
        let r = runs(&steps, &spans);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].0, vec![[0.0, 1.0], [1.0, 1.0], [1.0, 2.0], [2.0, 2.0]]);
        assert_eq!(r[1].0, vec![[3.0, 3.0], [4.0, 3.0]]);
        assert_eq!(r[2], (vec![[4.0, 3.0], [4.0, 4.0], [4.5, 4.0]], true));
    }

    /// A long run's margin left of 0 h, wider than an hour step, marks no
    /// hour before the run.
    #[test]
    fn no_hour_is_marked_before_the_run() {
        let marks = hour_marks(egui_plot::GridInput {
            bounds: (-2.3, 48.8),
            base_step_size: 0.2,
        });
        assert!(!marks.is_empty());
        assert!(marks.iter().all(|m| m.value >= 0.0), "{marks:?}");
        assert_eq!(marks[0].value, 0.0);
    }

    /// The hour at a time: its own span, the part hour's included.
    #[test]
    fn the_hour_at_a_time_is_the_one_whose_span_holds_it() {
        let spans = [(0.0, 1.0), (1.0, 1.4)];
        assert_eq!(hour_at(&spans, 0.5), Some(0));
        assert_eq!(hour_at(&spans, 1.2), Some(1));
        assert_eq!(hour_at(&spans, 1.5), None);
        assert_eq!(hour_at(&spans, -0.1), None);
    }
}
