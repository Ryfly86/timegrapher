//! The Parallel Guides drawn only where the session has run.

use super::*;

/// Every guide piece's two ends, along the strip's time axis, in points.
fn guide_ends(
    shapes: Vec<egui::epaint::ClippedShape>,
    faint: Color32,
    horizontal: bool,
) -> Vec<f32> {
    fn walk(s: &egui::Shape, faint: Color32, horizontal: bool, out: &mut Vec<f32>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, faint, horizontal, out)),
            egui::Shape::LineSegment { points, stroke } if stroke.color == faint => {
                out.extend(points.iter().map(|p| if horizontal { p.x } else { p.y }));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &shapes {
        walk(&c.shape, faint, horizontal, &mut out);
    }
    out
}

/// A 16 minute take on a 24 h strip: the session takes the last hundredth
/// of the strip, and the paper before it stays blank. The guides were
/// drawn across all of it, steep and close, a hatched field (#1952).
#[test]
fn the_guides_start_where_the_session_does() {
    let p = 0.125;
    let beats: Vec<Beat> = (0..7680)
        .map(|k| Beat {
            index: k,
            time: k as f64 * p * (1.0 - 10.0 / 86400.0),
            quality: 1.0,
        })
        .collect();
    let end = beats[beats.len() - 1].time;
    for horizontal in [true, false] {
        let ctx = egui::Context::default();
        let view = StripView {
            half_width_ms: 20.0,
            span_s: 86400.0,
            horizontal,
            still: false,
        };
        let extras = Extras {
            rate_line: Some(RateLine {
                from_s: 0.0,
                to_s: end,
                rate_s_per_day: 10.0,
            }),
            guides: true,
            ..Default::default()
        };
        let (mut drawn, mut faint) = (Rect::NOTHING, Color32::TRANSPARENT);
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    drawn = ui.available_rect_before_wrap();
                    faint = crate::theme::pal(ui).trace_rate.gamma_multiply(0.3);
                    draw_strip(
                        ui,
                        &beats,
                        p,
                        Some(Anchor {
                            index: 0,
                            time: 0.0,
                        }),
                        end,
                        &view,
                        &extras,
                        Vec2::new(760.0, 760.0),
                    );
                });
            },
        );
        let ends = guide_ends(out.shapes, faint, horizontal);
        assert!(
            !ends.is_empty(),
            "no guides drawn (horizontal {horizontal})"
        );
        // The session's 16 min of the 24 h: about 9 pt at the strip's
        // newest end, the right across and the top down.
        let (newest, along) = if horizontal {
            (drawn.right(), drawn.width())
        } else {
            (drawn.top(), drawn.height())
        };
        let furthest = ends.iter().map(|e| (newest - e).abs()).fold(0.0, f32::max);
        assert!(
            furthest < along * 0.05,
            "a guide {furthest} pt back from the newest end (horizontal {horizontal})"
        );
    }
}
