//! How often the window draws: as often as what it shows changes, and no
//! more, so a window left listening for days isn't load on the graphics
//! card for nothing.

use super::*;
use std::time::Duration;

/// How often the window draws while listening or reading a take: ten
/// times a second keeps the strip's new beats and the readings moving as
/// they come, at under half the draws of the 25 to 30 a second before.
const LIVE_EVERY: Duration = Duration::from_millis(100);
/// The least wait between draws while the window isn't the one in use:
/// what it shows still moves twice a second.
const BEHIND_EVERY: Duration = Duration::from_millis(500);

/// Whether the shared cursor's change from `was` to `next` draws again at
/// once: when the pointer `moved` or left, or anything else moved it with
/// the window not `listening` (a pan or zoom under a still pointer). Not
/// when only the live strip moved under a still pointer, which drew every
/// frame twice while listening; the next timed draw shows it.
pub(super) fn cursor_redraw(
    was: Option<f64>,
    next: Option<f64>,
    moved: bool,
    listening: bool,
) -> bool {
    next != was && (next.is_none() || moved || !listening)
}

impl TimegrapherApp {
    /// How soon the window draws again with no input: while listening or
    /// reading a take, and while work on its own thread is under way, so
    /// what it gives shows (and a take kept as its analysis only lets its
    /// sound go, #1221) without waiting for the pointer to move.
    pub(super) fn repaint_every(&self) -> Option<Duration> {
        let working = self.results.sound_job.is_some()
            || self.results.scope_job.is_some()
            || self.results.steady_job.is_some()
            || self.keeping.saving.is_some()
            || self.keeping.take_keep.is_some()
            || self.keeping.take_add.is_some()
            || self.keeping.analysis_save.is_some()
            || self.results.bg.test_read.is_some()
            || self.kept_writing()
            || self
                .session
                .recorder
                .as_ref()
                .is_some_and(Recorder::releasing);
        if self.running() || self.batch.is_some() {
            Some(LIVE_EVERY)
        } else {
            working
                .then_some(Duration::from_millis(100))
                .or_else(|| self.reserve_retry_in().or(self.kept_retry_in()))
        }
    }

    /// When the window draws next: as [`Self::repaint_every`] says, and
    /// once more at once on the frame the work ended, so what it left (a
    /// message, the take's files) shows with no input (Walter on #1232).
    /// A window not `in_use` (another window in front, or minimised)
    /// waits at least [`BEHIND_EVERY`]: the listening goes on without its
    /// frames ([`crate::pump`]), and a draw it isn't looked at for is
    /// load on the graphics card for nothing.
    pub(super) fn draw_next(&self, was_working: bool, in_use: bool) -> Option<Duration> {
        let next = self
            .repaint_every()
            .or(was_working.then_some(Duration::ZERO));
        if in_use {
            next
        } else {
            next.map(|d| d.max(BEHIND_EVERY))
        }
    }

    /// At the end of a frame: when the next one draws. The cursor shared
    /// by the strip and the charts follows the pointer, so it draws again
    /// at once when the pointer moves or leaves and every chart's line
    /// follows.
    pub(super) fn draw_again(&mut self, ctx: &egui::Context, was_working: bool) {
        let next = self.panes_view.cursor_next.take();
        let moved = ctx.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
        if cursor_redraw(self.panes_view.cursor_t, next, moved, self.running()) {
            ctx.request_repaint();
        }
        self.panes_view.cursor_t = next;
        let in_use = ctx.input(|i| {
            let v = i.viewport();
            v.focused != Some(false) && v.minimized != Some(true)
        });
        if let Some(every) = self.draw_next(was_working, in_use) {
            ctx.request_repaint_after(every);
        }
    }
}
