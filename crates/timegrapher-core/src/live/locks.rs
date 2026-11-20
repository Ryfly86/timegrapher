//! Auto's rate at a pause is the watch's (#2112, #2114). A watch lifted
//! off the stand leaves a quiet stand: after 5 s of it Auto lets its
//! rate go ([`LiveAnalyzer::quiet_pass`]), and the stand's own noise (a USB hum, say) can be
//! locked on to at a rate of its own. Every reading of the take is then
//! read at that rate, or at none, though no watch beat at it. So at a
//! pause the rate, and the template it was heard with, go back to the
//! rate heard as a watch longest ([`beats::watch_heard_s`]; Norm on
//! #2142), so noise heard after the watch takes its place only by being
//! heard as a watch for longer.
//! Listening again goes on from there, as from any lock.

use super::*;

/// One lock Auto found: from when its beats count, its rate, and the
/// template it heard them with.
#[derive(Debug, Clone)]
struct Lock {
    from_s: f64,
    bph: u32,
    template: Vec<f32>,
}

/// Auto's locks so far, oldest first.
#[derive(Debug, Clone, Default)]
pub(super) struct Locks(Vec<Lock>);

impl LiveAnalyzer {
    /// Auto has just locked on: its beats from `from_s` on are read at
    /// this rate, with this template.
    pub(super) fn note_lock(&mut self, from_s: f64) {
        if let (Some(bph), Some(template)) = (self.bph, &self.template) {
            self.locks.0.push(Lock {
                from_s,
                bph,
                template: template.clone(),
            });
        }
    }

    /// The template of the latest lock built again from the first chunk
    /// (#1327): the lock goes on with it.
    pub(super) fn relock_template(&mut self, template: &[f32]) {
        if let Some(l) = self.locks.0.last_mut() {
            l.template = template.to_vec();
        }
    }

    /// At a pause, with Auto: the watch is the rate heard as a watch the
    /// longest ([`beats::watch_heard_s`], summed over every lock at that
    /// rate, since a watch put back after a quiet is locked on to again
    /// at its own rate); a later rate takes its place only by being heard
    /// longer. Length is all that tells a watch from noise heard as one,
    /// so a second watch heard for less than the first (120 s, then 40 s)
    /// is read as the first, and its beats are let go. The locks at other
    /// rates after the rate's first are let go with what they read, and
    /// when the lock held now is not at that rate (or none is held), the
    /// rate's latest lock is put back.
    pub(super) fn back_to_watchs_lock(&mut self) {
        if self.cfg.analysis.bph.is_some() {
            return;
        }
        let Some(bph) = self.watchs_rate() else {
            return;
        };
        // Whether the lock held now, the latest, is at the watch's rate.
        let held = self.bph == Some(bph) && self.locks.0.last().is_some_and(|l| l.bph == bph);
        // From the rate's first lock on: what came before it stands.
        let first = self
            .locks
            .0
            .iter()
            .find(|l| l.bph == bph)
            .map_or(0.0, |l| l.from_s);
        let others = self.spans_where(|l| l.bph != bph && l.from_s >= first);
        self.let_go_within(&others);
        self.locks.0.retain(|l| l.bph == bph || l.from_s < first);
        // The lock held is the watch's already (a watch put back after a
        // lure): it stays as it is, with only the others let go.
        if held {
            return;
        }
        let Some(template) = self.locks.0.last().map(|l| l.template.clone()) else {
            return;
        };
        self.forget_rebuild();
        self.template = Some(template);
        self.bph = Some(bph);
        self.found_bph = true;
        self.logged_bph = self.bph;
        self.doubt = None;
        // The Tick and Tock profiles: the watch's again, from its beats.
        if let (false, Some(last)) = (others.is_empty(), self.beats.last()) {
            self.update_profiles(last.time, 3600.0 / f64::from(bph));
        }
    }

    /// The stretches, start and end in seconds, that the locks `which`
    /// picks read: each from its start to the next lock's.
    fn spans_where(&self, which: impl Fn(&Lock) -> bool) -> Vec<(f64, f64)> {
        let locks = &self.locks.0;
        (locks.iter().enumerate())
            .filter(|(_, l)| which(l))
            .map(|(i, l)| {
                (
                    l.from_s,
                    locks.get(i + 1).map_or(f64::INFINITY, |n| n.from_s),
                )
            })
            .collect()
    }

    /// What the locks let go read in `spans`, let go with them: their
    /// beats were numbered at a rate that isn't the watch's, and their
    /// amplitude windows cut on a template that isn't its. Nothing is put
    /// in their place; those stretches read as no watch. Only the
    /// analysis: the sound kept is untouched.
    fn let_go_within(&mut self, spans: &[(f64, f64)]) {
        let out = |t: f64| spans.iter().any(|&(a, b)| a <= t && t < b);
        let n = self.beats.len();
        self.beats.retain(|b| !out(b.time));
        if self.beats.len() < n {
            self.beats_gen = fresh_gen();
        }
        self.clipped.retain(|&t| !out(t));
        self.amp.retain(|w| !out(w.start_s));
        self.sample.retain(|x| !out(x.0));
        self.snippets.retain(|x| !out(x.0));
    }

    /// The rate heard as a watch the longest over all its locks, the
    /// first heard on a tie; none when no lock heard a run as long as a
    /// piece.
    fn watchs_rate(&self) -> Option<u32> {
        let mut heard: Vec<(u32, f64)> = Vec::new();
        for (l, (from, to)) in self.locks.0.iter().zip(self.spans_where(|_| true)) {
            let lo = self.beats.partition_point(|b| b.time < from);
            let hi = self.beats.partition_point(|b| b.time < to);
            let s = beats::watch_heard_s(&self.beats[lo..hi]);
            match heard.iter_mut().find(|h| h.0 == l.bph) {
                Some(h) => h.1 += s,
                None => heard.push((l.bph, s)),
            }
        }
        let mut best: Option<(u32, f64)> = None;
        for (bph, s) in heard {
            if s > 0.0 && best.is_none_or(|(_, b)| s > b) {
                best = Some((bph, s));
            }
        }
        best.map(|(bph, _)| bph)
    }

    /// Auto's locks forgotten, with the beats.
    pub(super) fn forget_locks(&mut self) {
        self.locks = Locks::default();
    }
}
