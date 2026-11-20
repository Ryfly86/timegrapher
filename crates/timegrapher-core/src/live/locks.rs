//! Auto's rate at a pause is the watch's (#2112, #2114). A watch lifted
//! off the stand leaves a quiet stand: after 5 s of it Auto lets its
//! rate go ([`LiveAnalyzer::quiet_pass`]), and the stand's own noise (a USB hum, say) can be
//! locked on to at a rate of its own. Every reading of the take is then
//! read at that rate, or at none, though no watch beat at it. So at a
//! pause the rate, and the template it was heard with, go back to the
//! lock that read the watch's last run of beats
//! ([`beats::last_watch_beat_s`]), the run the stopped cards read to.
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

    /// At a pause, with Auto: when the watch's last run was read at an
    /// earlier lock than the one now (or none is held now), that lock is
    /// put back, and the later ones, which read no watch, are let go.
    pub(super) fn back_to_watchs_lock(&mut self) {
        if self.cfg.analysis.bph.is_some() {
            return;
        }
        let Some(t) = beats::last_watch_beat_s(&self.beats) else {
            return;
        };
        let Some(i) = self.locks.0.iter().rposition(|l| l.from_s <= t) else {
            return;
        };
        if i + 1 == self.locks.0.len() && self.bph.is_some() {
            return;
        }
        self.locks.0.truncate(i + 1);
        let (bph, template) = (self.locks.0[i].bph, self.locks.0[i].template.clone());
        self.forget_rebuild();
        self.template = Some(template);
        self.bph = Some(bph);
        self.found_bph = true;
        self.logged_bph = self.bph;
        self.doubt = None;
    }

    /// Auto's locks forgotten, with the beats.
    pub(super) fn forget_locks(&mut self) {
        self.locks = Locks::default();
    }
}
