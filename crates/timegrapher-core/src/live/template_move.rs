//! Live builds its template again where the kept one has moved to another
//! sound on a side, as a saved read does (#1819,
//! [`crate::template_move`]): each chunk of the saved read's grid is cut
//! from the sound as it closes and judged on its own template; when two in
//! a row agree, a copy of the analyzer as it was at the first's start
//! reads the sound since again with that chunk's template, a little faster
//! than it comes in, and takes over once caught up. So live gives the
//! saved read's beats, up to [`MOVE_CHUNKS`] chunks late.

use super::rebuild::{Shadow, CATCH_UP};
use super::*;
use crate::stream::{StreamConfig, CHUNK_S};
use crate::template_move::{self, Move, MOVE_CHUNKS};

/// Where live is in watching its template for a move.
#[derive(Debug, Clone, Default)]
pub(super) struct Watch {
    /// The sound kept since the sample `.0`: the chunks are cut from it,
    /// and a held chunk is read again from it.
    audio: Option<(u64, Vec<f32>)>,
    /// The next chunk of the grid to cut, counted from the take's start.
    next: usize,
    /// The next chunk whose start the analyzer is to be copied at.
    next_copy: usize,
    /// The analyzer as it was before its first pass to reach a chunk's
    /// start, each with that chunk's number: the held chunk's, and the
    /// open one's.
    copies: Vec<(usize, Box<LiveAnalyzer>)>,
    /// A chunk whose kept template moved on a side, held until the next
    /// is judged: its number, its own template and its moves.
    held: Option<(usize, Vec<f32>, Vec<Move>)>,
    /// The copy reading the sound again from a held chunk's start.
    shadow: Option<Box<Shadow>>,
    /// In that copy: the time the held chunk starts and its template,
    /// which takes over there.
    switch: Option<(f64, Vec<f32>)>,
    /// In that copy: the next beat found is a break.
    jump_next: bool,
    /// The times the template was built again, seconds.
    rebuilt: Vec<f64>,
}

impl Watch {
    /// The copies kept of this analyzer, the one reading again among them,
    /// for a change the caller makes to reach them too.
    pub(super) fn copies(&mut self) -> impl Iterator<Item = &mut LiveAnalyzer> {
        let shadow = self.shadow.as_deref_mut().map(|s| &mut s.live);
        (self.copies.iter_mut().map(|c| &mut *c.1)).chain(shadow)
    }

    /// The time the template takes over in a copy reading again, if it is
    /// still to: no pass settles beats past it on the template before.
    pub(super) fn switch_s(&self) -> Option<f64> {
        self.switch.as_ref().map(|s| s.0)
    }

    /// Whether the next beat found is a break, once: asked as it is found.
    pub(super) fn take_jump(&mut self) -> bool {
        std::mem::take(&mut self.jump_next)
    }

    /// Forget the chunks watched, and the copies: the sound since is
    /// another stretch, or the template another's. A copy reading again
    /// carries on.
    pub(super) fn forget(&mut self) {
        self.audio = None;
        self.copies.clear();
        self.held = None;
    }
}

/// Samples in a chunk's core and margin, as a saved read cuts them.
fn grid(fs: f64) -> (u64, u64) {
    let s = StreamConfig::default();
    (
        (s.chunk_s * fs).round() as u64,
        (s.margin_s * fs).round() as u64,
    )
}

impl LiveAnalyzer {
    /// The times the template was built again because the kept one had
    /// moved to another sound on a side, seconds (#1819).
    pub fn template_moves(&self) -> &[f64] {
        &self.moves.rebuilt
    }

    /// Whether the template is the one the chunks are judged against:
    /// heard, built from the first chunk, and not about to change.
    fn watching(&self) -> bool {
        self.template.is_some()
            && !self.gathering()
            && self.moves.switch.is_none()
            && self.moves.shadow.is_none()
    }

