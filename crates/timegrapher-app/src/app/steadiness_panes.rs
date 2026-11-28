//! The Steadiness and Isochronism panes: their settings, jobs and drawing.

use super::*;
use timegrapher_core::steadiness::IsochronismCurve;
use timegrapher_core::walk::IsochronismWalk;

impl TimegrapherApp {
    /// A Steadiness pane's Rate, Amplitude and Beat Error switches.
    /// The series a steadiness pane shows: its own switches, or for the
    /// two maps their one shared choice, one map at a time.
    pub(super) fn series_shown(&self, view: steady::View) -> [bool; 3] {
        if view.is_map() {
            steady::one_series(
                self.bench
                    .panes_view
                    .steady_series
                    .get(&steady::View::Stft)
                    .copied()
                    .unwrap_or(RATE_ONLY),
            )
        } else {
            self.bench
                .panes_view
                .steady_series
                .get(&view)
                .copied()
                .unwrap_or(RATE_ONLY)
        }
    }

    pub(super) fn steady_settings(&mut self, ui: &mut egui::Ui, pane: Pane) {
        let Some(view) = pane.steady_view() else {
            return;
        };
        if view.is_map() {
            steady::series_choice(
                ui,
                self.bench
                    .panes_view
                    .steady_series
                    .entry(steady::View::Stft)
                    .or_insert(RATE_ONLY),
            );
        } else {
            steady::series_switches(
                ui,
                self.bench
                    .panes_view
                    .steady_series
                    .entry(view)
                    .or_insert(RATE_ONLY),
            );
        }
        let accent = theme::pal(ui).accent;
        if view == steady::View::Periods {
            theme::bar_gap(ui);
            theme::tool_switch(
                ui,
                Icon::Db,
                accent,
                "dB Scale",
                &mut self.bench.panes_view.periods_db,
                "Show the scores in decibels. Off: linear.",
            );
        }
        // Timing changes only how rate is drawn, so it can be used only
        // while rate is.
        if view == steady::View::Fft {
            theme::bar_gap(ui);
            let rate_shown = self.series_shown(view)[0];
            theme::tool_switch_unless(
                ui,
                Icon::Stopwatch,
                accent,
                "Timing",
                &mut self.bench.panes_view.fft_timing,
                "Draw rate as the beat timing's spectrum, in ms, at every period, rather than \
                 the swing it makes in 10 s rate readings, in s/d",
                (!rate_shown).then_some("Turn Rate on to draw it as the beat timing's spectrum."),
            );
        }
        if matches!(view, steady::View::Stft | steady::View::Wavelet) {
            theme::bar_gap(ui);
            let mut db = self.bench.panes_view.maps_db.on(view);
            theme::tool_switch(
                ui,
                Icon::Db,
                accent,
                "dB Scale",
                &mut db,
                "Show the sizes in decibels. Off: linear.",
            );
            self.bench.panes_view.maps_db.set(view, db);
        }
        if matches!(
            view,
            steady::View::Periods | steady::View::Fft | steady::View::Stft | steady::View::Wavelet
        ) {
            let marks = &mut self.bench.panes_view.show_marks;
            let mut wheels = marks.wheels.on(view);
            theme::tool_switch(
                ui,
                Icon::Wheel,
                accent,
                "Wheel Periods",
                &mut wheels,
                timegrapher_core::steady_views::WHEELS_HINT,
            );
            marks.wheels.set(view, wheels);
            // The meshes, a switch of their own (#1764).
            let mut meshes = marks.meshes.on(view);
            theme::tool_switch(
                ui,
                Icon::Mesh,
                accent,
                "Mesh",
                &mut meshes,
                timegrapher_core::steady_views::MESH_HINT,
            );
            marks.meshes.set(view, meshes);
        }
    }

    /// What the panes shown this pass or the last want read (#1830).
    pub(super) fn steady_need(&self, pass: u64) -> steady::Need {
        let shown = |k: usize| self.bench.results.bg.shown_pass[k].is_some_and(|p| p + 1 >= pass);
        if shown(1) {
            steady::Need::Maps
        } else if shown(0) {
            steady::Need::Tests
        } else {
            steady::Need::Isochronism
        }
    }

