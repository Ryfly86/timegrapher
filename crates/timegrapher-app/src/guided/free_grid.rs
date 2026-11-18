//! The This Take table of a free take: the six positions with their
//! Average and Largest Difference, then any 45° stops read (#1941).

use super::*;

/// The free take's table: Position, Duration, Rate, Amplitude and Beat
/// Error, each reading in its own colour, then the mean and the largest
/// difference once two positions have readings, then the 45° stops.
pub(super) fn free_grid(ui: &mut egui::Ui, t: &FreeTable) {
    let pal = theme::pal(ui);
    let (mut tops, mut extra_tops) = (Vec::new(), Vec::new());
    let grid = egui::Grid::new("free-positions")
        .num_columns(5)
        .spacing(Vec2::new(16.0, 5.0))
        .min_col_width(LABEL_W.min(FIGURE_W))
        .striped(true)
        .show(ui, |ui| {
            position_head(ui);
            for (h, unit, hint) in [
                (
                    "Duration",
                    "",
                    "Time marked in the position, less its settling and masked time",
                ),
                ("Rate", "s/d", "Seconds a day gained (+) or lost (−)"),
                (
                    "Amplitude",
                    "°",
                    "The balance's swing, from the lift angle set",
                ),
                ("Beat Error", "ms", "From the unlock, as a size"),
            ] {
                // Beat error's figures are in the text colour in light
                // mode: its swatch beside the heading says whose they are.
                let be = pal.trace_beat_error;
                let swatch = (h == "Beat Error" && pal.ink(be) != be).then_some(be);
                column_head_swatch(ui, FIGURE_W, h, unit, hint, swatch);
            }
            ui.end_row();
            for (label, duration, cell) in &t.rows {
                tops.push(ui.cursor().top());
                free_row(ui, (label, *duration, cell), None);
            }
            tops.push(ui.cursor().top());
            if t.read >= 2 {
                free_means(ui, t);
            }
            // The 45° stops after the six's Average and Largest
            // Difference, set apart by a rule: in neither (Hugh on #1964).
            for (label, _, duration, cell) in &t.extra {
                extra_tops.push(ui.cursor().top());
                free_row(ui, (label, *duration, cell), Some(FORTY_FIVE_HINT));
            }
            extra_tops.push(ui.cursor().top());
        });
    let x = grid.response.rect.x_range();
    let six: Vec<_> = card_order().into_iter().map(|p| p.common()).collect();
    crate::position_lanes::table_rows(ui, (x, 5.0), &six, &tops);
    if !t.extra.is_empty() {
        let codes: Vec<_> = t.extra.iter().map(|e| e.1).collect();
        crate::position_lanes::table_rows(ui, (x, 5.0), &codes, &extra_tops);
        ui.painter().hline(
            x,
            extra_tops[0] - 2.5,
            egui::Stroke::new(1.0_f32, pal.text_tertiary),
        );
    }
    crate::position_lanes::note_table_w(ui, grid.response.rect.width());
}

/// The free take's table from core's, in the Current Position card's
/// order, with its `six` positions and the take's `upright` stops as
/// readings.
pub fn free_table(
    t: &session::PositionsTable,
    six: TakeReadings,
    (upright, upright_rows): (TakeReadings, &[session::PositionRow]),
) -> FreeTable {
    let rows: Vec<(String, f64, FreeCell)> = card_rows(t)
        .into_iter()
        .map(|r| (position_label(r.position), r.duration_s, cell_of(&r)))
        .collect();
    let read = rows
        .iter()
        .filter(|r| matches!(r.2, FreeCell::Read(..)))
        .count();
    let six_of = card_order();
    let marked = |r: &&session::PositionRow| cell_of(r) != FreeCell::NotMarked;
    let extra = upright_rows
        .iter()
        .filter(|r| !six_of.contains(&r.position))
        .filter(marked)
        .map(|r| {
            let (l, d) = (position_label(r.position), r.duration_s);
            (l, r.position.common(), d, cell_of(r))
        })
        .collect();
    let short = upright_rows
        .iter()
        .filter(marked)
        .filter(|r| r.measurement.is_none())
        .map(|r| (position_label(r.position), r.duration_s))
        .collect();
    FreeTable {
        rows,
        extra,
        mean: [
            t.indices.x,
            t.indices.amplitude_mean,
            t.indices.beat_error_mean,
        ],
        largest: [
            t.indices.d_rate,
            t.indices.d_amplitude,
            t.indices.d_beat_error,
        ],
        read,
        six,
        upright,
        short,
    }
}