    /// After a pass, a copy reading again takes on the held chunk's
    /// template once its beats are settled to the chunk's start.
    pub(super) fn switch_template(&mut self) {
        if let Some((at, t)) = self.moves.switch.take() {
            if self.settled_to >= at - 1e-9 {
                self.template = Some(t);
                self.moves.jump_next = true;
                self.moves.rebuilt.push(at);
            } else {
                self.moves.switch = Some((at, t));
            }
        }
    }

    /// After a push of `samples`: reads the sound again a little further
    /// while a copy does, or else keeps the sound, copies the analyzer at
    /// each chunk's start and judges each chunk that has closed. True when
    /// the readings may have changed.
    pub(super) fn moves_after_push(&mut self, samples: &[f32]) -> bool {
        if let Some(sh) = &mut self.moves.shadow {
            sh.pending.extend_from_slice(samples);
            let step = (self.cfg.update_s * self.fs).round().max(1.0) as usize;
            if sh.read(CATCH_UP * samples.len(), step) {
                self.take_over();
            }
            return true;
        }
        if !self.watching() {
            self.moves.forget();
            return false;
        }
        self.keep_sound(samples);
        self.copy_at_chunk_start();
        let (core, margin) = grid(self.fs);
        let mut changed = false;
        while self.total >= (self.moves.next as u64 + 1) * core + margin {
            changed |= self.judge_chunk(false);
        }
        changed
    }

    /// Keeps `samples`, just pushed, from the first push watched on.
    fn keep_sound(&mut self, samples: &[f32]) {
        let (core, margin) = grid(self.fs);
        match &mut self.moves.audio {
            // A copy that took over in this push has kept its sound.
            Some((a0, a)) => {
                let new = (self.total - *a0 - a.len() as u64) as usize;
                a.extend_from_slice(&samples[samples.len() - new..]);
            }
            None => {
                let a0 = self.total - samples.len() as u64;
                // The first chunk all of whose sound is kept.
                let k = if a0 == 0 {
                    0
                } else {
                    (a0 + margin).div_ceil(core)
                };
                self.moves.audio = Some((a0, samples.to_vec()));
                self.moves.next = k as usize;
                self.moves.next_copy = self.moves.next_copy.max(k as usize);
            }
        }
    }

    /// A copy of the analyzer before its first pass to reach the next
    /// chunk's start, that the chunk can be read again from.
    fn copy_at_chunk_start(&mut self) {
        let (core, _) = grid(self.fs);
        while self.moves.next_copy as u64 * core <= self.total {
            self.moves.next_copy += 1;
        }
        let k = self.moves.next_copy;
        if self.next_pass < k as u64 * core {
            return;
        }
        let watch = std::mem::take(&mut self.moves);
        let copy = Box::new(self.clone());
        self.moves = watch;
        self.moves.copies.push((k, copy));
        self.moves.next_copy += 1;
    }

