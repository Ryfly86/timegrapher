//! Until live builds a file's first-chunk template again (#1327), its
//! own beats are timed on the template it locked on with, so its figures
//! would part from a saved take's of the same sound (#1987). Until then,
//! everything heard is read again as a file reads it
//! ([`crate::stream::analyze_samples`]), back to back on one background
//! thread, and the figures come only from the newest read finished: a
//! reading states the end of the sound that read covered, and equals the
//! saved take of that length. Live's own beats still draw the strip.
//! A stop reads everything heard once more, so the stopped figures are
//! the saved take's (Norm's ruling on #1987).

use super::*;
use std::thread::JoinHandle;
use std::time::Instant;

/// How much sound, seconds, is read again as a file at most: twice a
/// file's first chunk. Past it the figures are live's own even if the
/// template was never built again (the watch first heard late, or never),
/// so a read never grows past what the lag allows.
pub const AS_FILE_MAX_S: f64 = 2.0 * crate::stream::CHUNK_S;

/// How far, seconds, the figures may fall behind the sound heard, while
/// it is pushed faster than real time (a test, a headless read): the
/// newest finished read's end against all heard. Past it such a push waits
/// for the read in flight. Heard as it comes, nothing waits, so the strip
/// never trails the watch; the figures then trail it by two reads at most,
/// about 1.2 s for the most heard before the template is built again. The
/// app's feed never waits (Norm's ruling on #1987).
pub const READ_LAG_CAP_S: f64 = 0.5;

/// The sound read again as a file, the read in flight, and the newest
/// finished, ready to give the figures.
#[derive(Default)]
pub(super) struct AsFile {
    /// Off on a copy that isn't the reading shown (the template's reread,
    /// a reading held from before a rate change), and once the figures
    /// are live's own for good.
    on: bool,
    /// Everything heard since the start.
    heard: Vec<f32>,
    /// The read in flight, and how many samples of `heard` it reads.
    job: Option<(JoinHandle<BeatLog>, usize)>,
    /// The newest finished read, set as live is.
    view: Option<Box<LiveAnalyzer>>,
    /// When the first sound came, to tell a feed faster than real time.
    since: Option<Instant>,
}

/// A copy keeps the newest read and what was heard; a read in flight
/// stays with the original.
impl Clone for AsFile {
    fn clone(&self) -> Self {
        AsFile {
            on: self.on,
            heard: self.heard.clone(),
            job: None,
            view: self.view.clone(),
            since: self.since,
        }
    }
}

impl std::fmt::Debug for AsFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsFile")
            .field("on", &self.on)
            .field("heard", &self.heard.len())
            .field("reading", &self.job.as_ref().map(|j| j.1))
            .field("view_s", &self.view.as_ref().map(|v| v.settled_to))
            .finish()
    }
}

impl LiveAnalyzer {
    /// This analyzer with its figures read as a file until the template
    /// is built again, as a screen showing a watch heard live needs them
    /// (#1987). Without it every figure is live's own, as a reading that
    /// must not hang on a background thread's timing (a test, a
    /// headless print) needs.
    pub fn with_figures_as_file(mut self) -> Self {
        self.as_file = AsFile {
            on: true,
            ..Default::default()
        };
        self
    }

    /// The figures' analyzer: the newest read as a file while those give
    /// them, else this one.
    pub(super) fn figures(&self) -> Option<&LiveAnalyzer> {
        self.as_file.view.as_deref().filter(|_| self.as_file.on)
    }

    /// Whether the figures still come from reads as a file: until the
    /// template is built again from the first chunk, or that is given up,
    /// and within [`AS_FILE_MAX_S`].
    pub fn reads_as_file(&self) -> bool {
        self.as_file.on
    }

    /// The end of the sound the newest read as a file covered, seconds,
    /// while the figures come from those reads.
    pub fn read_as_file_to(&self) -> Option<f64> {
        self.figures().map(|v| v.settled_to)
    }

