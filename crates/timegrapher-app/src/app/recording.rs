//! The recording on one bench: the input chosen and its meter, the
//! capture and live analyser, a file being read, the input's clock and
//! the gaps, stalls and faults seen while listening.

use super::*;

/// Where the sound comes from and what is listening to it.
pub(super) struct Recording {
    pub(super) input: Input,
    pub(super) devices: Vec<InputDevice>,
    /// Whether `devices` has been filled. Listing the inputs asks each one
    /// what it supports, which briefly opens it and can disturb another
    /// program recording from it, so the app lists them only when the
    /// microphone menu opens, Rescan is pressed or listening starts.
    pub(super) devices_listed: bool,
    pub(super) device: Option<String>,
    pub(super) file_path: String,
    /// This session began with Quick Listen: kept only by Keep in a
    /// Watch…, or by Save… into Unsorted.
    pub(super) quick_listen: bool,
    /// Quick Listen starts on the first frame (Listen When the App Opens).
    pub(super) listen_pending: bool,
    /// Why the input last failed to open, for its row in Settings; none
    /// once one opens.
    pub(super) input_fault: Option<String>,
    /// The file dialog asked for, shown once the frame lets the app go.
    pub(super) picker: Option<crate::pick::Picker>,
    /// The check at start is still to be made (on the first frame, which
    /// has the context it repaints).
    pub(super) check_pending: bool,
    /// The USB lines of the sound heard here, read as it comes (#1721).
    pub(super) live_usb: Option<live_usb::LiveUsb>,
    pub(super) capture: Option<Capture>,
    pub(super) live: Option<LiveAnalyzer>,
    pub(super) source_label: String,
    pub(super) batch: Option<Batch>,
    /// The Listen button since it last stood in for Stop: see
    /// [`listen_tap`] (#725).
    pub(super) listen_tap: Option<(f64, bool)>,
    pub(super) anchor: Option<Anchor>,
    /// While a take opens: a beat rate its settings change reads nothing
    /// again, the take before least of all (#1241).
    pub(super) opening: bool,
    /// The input's level meter, level control and automatic gain switch.
    pub(super) meter: input_meter::InputMeter,
    /// The stored clock error of the input this session listens to, if one
    /// was measured (`timegrapher clock measure --save`).
    pub(super) clock: Option<clockstore::DeviceClock>,
    /// The microphone's clock timed against the computer's as its audio
    /// arrives: offered to keep once it has run long enough.
    pub(super) clock_track: Option<ClockTrack>,
    /// A take kept as its analysis alone as it records (#1514).
    pub(super) kept_live: kept_analysis::Kept,
    /// The jumps in the live sound's beats as of the last pass, to see a
    /// new one.
    pub(super) jumps_seen: Vec<f64>,
    /// When live's first chunk read again took over and why it was not,
    /// as last seen (#1327).
    pub(super) rebuild_seen: RebuildSeen,
    /// Offer every input the system lists, not just one per microphone.
    pub(super) all_inputs: bool,
    /// When the microphone was opened and when it last sent audio.
    pub(super) opened_at: Instant,
    pub(super) audio_at: Option<Instant>,
    /// Core's gap rule over a live input's blocks: a break where the
    /// computer slept or the input stalled.
    pub(super) gaps: Option<GapWatch>,
    /// When the sound was last taken in, to tell when nothing ran at all
    /// (the computer asleep, a dialog open) from an input gone quiet.
    pub(super) polled_at: Instant,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            input: Input::Microphone,
            devices: Vec::new(),
            devices_listed: true,
            device: None,
            file_path: String::new(),
            quick_listen: false,
            listen_pending: false,
            input_fault: None,
            picker: None,
            check_pending: false,
            live_usb: None,
            capture: None,
            live: None,
            source_label: String::new(),
            batch: None,
            listen_tap: None,
            anchor: None,
            opening: false,
            meter: input_meter::InputMeter::default(),
            clock: None,
            clock_track: None,
            kept_live: Default::default(),
            jumps_seen: Vec::new(),
            rebuild_seen: RebuildSeen::default(),
            all_inputs: false,
            opened_at: Instant::now(),
            audio_at: None,
            gaps: None,
            polled_at: Instant::now(),
        }
    }
}

impl TimegrapherApp {
    /// List the inputs, once, and keep the chosen one if it is still there,
    /// else take the first.
    pub(super) fn ensure_devices(&mut self) {
        if !self.bench.recording.devices_listed {
            self.rescan();
        }
    }

    /// List the inputs again, after one is plugged in.
    pub(super) fn rescan(&mut self) {
        self.bench.recording.devices = capture::list().unwrap_or_default();
        self.bench.recording.devices_listed = true;
        let there = self
            .bench
            .recording
            .device
            .as_ref()
            .is_some_and(|d| self.bench.recording.devices.iter().any(|x| &x.id == d));
        if !there {
            self.bench.recording.device = capture::choices(&self.bench.recording.devices)
                .first()
                .map(|c| c.id.clone());
        }
    }

    /// The input didn't open: why, kept for its row in Settings, and said.
    pub(super) fn input_failed(&mut self, e: &str) {
        let m = format!("Can't open the input. {}", capture::explain_error(e));
        self.bench.recording.input_fault = Some(m.clone());
        self.error(m);
    }
}

/// When live's first chunk read again took over, and why it was not
/// ([`LiveAnalyzer::template_switch_s`],
/// [`LiveAnalyzer::template_rebuild_given_up`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct RebuildSeen {
    pub(super) switch_s: Option<f64>,
    pub(super) given_up: Option<&'static str>,
}

impl RebuildSeen {
    /// As `live` has it now.
    pub(super) fn of(live: &LiveAnalyzer) -> Self {
        Self {
            switch_s: live.template_switch_s(),
            given_up: live.template_rebuild_given_up(),
        }
    }
}