    /// Judges the next chunk of the grid, or with `last` the sound left of
    /// it at a stop: holds it when its kept template moved on a side, and
    /// reads again from a held chunk the chunk after agrees with. True when
    /// the reading again started.
    fn judge_chunk(&mut self, last: bool) -> bool {
        let fs = self.fs;
        let (core, margin) = grid(fs);
        let k = self.moves.next;
        self.moves.next += 1;
        let (Some(t), Some(bph), Some((a0, a))) = (&self.template, self.bph, &self.moves.audio)
        else {
            return false;
        };
        let x0 = (k as u64 * core).saturating_sub(margin);
        let x1 = if last {
            self.total
        } else {
            (k as u64 + 1) * core + margin
        };
        let x = &a[(x0 - a0) as usize..(x1 - a0) as usize];
        let an = &self.cfg.analysis;
        let env = envelope(x, fs, &an.envelope);
        let off = x0 as f64 / fs;
        let core_a = (k as u64 * core) as f64 / fs;
        let core_b = if last {
            f64::INFINITY
        } else {
            ((k as u64 + 1) * core) as f64 / fs
        };
        let own = template_move::own_beats(&env, fs, an.bph, an.amplitude_window_s);
        let judged = own.as_ref().and_then(|o| {
            let m = template_move::chunk_moves(
                &env,
                fs,
                (bph, t),
                o,
                (core_a - off, core_b - off),
                off,
            )?;
            Some((m, o.template.clone()?))
        });
        let (moves, own_t) = judged.unwrap_or_default();
        let beat = 3600.0 / bph as f64;
        let agrees =
            |h: &(usize, Vec<f32>, Vec<Move>)| template_move::any_agree(&h.2, &moves, beat);
        if self.moves.held.as_ref().is_some_and(agrees) {
            if let Some((p, t, _)) = self.moves.held.take() {
                if self.read_again_from(p, t) {
                    return true;
                }
            }
        }
        if last {
            // Listening on, the chunk is judged again once it closes.
            self.moves.next = k;
            return false;
        }
        self.moves.held = None;
        if !moves.is_empty() && self.moves.copies.iter().any(|c| c.0 == k) {
            self.moves.held = Some((k, own_t, moves));
        }
        self.let_go_before(core, margin);
        false
    }

    /// Lets go of the sound and copies no chunk held or still to come
    /// needs.
    fn let_go_before(&mut self, core: u64, margin: u64) {
        let first = self.moves.held.as_ref().map_or(self.moves.next, |h| h.0);
        self.moves.copies.retain(|c| c.0 >= first);
        if let Some((a0, a)) = &mut self.moves.audio {
            let keep = (first as u64 * core).saturating_sub(margin).max(*a0);
            a.drain(..(keep - *a0) as usize);
            *a0 = keep;
        }
    }

    /// Starts reading the sound again from chunk `k`'s start with its own
    /// template `t`: the copy made there, which keeps its beats before the
    /// chunk, takes `t` on at its start, with a break, and reads the sound
    /// since through the same passes. False when no copy is kept.
    fn read_again_from(&mut self, k: usize, t: Vec<f32>) -> bool {
        let Some(i) = self.moves.copies.iter().position(|c| c.0 == k) else {
            return false;
        };
        let Some(from) = (self.moves.audio.as_ref())
            .and_then(|(a0, _)| self.moves.copies[i].1.total.checked_sub(*a0))
        else {
            return false;
        };
        let mut l = *self.moves.copies.swap_remove(i).1;
        let Some((_, a)) = self.moves.audio.take() else {
            return false;
        };
        l.moves.switch = Some((k as f64 * CHUNK_S, t));
        l.moves.rebuilt = self.moves.rebuilt.clone();
        let pending = a[from as usize..].to_vec();
        self.moves.forget();
        self.moves.shadow = Some(Box::new(Shadow {
            live: l,
            pending,
            at: 0,
        }));
        true
    }

    /// The copy reading again has caught up: its reading takes over.
    fn take_over(&mut self) {
        if let Some(sh) = self.moves.shadow.take() {
            *self = sh.live;
        }
    }

    /// A copy reading again reads the rest at once and takes over: before
    /// a stop, or a change of the caller's (a new stretch, a pause, its
    /// own breaks) that the copy, behind in time, could not take.
    pub(super) fn read_again_now(&mut self) {
        let step = (self.cfg.update_s * self.fs).round().max(1.0) as usize;
        if let Some(sh) = &mut self.moves.shadow {
            sh.read(usize::MAX / 2, step);
            self.take_over();
        }
    }

    /// At a stop, as a saved read ends: a copy reading again reads the
    /// rest at once and takes over, and the sound left after a held chunk
    /// is judged as the last chunk, reading again from the held one when
    /// it agrees. Called by [`Self::settle`].
    pub(super) fn finish_moves(&mut self) {
        loop {
            if self.moves.shadow.is_some() {
                self.read_again_now();
                continue;
            }
            let (core, _) = grid(self.fs);
            let open = self.total > self.moves.next as u64 * core;
            if !(self.watching() && self.moves.held.is_some() && open && self.judge_chunk(true)) {
                return;
            }
        }
    }
}