/// A marked position's cell: read, heard with nothing read, or too short.
fn cell_of(r: &session::PositionRow) -> FreeCell {
    match &r.measurement {
        Some(m) => match (m.rate_s_per_day, m.amplitude_deg, m.beat_error()) {
            (None, None, None) => FreeCell::Unread,
            (rate, amp, be) => FreeCell::Read(rate, amp, be),
        },
        None if r.duration_s > 0.0 => FreeCell::Short,
        None => FreeCell::NotMarked,
    }
}

/// One stop's row: its name (with `hint` on hover), Duration, Rate,
/// Amplitude and Beat Error, as far as read.
fn free_row(
    ui: &mut egui::Ui,
    (label, duration, cell): (&str, f64, &FreeCell),
    hint: Option<&str>,
) {
    let pal = theme::pal(ui);
    let marked = *cell != FreeCell::NotMarked;
    let color = if marked { pal.text } else { pal.text_tertiary };
    let name = ui.label(RichText::new(label).color(color));
    if let Some(h) = hint {
        name.on_hover_text(h);
    }
    match cell {
        FreeCell::NotMarked => {
            right(ui, "Not marked".into(), false, pal.text_tertiary);
            let figures = 3;
            for _ in 0..figures {
                ui.label("");
            }
        }
        FreeCell::Short => {
            right(ui, table_duration(duration), false, pal.text);
            under_10_s_cell(ui, FIGURE_W);
            for _ in 0..2 {
                ui.label("");
            }
        }
        FreeCell::Unread => {
            right(ui, table_duration(duration), false, pal.text);
            right(ui, "No reading".into(), false, pal.text_secondary);
            for _ in 0..2 {
                ui.label("");
            }
        }
        FreeCell::Read(rate, amp, be) => {
            right(ui, table_duration(duration), false, pal.text);
            right(ui, signed(*rate, RATE), false, pal.trace_rate);
            right(ui, plain(*amp, AMPLITUDE), false, pal.trace_amplitude);
            right(
                ui,
                plain(*be, BEAT_ERROR),
                false,
                pal.ink(pal.trace_beat_error),
            );
        }
    }
    ui.end_row();
}

/// The hover on a 45° stop's row.
pub(crate) const FORTY_FIVE_HINT: &str =
    "Read in a Quick Listen. Not in Average or Largest Difference.";

/// A free take's table's Average and Largest Difference rows.
fn free_means(ui: &mut egui::Ui, t: &FreeTable) {
    let pal = theme::pal(ui);
    let inks = [
        pal.trace_rate,
        pal.trace_amplitude,
        pal.ink(pal.trace_beat_error),
    ];
    ui.label(RichText::new("Average").strong().color(pal.text))
        .on_hover_text("Over the positions read");
    right(ui, format!("{} of 6", t.read), false, pal.text_secondary);
    right(ui, signed(t.mean[0], RATE), true, inks[0]);
    right(ui, plain(t.mean[1], AMPLITUDE), true, inks[1]);
    right(ui, plain(t.mean[2], BEAT_ERROR), true, inks[2]);
    ui.end_row();
    largest_label(ui);
    ui.label("");
    right(ui, plain(t.largest[0], RATE), true, inks[0]);
    right(ui, plain(t.largest[1], AMPLITUDE), true, inks[1]);
    right(ui, plain(t.largest[2], BEAT_ERROR), true, inks[2]);
    ui.end_row();
}
