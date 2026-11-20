//! Auto's rate at a pause is the watch's (#2112, #2114). A watch lifted
//! off the stand leaves a quiet stand: after 5 s of it Auto lets its
//! rate go ([`LiveAnalyzer::quiet_pass`]), and the stand's own noise (a USB hum, say) can be
//! locked on to at a rate of its own. Every reading of the take is then
//! read at that rate, or at none, though no watch beat at it. So at a
//! pause the rate, and the template it was heard with, go back to the
//! lock that heard a watch longest ([`beats::watch_heard_s`]; Norm on
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

    /// At a pause, with Auto: the watch is the lock whose beats were
    /// heard as a watch the longest ([`beats::watch_heard_s`]); a later
    /// lock takes its place only by being heard longer. When that is not
    /// the lock now (or none is held now), it is put back, and the later
    /// ones are let go with what they read.
    pub(super) fn back_to_watchs_lock(&mut self) {
        if self.cfg.analysis.bph.is_some() {
            return;
        }
        let Some(i) = self.watchs_lock() else {
            return;
        };
        if i + 1 == self.locks.0.len() && self.bph.is_some() {
            return;
        }
        let culled = self.locks.0.get(i + 1).map(|n| n.from_s);
        if let Some(from_s) = culled {
            self.let_go_from(from_s);
        }
        self.locks.0.truncate(i + 1);
        let (bph, template) = (self.locks.0[i].bph, self.locks.0[i].template.clone());
        self.forget_rebuild();
        self.template = Some(template);
        self.bph = Some(bph);
        self.found_bph = true;
        self.logged_bph = self.bph;
        self.doubt = None;
        // The Tick and Tock profiles: the watch's again, from its beats.
        if let (Some(_), Some(last)) = (culled, self.beats.last()) {
            self.update_profiles(last.time, 3600.0 / f64::from(bph));
        }
    }

    /// What the locks let go read from `from_s` on, let go with them:
    /// their beats were numbered at a rate that isn't the watch's, and
    /// their amplitude windows were cut on a template that isn't its.
    /// Nothing is put in their place; that stretch reads as no watch.
    fn let_go_from(&mut self, from_s: f64) {
        let keep = self.beats.partition_point(|b| b.time < from_s);
        if keep < self.beats.len() {
            self.beats.truncate(keep);
            self.beats_gen = fresh_gen();
        }
        self.clipped.retain(|&t| t < from_s);
        self.amp.retain(|w| w.start_s < from_s);
        self.sample.retain(|x| x.0 < from_s);
        self.snippets.retain(|x| x.0 < from_s);
    }

    /// The lock heard as a watch the longest, the earliest on a tie; none
    /// when no lock heard a run as long as a piece.
    fn watchs_lock(&self) -> Option<usize> {
        let locks = &self.locks.0;
        let mut best: Option<(usize, f64)> = None;
        for (i, l) in locks.iter().enumerate() {
            let to = locks.get(i + 1).map_or(f64::INFINITY, |n| n.from_s);
            let lo = self.beats.partition_point(|b| b.time < l.from_s);
            let hi = self.beats.partition_point(|b| b.time < to);
            let heard = beats::watch_heard_s(&self.beats[lo..hi]);
            if heard > 0.0 && best.is_none_or(|(_, h)| heard > h) {
                best = Some((i, heard));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Auto's locks forgotten, with the beats.
    pub(super) fn forget_locks(&mut self) {
        self.locks = Locks::default();
    }
}
