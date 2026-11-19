//! The Complication Test's table and its text (#1576).

use super::*;

/// The Complication Test's table (#1576, Hugh's spec): a row for each
/// stretch, Off, On, Off, with amplitude first, then On less Off, core's
/// difference with its ±. `status` names what the next row is doing while
/// the test runs, and `live` its figures so far.
pub(super) fn complication_grid(
    ui: &mut egui::Ui,
    t: &Test,
    status: Option<String>,
    live: Option<&FreeCell>,
) {
    let pal = theme::pal(ui);
    let readings = t.readings_so_far();
    let measurements: Vec<Measurement> = readings.iter().map(|r| r.measurement.clone()).collect();
    let mut tops = Vec::new();
    let grid = egui::Grid::new("complication")
        .num_columns(4)
        .spacing(Vec2::new(GAP, 5.0))
        .min_col_width(FIGURE_W)
        .striped(true)
        .show(ui, |ui| {
            let size = Vec2::new(FIGURE_W, ui.spacing().interact_size.y);
            ui.allocate_ui_with_layout(
                size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(size);
                    ui.label(theme::caption("Stretch").color(pal.text_secondary));
                },
            );
            for q in complication::SHOWN {
                column_head(
                    ui,
                    FIGURE_W,
                    q.title(),
                    complication::unit_head(q),
                    complication_hint(q),
                );
            }
            ui.end_row();
            let mut status = status;
            for i in 0..t.order().len() {
                tops.push(ui.cursor().top());
                let label = t.step_code(i);
                if let Some(r) = readings.get(i) {
                    let f = complication::figures(&r.measurement);
                    ui.label(RichText::new(label).color(pal.text));
                    for q in complication::SHOWN {
                        let cell = right(ui, figure_cell(q, &f), false, pal.text);
                        if q == SeriesKind::BeatError {
                            // Its source, as the Positions table hovers it.
                            let src = r.measurement.beat_error_source;
                            cell.on_hover_text(session::beat_error_source_label(src));
                        }
                    }
                } else {
                    match (status.take(), live) {
                        (Some(_), Some(FreeCell::Read(rate, amp, be))) => {
                            ui.label(RichText::new(label).strong().color(pal.text))
                                .on_hover_text("Being measured: its figures so far");
                            right(ui, plain(*amp, AMPLITUDE), false, pal.accent);
                            right(ui, signed(*rate, RATE), false, pal.accent);
                            right(ui, plain(*be, BEAT_ERROR), false, pal.accent);
                        }
                        (Some(st), _) => {
                            ui.label(RichText::new(label).strong().color(pal.text));
                            right(ui, st, false, pal.accent);
                            ui.label("");
                            ui.label("");
                        }
                        (None, _) => {
                            ui.label(RichText::new(label).color(pal.text_tertiary));
                            for _ in 0..3 {
                                ui.label("");
                            }
                        }
                    }
                }
                ui.end_row();
            }
            tops.push(ui.cursor().top());
            if measurements.len() == complication::ORDER.len() {
                ui.label(
                    RichText::new(complication::DIFFERENCE_NAME)
                        .strong()
                        .color(pal.text),
                )
                .on_hover_text(complication::method_note());
                for q in complication::SHOWN {
                    let d = complication::change(q, &measurements).map_or_else(
                        || "—".to_string(),
                        |c| true_minus(&complication::difference_text(q, c.on_less_off)),
                    );
                    right(ui, d, true, pal.text);
                }
                ui.end_row();
            }
        });
    // The hovered stretch's row outlined, as the plots' (#1729).
    let rows: Vec<usize> = (0..t.order().len()).collect();
    let x = (grid.response.rect.x_range(), 5.0);
    crate::position_lanes::rows_hovered(ui, x, (&rows, &tops), complication_hover_ids());
}

/// Where the Complication pane keeps the stretch hovered in its table or
/// its plots, and the one under the pointer in its table (#1729).
pub(crate) fn complication_hover_ids() -> (egui::Id, egui::Id) {
    (
        egui::Id::new("complication hover"),
        egui::Id::new("complication table hover"),
    )
}

/// A stretch's figure as the table and Copy write it: core's number at
/// the series' place, the rate signed ([`SeriesKind::number`]).
pub(super) fn figure_cell(q: SeriesKind, f: &complication::Figures) -> String {
    complication::figure(q, f).map_or_else(|| "—".to_string(), |f| true_minus(&q.number(f.value)))
}

/// A Complication Test column's hover.
pub(super) fn complication_hint(q: SeriesKind) -> &'static str {
    match q {
        SeriesKind::Amplitude => {
            "The balance's swing, from the lift angle set. A complication that loads the \
             going train lowers it."
        }
        SeriesKind::Rate => "Seconds a day gained (+) or lost (−)",
        SeriesKind::BeatError => {
            "As a size, from the unlock, or from the drop where the unlock isn't found."
        }
    }
}

/// The Complication Test's table as text a spreadsheet takes: a header,
/// a row for each stretch, then On less Off with its ±. Fields are split
/// by `sep`.
pub(super) fn complication_text(test: &Test, sep: char) -> String {
    let readings = test.readings_so_far();
    let mut out = "Stretch".to_string();
    for q in complication::SHOWN {
        out.push_str(&format!(
            "{sep}{} ({})",
            q.title(),
            complication::unit_head(q)
        ));
    }
    out.push('\n');
    for (i, r) in readings.iter().enumerate() {
        let f = complication::figures(&r.measurement);
        out.push_str(test.step_code(i));
        for q in complication::SHOWN {
            out.push_str(&format!("{sep}{}", figure_cell(q, &f)));
        }
        out.push('\n');
    }
    let m: Vec<Measurement> = readings.iter().map(|r| r.measurement.clone()).collect();
    if m.len() == complication::ORDER.len() {
        out.push_str(complication::DIFFERENCE_NAME);
        for q in complication::SHOWN {
            let d = complication::change(q, &m).map_or_else(
                || "—".to_string(),
                |c| true_minus(&complication::difference_text(q, c.on_less_off)),
            );
            out.push_str(&format!("{sep}{d}"));
        }
        out.push('\n');
        for q in complication::SHOWN {
            if let Some(c) = complication::change(q, &m) {
                out.push_str(&format!(
                    "{}: {}\n",
                    q.title(),
                    true_minus(&complication::finding_text(q, c.on_less_off))
                ));
            }
        }
    }
    out
}