    /// The seconds of beats heard the answers so far for `need` read:
    /// none when nothing read answers it.
    pub(super) fn steady_upto(&self, need: steady::Need) -> Option<f64> {
        let tests = self.bench.results.steady.as_ref().map(|s| &s.1);
        let tests = tests.filter(|s| need < steady::Need::Maps || s.maps.is_some());
        let alone = self
            .bench
            .results
            .bg
            .isochronism
            .as_ref()
            .map(|i| i.1.heard_s);
        let alone = alone.filter(|_| need == steady::Need::Isochronism);
        tests
            .map(|s| s.heard_s)
            .into_iter()
            .chain(alone)
            .reduce(f64::max)
    }

    /// The isochronism on show: the newer of the whole check's and the one
    /// read alone.
    pub(super) fn isochronism_shown(
        &self,
    ) -> Option<(Option<&IsochronismCurve>, Option<&IsochronismWalk>)> {
        let tests = self
            .bench
            .results
            .steady
            .as_ref()
            .map(|s| (&s.1.report, s.1.upto_s));
        let alone = self.bench.results.bg.isochronism.as_ref().map(|i| &i.1);
        if let Some(a) = alone.filter(|a| tests.is_none_or(|(_, u)| a.upto_s > u)) {
            return Some((a.isochronism.as_ref(), a.walk.as_ref()));
        }
        let (r, _) = tests?;
        Some((r.isochronism.as_ref(), r.walk.as_ref()))
    }

    /// Drop the steadiness answers and the run under way read before the
    /// take's settings, clock, breaks or masks last changed, or at another
    /// Average Over: what the tests leave out as unheard rests on it (Norm
    /// on #2451).
    fn drop_stale_steady(&mut self) {
        let rev = self.bench.take.rev;
        let average = self.bench.take.average_s;
        let results = &mut self.bench.results;
        if results.steady_average_s.is_some_and(|a| a != average) {
            results.steady_job = None;
            results.steady = None;
            results.bg.isochronism = None;
        }
        results.steady_average_s = Some(average);
        if results.steady_job.as_ref().is_some_and(|j| j.rev != rev) {
            results.steady_job = None;
        }
        if results.steady.as_ref().is_some_and(|s| s.0 != rev) {
            results.steady = None;
        }
        if results.bg.isochronism.as_ref().is_some_and(|s| s.0 != rev) {
            results.bg.isochronism = None;
        }
    }

