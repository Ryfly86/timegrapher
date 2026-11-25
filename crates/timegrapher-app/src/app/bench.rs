//! One bench: the recording, what the take is read with, the guided
//! test, the take's readings, how its panes look and what of it is kept.
//! Bench tabs (#1862) hold one of these each.

use super::*;

/// Everything that belongs to the take on one bench.
pub(super) struct Bench {
    /// How the strip and charts look and follow.
    pub(super) panes_view: pane_view::PaneView,
    /// The guided tests for this bench.
    pub(super) test: guided_test::GuidedTest,
    /// The take's readings and the runs in the background that read them.
    pub(super) results: results::Results,
}
