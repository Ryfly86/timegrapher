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
    /// What the take on show is read with: the watch, its beat rate and
    /// lift, the positions, masks and clock, and what its readings rest on.
    pub(super) take: take_settings::TakeSettings,
    /// Keeping and saving the take on show: kept sounds, Keep in a
    /// Watch…, saves under way, opening and the crash note.
    pub(super) keeping: keeping::Keeping,
    /// The recording on this bench: the input and its meter, listening,
    /// the live analyser, the file read and the input's clock.
    pub(super) recording: recording::Recording,
}

impl Bench {
    /// A bench listening to the first of `devices`, or reading `file`.
    pub(super) fn new(devices: Vec<InputDevice>, file: Option<&PathBuf>) -> Self {
        let device = capture::choices(&devices).first().map(|c| c.id.clone());
        Bench {
            panes_view: pane_view::PaneView::default(),
            test: guided_test::GuidedTest::default(),
            results: results::Results::default(),
            take: take_settings::TakeSettings::default(),
            keeping: keeping::Keeping::default(),
            recording: recording::Recording {
                input: if file.is_some() {
                    Input::File
                } else {
                    Input::Microphone
                },
                devices,
                device,
                file_path: file
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                ..Default::default()
            },
        }
    }
}