const _: () = assert!(MOVE_CHUNKS == 2, "live holds one chunk, then judges");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{analyze_samples, StreamConfig};
    use crate::synth::{generate, SynthConfig};

    /// 300 s at 300°, then 300 s at 150° whose Tock grows a loud unlock
    /// and impulse (as a run-down, #1819); or 600 s steady.
    fn take(moves: bool) -> (Vec<f32>, u32) {
        let seg = |seed: u64, odd_gains: Option<[f64; 3]>| {
            generate(
                &SynthConfig {
                    duration_s: 300.0,
                    seed,
                    odd_gains,
                    ..Default::default()
                },
                move |_| if odd_gains.is_some() { 150.0 } else { 300.0 },
                |_| 0.3,
            )
        };
        let a = seg(1819, None);
        let b = if moves {
            seg(1820, Some([0.5, 1.0, 0.8]))
        } else {
            seg(1820, None)
        };
        let mut x = a.samples;
        x.extend_from_slice(&b.samples);
        (x, a.sample_rate)
    }

    fn live(x: &[f32], fs: u32) -> LiveAnalyzer {
        let mut cfg = LiveConfig::default();
        cfg.analysis.bph = Some(28800);
        cfg.max_beats = usize::MAX;
        let mut l = LiveAnalyzer::new(fs, cfg);
        for c in x.chunks(4_800) {
            l.push(c);
        }
        l.settle();
        l
    }

    /// Live builds the template again where a saved read does, at the
    /// start of the chunk the Tock's sound moved in, with the same break,
    /// and reads the beats after on it as the saved read does: 99% within
    /// a microsecond, where the kept template put the Tock 7.6 ms early.
    /// On a steady take it builds nothing again.
    #[test]
    fn live_builds_the_template_again_where_a_saved_read_does() {
        let (x, fs) = take(true);
        let mut cfg = StreamConfig::default();
        cfg.analysis.bph = Some(28800);
        let saved = analyze_samples(&x, fs, &cfg);
        let l = live(&x, fs);
        assert_eq!(l.template_moves(), &[282.0]);
        let near = |t: f64| {
            let i = l.beats().partition_point(|b| b.time < t);
            [i.checked_sub(1), Some(i)]
                .into_iter()
                .flatten()
                .filter_map(|j| l.beats().get(j))
                .map(|b| (b.time - t).abs())
                .fold(f64::INFINITY, f64::min)
        };
        let after: Vec<f64> = (saved.beats.iter())
            .filter(|b| (290.0..590.0).contains(&b.time))
            .map(|b| near(b.time))
            .collect();
        let close = after.iter().filter(|d| **d < 1e-6).count();
        assert!(
            close * 100 >= after.len() * 99,
            "{close} of {}",
            after.len()
        );
        assert_eq!(l.jumps(), &saved.jumps[..]);
        let (y, _) = take(false);
        assert!(live(&y, fs).template_moves().is_empty());
    }

    /// Stopped in the chunk after the one the sound moved in, live judges
    /// the sound left as a saved read judges its last chunk
    /// ([`LiveAnalyzer::settle`]): built again from the same chunk, with
    /// the same breaks.
    #[test]
    fn a_stop_judges_the_open_chunk_as_a_saved_read_does() {
        let (mut x, fs) = take(true);
        x.truncate(350 * fs as usize);
        let mut cfg = StreamConfig::default();
        cfg.analysis.bph = Some(28800);
        let saved = analyze_samples(&x, fs, &cfg);
        let l = live(&x, fs);
        assert_eq!(l.template_moves(), &[282.0]);
        assert_eq!(l.jumps(), &saved.jumps[..]);
    }
}