    /// The reading over the `average_s` seconds before `end_s`, while the
    /// figures come from reads as a file: live's listening state (the
    /// watch heard or not, the beat being found, the sound's level and
    /// clipping) at `end_s`, with the newest read's figures to `read_end_s`
    /// or the end of the sound it covered if sooner, where live would show
    /// figures at all; none before the first read.
    pub(super) fn reading_as_file(
        &self,
        end_s: f64,
        read_end_s: f64,
        average_s: f64,
    ) -> Option<LiveReading> {
        if !self.as_file.on {
            return None;
        }
        let own = self.own_reading_at(end_s, average_s);
        // Live shows no figures yet: none from the read either.
        if !own.watch_heard || own.finding_beat || own.listening.is_some() {
            return Some(own);
        }
        let read = (self.as_file.view.as_ref())
            .map(|v| v.reading_at(read_end_s.min(v.settled_to), average_s));
        Some(match read {
            Some(r) => with_read_figures(own, r, self.sides_known_at(end_s)),
            None => without_figures(own),
        })
    }

    /// The trend while the figures come from reads as a file: the newest
    /// read's.
    pub(super) fn trend_as_file(&self, average_s: f64) -> Option<Vec<TrendReading>> {
        if !self.as_file.on {
            return None;
        }
        Some(match &self.as_file.view {
            Some(v) => v.trend(average_s),
            None => Vec::new(),
        })
    }

    /// Whether the sound comes faster than real time and the figures are
    /// more than [`READ_LAG_CAP_S`] behind it, so a push waits for the read
    /// in flight. Takes a finished read's figures and starts the next, as
    /// a push does. Never waits itself.
    fn reads_behind(&mut self) -> bool {
        if !self.as_file.on {
            return false;
        }
        if self.as_file.job.as_ref().is_some_and(|j| j.0.is_finished()) {
            self.take_read();
        }
        if self.as_file.job.is_none() && self.as_file.heard.len() > self.view_len() {
            self.start_read();
        }
        let heard_s = self.as_file.heard.len() as f64 / self.fs;
        let read_to = self.as_file.view.as_ref().map_or(0.0, |v| v.settled_to);
        // Sound heard as it comes is never ahead of the clock by more than
        // a block or two the card holds back.
        let ahead = (self.as_file.since)
            .is_some_and(|t| heard_s - t.elapsed().as_secs_f64() > READ_LAG_CAP_S);
        ahead && self.as_file.job.is_some() && heard_s - read_to > READ_LAG_CAP_S
    }

    /// No more reads as a file: the figures are this analyzer's own.
    pub(super) fn stop_reading_as_file(&mut self) {
        self.as_file = AsFile::default();
    }

    /// A setting changed: the newest read as a file given it too.
    pub(super) fn in_view(&mut self, f: impl FnOnce(&mut LiveAnalyzer)) {
        if let Some(v) = &mut self.as_file.view {
            f(v);
        }
    }

    /// After a push of `samples`: keeps them, takes a finished read's
    /// figures (waiting for it first, if the feed `wait`s, while the
    /// figures are more than [`READ_LAG_CAP_S`] behind a feed faster than
    /// real time), and starts the next read on everything heard. True when
    /// the figures changed.
    pub(super) fn as_file_after_push(&mut self, samples: &[f32], wait: bool) -> bool {
        if !self.as_file.on {
            return false;
        }
        if self.template_settled() || self.duration_s() > AS_FILE_MAX_S {
            self.stop_reading_as_file();
            return true;
        }
        let mut changed = wait && self.reads_behind() && self.take_read();
        self.as_file.since.get_or_insert_with(Instant::now);
        self.as_file.heard.extend_from_slice(samples);
        if self.as_file.job.as_ref().is_some_and(|j| j.0.is_finished()) {
            changed |= self.take_read();
        }
        if self.as_file.job.is_none() && self.as_file.heard.len() > self.view_len() {
            self.start_read();
        }
        changed
    }

    /// Samples the newest read covered.
    fn view_len(&self) -> usize {
        self.as_file.view.as_ref().map_or(0, |v| v.total as usize)
    }

    fn start_read(&mut self) {
        let sound = self.as_file.heard.clone();
        let n = sound.len();
        let fs = self.fs as u32;
        let cfg = self.stream_config();
        let job = std::thread::spawn(move || crate::stream::analyze_samples(&sound, fs, &cfg));
        self.as_file.job = Some((job, n));
    }

    /// How a saved take of this sound is read.
    fn stream_config(&self) -> crate::stream::StreamConfig {
        crate::stream::StreamConfig {
            analysis: self.cfg.analysis.clone(),
            ..Default::default()
        }
    }

