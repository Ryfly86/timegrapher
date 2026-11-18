//! The This Take table of a free take: the six positions, then any 45°
//! stops read (#1941), with their Average and Largest Difference.

use super::*;

/// The free take's table: Position, Duration, Rate, Amplitude and Beat
/// Error, each reading in its own colour, then the mean and the largest
/// difference once two positions have readings.
pub(super) fn free_grid(ui: &mut egui::Ui, t: &FreeTable) {
    let pal = theme::pal(ui);
    let mut tops = Vec::new();
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
            let six = t.rows.iter().map(|(l, d, c)| (l, d, c));
            let extra = t.extra.iter().map(|(l, _, d, c)| (l, d, c));
            for (label, duration, cell) in six.chain(extra) {
                tops.push(ui.cursor().top());
                let marked = *cell != FreeCell::NotMarked;
                let color = if marked { pal.text } else { pal.text_tertiary };
                ui.label(RichText::new(label).color(color));
                match cell {
                    FreeCell::NotMarked => {
                        right(ui, "Not marked".into(), false, pal.text_tertiary);
                        for _ in 0..3 {
                            ui.label("");
                        }
                    }
                    FreeCell::Short => {
                        right(ui, table_duration(*duration), false, pal.text);
                        right(ui, "Under 10 s".into(), false, pal.text_secondary);
                        for _ in 0..2 {
                            ui.label("");
                        }
                    }
                    FreeCell::Unread => {
                        right(ui, table_duration(*duration), false, pal.text);
                        right(ui, "No reading".into(), false, pal.text_secondary);
                        for _ in 0..2 {
                            ui.label("");
                        }
                    }
                    FreeCell::Read(rate, amp, be) => {
                        right(ui, table_duration(*duration), false, pal.text);
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
            tops.push(ui.cursor().top());
            if t.read >= 2 {
                free_means(ui, t);
            }
        });
    let codes: Vec<_> = card_order()
        .into_iter()
        .map(|p| p.common())
        .chain(t.extra.iter().map(|e| e.1))
        .collect();
    crate::position_lanes::table_rows(ui, (grid.response.rect.x_range(), 5.0), &codes, &tops);
    crate::position_lanes::note_table_w(ui, grid.response.rect.width());
}

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