    pub(super) fn update_steady(&mut self, ctx: &egui::Context) {
        self.drop_stale_steady();
        if let Some(results::SteadyJob { rev, rx }) = &self.bench.results.steady_job {
            match rx.try_recv() {
                Ok(steady::Answer::Tests(s)) => {
                    self.bench.results.steady = Some((*rev, *s));
                    self.bench.results.steady_job = None;
                }
                Ok(steady::Answer::Isochronism(i)) => {
                    self.bench.results.bg.isochronism = Some((*rev, i));
                    self.bench.results.steady_job = None;
                }
                Err(TryRecvError::Disconnected) => self.bench.results.steady_job = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        // Only the beats heard in the position the watch is in now,
        // joined: 5 minutes of them, not of sound, starts the tests, and
        // only more of them reads them again, so silence or the stand's
        // noise never does (#2451).
        let runs = self.heard_runs();
        let heard: f64 = runs.read.iter().map(|(a, b)| b - a).sum();
        let clock = Self::rate_clock(&self.bench.take.take_clock).cloned();
        let joined = !self.session.stretches().is_empty();
        let need = self.steady_need(ctx.cumulative_pass_nr());
        let due = match self.steady_upto(need) {
            None => true,
            Some(u) if self.running() => heard - u >= steady::RERUN_S,
            Some(u) => heard - u > 1.0,
        };
        // What is heard rests on readings at the Average Over and take
        // now: none starts on readings not yet read again (Vernon on #2477).
        let fresh = self.trend_current();
        if self.bench.results.steady_job.is_none() && fresh && due && heard >= steady::MIN_S {
            if let Some((log, breaks)) = self.steady_read(&runs, clock.as_ref()) {
                let wheels = self.calibre_wheels();
                let meshes = self.calibre_meshes();
                let clock = clock.filter(|_| !joined); // A joined log is on it.
                let (tx, rx) = std::sync::mpsc::channel();
                let ctx = ctx.clone();
                let stage = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
                self.bench.results.steady_work = (stage.clone(), std::time::Instant::now());
                std::thread::spawn(move || {
                    let mut a =
                        steady::compute_for(need, &log, &breaks, wheels, clock.as_ref(), &stage);
                    match &mut a {
                        steady::Answer::Tests(s) => {
                            s.meshes = meshes;
                            s.heard_s = heard;
                        }
                        steady::Answer::Isochronism(i) => i.heard_s = heard,
                    }
                    let _ = tx.send(a);
                    ctx.request_repaint();
                });
                self.bench.results.steady_job = Some(results::SteadyJob {
                    rev: self.bench.take.rev,
                    rx,
                });
            }
        }
    }

    /// The runs in the position the watch is in now as the steadiness
    /// tests read them: only where a watch was heard
    /// ([`timegrapher_core::live::heard_spans`]), each unheard stretch a
    /// cut, as a hole in a run is (Norm on #2451). Their length is what
    /// "5 minutes of beats" and "Tested over" count, with the wait.
    pub(super) fn heard_runs(&self) -> timegrapher_core::take_context::Stretches<Position> {
        let mut runs = self.current_runs();
        let end = self
            .bench
            .recording
            .live
            .as_ref()
            .map_or(0.0, |l| l.duration_s());
        runs.read = timegrapher_core::live::heard_spans(
            &self.bench.results.trend,
            self.bench.take.average_s,
            &runs.read,
            end,
        );
        runs.at = vec![self.bench.take.position; runs.read.len()];
        runs
    }

    /// What the steadiness tests read of the session, and the cuts they
    /// stop at ([`Self::steady_log`] of the position's heard `runs`): each
    /// stretch no reading hears a watch in left out and cut at both ends,
    /// as a mask is, so nothing bridges it, on the take's own time or
    /// joined (Norm on #2451).
    pub(super) fn steady_read(
        &self,
        runs: &timegrapher_core::take_context::Stretches<Position>,
        clock: Option<&timegrapher_core::clock::ClockFit>,
    ) -> Option<(BeatLog, Vec<timegrapher_core::breaks::Break>)> {
        let live = self.bench.recording.live.as_ref()?;
        let unheard = timegrapher_core::live::unheard_spans(
            &self.bench.results.trend,
            self.bench.take.average_s,
            live.duration_s(),
        );
        let masked = timegrapher_core::session::with_holes(live.masked(), &unheard);
        let log = steady::log_masked(live, &masked)?;
        Some(self.steady_log(log, runs, clock))
    }

    /// What the steadiness tests read of `log`, and the cuts they stop at:
    /// a take with no position marked whole, at its own breaks with the time
    /// each gap held and every other jump, as `timegrapher series` reads it;
    /// else the position's `runs` joined end to end on the take's `clock`
    /// ([`steady::join`]), each hole in a run left out and cut at both ends.
    pub(super) fn steady_log(
        &self,
        mut log: BeatLog,
        runs: &timegrapher_core::take_context::Stretches<Position>,
        clock: Option<&timegrapher_core::clock::ClockFit>,
    ) -> (BeatLog, Vec<timegrapher_core::breaks::Break>) {
        let mut opened = self.bench.take.take_breaks.clone().unwrap_or_default();
        if self.session.stretches().is_empty() {
            let breaks = steady::reading_breaks(opened, &log);
            (log, breaks)
        } else {
            // A hole in a run is left out and cut at both ends, as `series`
            // reads a take's holes (#1156). The masks by hand are already
            // out of `log`, cut among its jumps (`steady::log_of`).
            let mut no_masks = Vec::new();
            timegrapher_core::session::leave_out(&mut log, &mut opened, &mut no_masks, &runs.holes);
            steady::join(&log, &runs.read, &opened, clock)
        }
    }

    /// While the steadiness tests run: a spinner, the step they're on and
    /// how long they've taken, for the Steadiness and Isochronism panes.
    /// Nothing when they aren't running.
    fn steady_working(&self, ui: &mut egui::Ui) {
        if self.bench.results.steady_job.is_none() {
            return;
        }
        let pal = theme::pal(ui);
        let (stage, since) = &self.bench.results.steady_work;
        let stage = stage.load(std::sync::atomic::Ordering::Relaxed);
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().size(12.0).color(pal.text_secondary));
            ui.label(
                RichText::new(format!(
                    "{}… {}",
                    steady::stage_text(stage),
                    length_text(since.elapsed().as_secs_f64())
                ))
                .small()
                .color(pal.text_secondary),
            );
        })
        .response
        .on_hover_text(
            "The tests run in the background over every reading of the session, so a \
             session of many hours takes a while; what is shown meanwhile is the last \
             answer, if there is one.",
        );
    }

    pub(super) fn isochronism_pane(&mut self, ui: &mut egui::Ui) {
        self.update_steady(ui.ctx());
        let pal = theme::pal(ui);
        self.steady_working(ui);
        match self.isochronism_shown() {
            Some((curve, walk)) => {
                let walk = walk.and_then(timegrapher_core::steady_views::walk_view);
                isochronism::ui(ui, curve, walk.as_ref(), self.bench.panes_view.iso_view);
            }
            None if self.bench.results.steady_job.is_some() => {}
            None => {
                ui.label(
                    RichText::new(self.steady_wait())
                        .small()
                        .color(pal.text_secondary),
                );
            }
        }
    }

    /// How long a watch was heard in the position the watch is in now,
    /// seconds: what "5 minutes of beats" counts, for the tests' start
    /// and their wait alike (Vernon on #2449): the length of
    /// [`Self::heard_runs`].
    pub(super) fn heard_in_position(&self) -> f64 {
        self.heard_runs().read.iter().map(|(a, b)| b - a).sum()
    }

    /// What the steadiness tests wait for, in the terms of what is being
    /// done: a Positions Test never stays long enough in one position.
    /// "So far" is the beats heard in the position, never the silence or
    /// the stand's noise around them (#2183).
    pub(super) fn steady_wait(&self) -> String {
        if self.bench.test.guided.is_some() {
            return "Needs 5 minutes of beats in one position, longer than a Positions Test \
                    stays in each. Record one position for 5 minutes or more to see it."
                .to_string();
        }
        let heard = self.heard_in_position();
        // The cards read a watch the readings at this Average Over don't
        // cover whole yet (Walter on #2449): no count, rather than "none".
        // Nor while the readings are still being read again after a
        // change: their count is at the old footing (Vernon on #2477).
        if !self.trend_current() || (heard <= 0.0 && self.cards_unheard().is_none()) {
            return "Needs 5 minutes of beats in one position.".to_string();
        }
        if heard > 0.0 {
            format!(
                "Needs 5 minutes of beats in one position; {} heard so far.",
                length_text(heard)
            )
        } else {
            "Needs 5 minutes of beats in one position; none heard so far.".to_string()
        }
    }

    pub(super) fn steady_pane(&mut self, ui: &mut egui::Ui, view: steady::View) {
        let pass = Some(ui.ctx().cumulative_pass_nr());
        self.bench.results.bg.shown_pass[0] = pass;
        if matches!(view, steady::View::Stft | steady::View::Wavelet) {
            self.bench.results.bg.shown_pass[1] = pass;
        }
        self.update_steady(ui.ctx());
        self.steady_working(ui);
        let status = self.steady_status();
        steady::draw(
            ui,
            self.bench.results.steady.as_ref().map(|s| &s.1),
            &status,
            view,
            &self.series_shown(view),
            self.steady_opts(view),
        );
    }

    /// A Steadiness pane's status line: what its answer was tested over,
    /// in beats heard, not sound (#2451); nothing while the first run is
    /// under way; else what the tests wait for.
    pub(super) fn steady_status(&self) -> String {
        let of = if self.session.stretches().is_empty() {
            String::new()
        } else {
            format!("{} ", self.bench.take.position.name())
        };
        match self.bench.results.steady.as_ref().map(|s| s.1.heard_s) {
            Some(u) => format!("Tested over {} of {of}beats", length_text(u)),
            None if self.bench.results.steady_job.is_some() => String::new(),
            None => self.steady_wait(),
        }
    }

    /// How a steadiness pane draws `view`. On the take's own clock its
    /// time is the take's span, the strip's; a position's stretches joined
    /// end to end are on a clock of their own, named on the axis (#1256).
    pub(super) fn steady_opts(&self, view: steady::View) -> steady::Opts {
        let joined = !self.session.stretches().is_empty();
        steady::Opts {
            periods_db: self.bench.panes_view.periods_db,
            fft_timing: self.bench.panes_view.fft_timing,
            maps_db: self.bench.panes_view.maps_db.on(view),
            wheels: self.bench.panes_view.show_marks.wheels.on(view),
            meshes: self.bench.panes_view.show_marks.meshes.on(view),
            info: self.bench.panes_view.steady_info.on(view),
            take_span: self.take_span().filter(|_| !joined),
            joined_in: joined.then(|| self.bench.take.position.code()),
        }
    }
}