    /// The read in flight, waited for, as the figures. True when it gave
    /// a read.
    fn take_read(&mut self) -> bool {
        let Some((job, _)) = self.as_file.job.take() else {
            return false;
        };
        match job.join() {
            Ok(log) => {
                self.set_view(log);
                true
            }
            Err(_) => false,
        }
    }

    /// `log` as the figures' analyzer, set as live is: its clock, masks,
    /// known flags, profile span and the caller's breaks.
    fn set_view(&mut self, log: BeatLog) {
        let mut v = LiveAnalyzer::from_log(log, self.cfg.clone());
        v.rate_clock = self.rate_clock.clone();
        v.clock_known = self.clock_known;
        v.masked = self.masked.clone();
        let caller: Vec<f64> = (self.breaks.iter().copied())
            .filter(|b| !self.intrinsic_breaks.contains(b))
            .collect();
        if !caller.is_empty() {
            v.set_breaks(caller);
        }
        self.as_file.view = Some(Box::new(v));
    }

    /// Reads everything heard as a file now, waiting for it, so the
    /// figures are the saved take's of all of it: for a stop, before the
    /// stopped figures show. Nothing once the figures are live's own.
    pub fn read_as_file_now(&mut self) {
        if !self.as_file.on {
            return;
        }
        if let Some((job, _)) = self.as_file.job.take() {
            let _ = job.join();
        }
        let log = crate::stream::analyze_samples(
            &self.as_file.heard,
            self.fs as u32,
            &self.stream_config(),
        );
        self.set_view(log);
    }
}

/// Live's listening state, `own`, with the read's figures, `read`: its
/// rate, beat error and amplitude with their ±, scatter, the beats and span
/// behind them and its time, and its caveats on those figures. The figures
/// that name a side wait, as live's do, until live tells the sides apart
/// (`sided`).
fn with_read_figures(own: LiveReading, read: LiveReading, sided: bool) -> LiveReading {
    let sided = |v: Option<f64>| v.filter(|_| sided);
    let mut caveats: Vec<Caveat> = (read.caveats.into_iter())
        .filter(|c| !listening_caveat(c))
        .collect();
    for c in own.caveats.into_iter().filter(listening_caveat) {
        measure::add(&mut caveats, c);
    }
    LiveReading {
        bph: own.bph,
        bph_set: own.bph_set,
        snr: own.snr,
        clipped_beats: own.clipped_beats,
        clipped_fraction: own.clipped_fraction,
        watch_heard: own.watch_heard,
        finding_beat: own.finding_beat,
        listening: own.listening,
        beat_error_ms: sided(read.beat_error_ms),
        beat_error_unlock_ms: sided(read.beat_error_unlock_ms),
        beat_error_unlock_error_ms: sided(read.beat_error_unlock_error_ms),
        amplitude_even_deg: sided(read.amplitude_even_deg),
        amplitude_odd_deg: sided(read.amplitude_odd_deg),
        caveats,
        ..read
    }
}

/// Live's listening state with no figures: before the first read.
fn without_figures(own: LiveReading) -> LiveReading {
    LiveReading {
        rate_s_per_day: None,
        beat_error_ms: None,
        beat_error_unlock_ms: None,
        beat_error_source: None,
        amplitude_deg: None,
        amplitude_error_deg: None,
        amplitude_no_pm: None,
        amplitude_even_deg: None,
        amplitude_odd_deg: None,
        jitter_us: None,
        timing_scatter_fraction: None,
        beats_used: 0,
        span_s: 0.0,
        whole: false,
        rate_error_s_per_day: None,
        rate_no_pm: None,
        rate_noise_values: 0,
        rate_own_values: 0,
        amplitude_values: 0,
        amplitude_noise_values: 0,
        beat_error_unlock_error_ms: None,
        caveats: own.caveats.into_iter().filter(listening_caveat).collect(),
        whole_take: false,
        ..own
    }
}

/// A caveat on what is heard, not on the figures: no watch, a set rate
/// the sound disagrees with, clipping.
fn listening_caveat(c: &Caveat) -> bool {
    matches!(
        c,
        Caveat::NoWatch { .. } | Caveat::SetRateDisagrees { .. } | Caveat::Clipped { .. }
    )
}
