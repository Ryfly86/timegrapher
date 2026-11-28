//! A guided test read as it is heard: each position once its beats are
//! final, and read again once live's template built again times them
//! anew, so its table reads as the test's end reads it (#2510).

use super::Test;
use timegrapher_core::clock::ClockFit;
use timegrapher_core::stream::BeatLog;

impl Test {
    /// Read each position measured since the last call, so the pane's
    /// table fills in as the test goes: one whose beats are final to its
    /// end, `settled_s` (core's `LiveAnalyzer::settled_s`), so it reads as
    /// it does once the test ends (#2510).
    pub fn read_new(
        &mut self,
        log: &BeatLog,
        clock: Option<&ClockFit>,
        masked: &[(f64, f64)],
        settled_s: f64,
    ) {
        while self.done.len() < self.spans.len() {
            let i = self.done.len();
            let (_, from, to) = self.spans[i];
            if to > settled_s {
                break;
            }
            match self.reading(i, from, to, log, clock, masked) {
                Some(r) => self.done.push(r),
                None => break,
            }
        }
    }

    /// Whether the positions read so far were read before live's template
    /// built again took over at `switch`, and are to be read again
    /// ([`Self::read_on`]).
    pub fn read_after(&self, switch: Option<f64>) -> bool {
        self.readings.is_none() && switch != self.read_after_switch
    }

    /// [`Self::read_new`] as the test goes, on live's beats with its
    /// template built again at `switch` (core's
    /// `LiveAnalyzer::template_switch_s`): once it has taken over since
    /// the positions so far were read, they are read again on the beats it
    /// timed, as the test's end reads them (#2510).
    pub fn read_on(
        &mut self,
        log: &BeatLog,
        clock: Option<&ClockFit>,
        masked: &[(f64, f64)],
        settled_s: f64,
        switch: Option<f64>,
    ) {
        if self.readings.is_some() {
            return;
        }
        if switch != self.read_after_switch {
            self.read_after_switch = switch;
            self.done.clear();
        }
        self.read_new(log, clock, masked, settled_s);
    }
}
