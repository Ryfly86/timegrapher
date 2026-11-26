//! How live renumbers the beats it has already given, so a view anchored
//! on them (the app's strip, `record --live`'s strip) moves with them
//! instead of each one jumping a beat period and swapping sides (#2318).

use super::{fresh_gen, LiveAnalyzer};
use crate::timing::Anchor;

/// One renumbering: every beat numbered `from` or later (as numbered
/// before the move) moved by `dt_s` seconds and `di` in the count.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Move {
    from: i64,
    dt_s: f64,
    di: i64,
}

/// The renumberings of one run of beats, oldest first. A new run (a
/// restart, a reading of the sound again whose beats can't be paired with
/// these) has a new `run`, and a view anchored on the old one starts again.
#[derive(Debug, Clone)]
pub(super) struct Numbering {
    run: u64,
    moves: Vec<Move>,
}

impl Default for Numbering {
    fn default() -> Self {
        Numbering {
            run: fresh_gen(),
            moves: Vec::new(),
        }
    }
}

impl Numbering {
    /// Every beat numbered `from` or later numbered one further: the sides
    /// were named, or found swapped after a jump
    /// ([`crate::beats::SideCheck`]).
    pub(super) fn one_further_from(&mut self, from: i64) {
        self.moves.push(Move {
            from,
            dt_s: 0.0,
            di: 1,
        });
    }

    /// The beats of a reading of the sound again took over from these
    /// (#1327), moved by `shift` ([`LiveAnalyzer::template_shift`]): the
    /// run goes on with the move added. Beats that could not be paired
    /// start a new run.
    pub(super) fn taken_over(self, shift: Option<(f64, i64)>) -> Numbering {
        let Some((dt_s, di)) = shift else {
            return Numbering::default();
        };
        let mut moves = self.moves;
        moves.push(Move {
            from: i64::MIN,
            dt_s,
            di,
        });
        Numbering {
            run: self.run,
            moves,
        }
    }
}

/// How far a view has followed live's numbering of its beats
/// ([`LiveAnalyzer::follow_numbering`]). Start at the default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NumberingSeen {
    run: u64,
    moves: usize,
}

impl LiveAnalyzer {
    /// Move `anchor` with every renumbering of the beats since `seen`, and
    /// bring `seen` up to date. Where every beat was renumbered (the sides
    /// named the other way round, a takeover) a beat's lead on the anchor
    /// stays where it was drawn and its side swaps; where only the beats
    /// after a jump were, the anchor before it stays and those beats move
    /// a beat period. When the beats are a new run (a restart, a reading
    /// again whose beats can't be paired) the anchor is dropped, to be set
    /// again from the new beats. True when the beats already given were
    /// numbered again since `seen`, while anchored.
    pub fn follow_numbering(&self, anchor: &mut Option<Anchor>, seen: &mut NumberingSeen) -> bool {
        let n = &self.numbering;
        let now = NumberingSeen {
            run: n.run,
            moves: n.moves.len(),
        };
        let was = std::mem::replace(seen, now);
        // Never followed before: the anchor was set on the beats as they are.
        if was.run == 0 {
            return false;
        }
        if was.run != n.run {
            return anchor.take().is_some();
        }
        let Some(a) = anchor.as_mut() else {
            return false;
        };
        let new = n.moves.get(was.moves..).unwrap_or_default();
        // By the anchor's beat number, not its time: the anchor's time is
        // projected from several beats and may fall either side of a jump.
        for m in new {
            if a.index >= m.from {
                (a.time, a.index) = (a.time + m.dt_s, a.index + m.di);
            }
        }
        !new.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::LiveConfig;
    use crate::synth::{generate, SynthConfig};
    use std::collections::BTreeMap;

    /// Seconds behind the newest sound past which a beat is no longer timed
    /// again ([`LiveAnalyzer::redrop_heard_blocks`] gives a beat again for
    /// up to a pass's window, 4 s, after it was first given).
    const SETTLED_S: f64 = 5.0;

    /// A strip anchored on the first 16 beats, as the app and `record
    /// --live` anchor it, followed through 20 s of a watch whose Tock comes
    /// late (beat error `beat_error_ms`): each beat's lead and side every
    /// second once it is `SETTLED_S` old, keyed by its time in 0.1 ms, and
    /// how many times the anchor moved.
    fn strip_seen(beat_error_ms: f64) -> (BTreeMap<i64, Vec<(f64, i64)>>, usize) {
        let cfg = SynthConfig {
            duration_s: 20.0,
            beat_error_ms,
            ..Default::default()
        };
        let audio = generate(&cfg, |_| 270.0, |_| 0.0);
        let mut live = LiveAnalyzer::new(cfg.sample_rate, LiveConfig::default());
        let (mut anchor, mut seen, mut moves) = (None, NumberingSeen::default(), 0);
        let mut drawn: BTreeMap<i64, Vec<(f64, i64)>> = BTreeMap::new();
        for second in audio.samples.chunks(cfg.sample_rate as usize) {
            live.push(second);
            moves += usize::from(live.follow_numbering(&mut anchor, &mut seen));
            let Some(p) = live.bph().map(|b| 3600.0 / b as f64) else {
                continue;
            };
            let beats = live.beats();
            if anchor.is_none() && beats.len() >= 16 {
                anchor = Anchor::centre_on(&beats[..16], p, 16);
            }
            let Some(a) = anchor else {
                continue;
            };
            let settled = live.duration_s() - SETTLED_S;
            for b in beats.iter().filter(|b| b.time < settled) {
                let key = (b.time * 1e4).round() as i64;
                drawn
                    .entry(key)
                    .or_default()
                    .push((a.lead_ms(b, p), b.index.rem_euclid(2)));
            }
        }
        (drawn, moves)
    }

    /// #2318: with the Tock late, the sides are named the other way round
    /// about 10 s in and every beat so far is numbered one further. A beat
    /// already drawn keeps its lead (it jumped a beat period, 125 ms at
    /// 28,800 bph) and swaps side once, as the sides are named.
    #[test]
    fn a_drawn_beat_keeps_its_lead_when_the_sides_are_named() {
        let (drawn, moves) = strip_seen(-1.5);
        assert_eq!(moves, 1, "the sides named once");
        let mut swapped = 0;
        for seen in drawn.values() {
            let (lead, side) = seen[0];
            for &(l, s) in seen {
                assert!((l - lead).abs() < 1.0, "lead {lead} then {l} ms");
                swapped += usize::from(s != side);
            }
        }
        assert!(swapped > 0, "the beats drawn before the naming swap sides");
    }

    /// Named the way they were counted (Tick late), nothing moves.
    #[test]
    fn nothing_moves_when_the_count_stands() {
        let (drawn, moves) = strip_seen(1.5);
        assert_eq!(moves, 0);
        for seen in drawn.values() {
            assert!(seen
                .iter()
                .all(|x| (x.0 - seen[0].0).abs() < 1.0 && x.1 == seen[0].1));
        }
    }

    fn anchored() -> (LiveAnalyzer, Option<Anchor>, NumberingSeen) {
        let live = LiveAnalyzer::new(48_000, LiveConfig::default());
        let mut seen = NumberingSeen::default();
        let mut anchor = None;
        live.follow_numbering(&mut anchor, &mut seen);
        let anchor = Some(Anchor {
            index: 40,
            time: 5.0,
        });
        (live, anchor, seen)
    }

    /// A renumbering from a jump moves the anchor only when it is on the
    /// beats renumbered, and each is followed once.
    #[test]
    fn a_renumbering_after_the_anchor_leaves_it_and_each_is_followed_once() {
        let (mut live, mut anchor, mut seen) = anchored();
        live.numbering.one_further_from(41);
        assert!(live.follow_numbering(&mut anchor, &mut seen));
        assert_eq!(anchor.map(|a| a.index), Some(40));
        assert!(!live.follow_numbering(&mut anchor, &mut seen));
        live.numbering.one_further_from(40);
        assert!(live.follow_numbering(&mut anchor, &mut seen));
        assert!(!live.follow_numbering(&mut anchor, &mut seen));
        assert_eq!(anchor.map(|a| a.index), Some(41));
    }

    /// #1327's takeover moves the anchor by the template's shift, in time
    /// and count; unpaired beats, or a restart, drop it to be set again.
    #[test]
    fn a_takeover_moves_the_anchor_and_a_new_run_drops_it() {
        let (mut live, mut anchor, mut seen) = anchored();
        live.numbering = std::mem::take(&mut live.numbering).taken_over(Some((0.002, 1)));
        assert!(live.follow_numbering(&mut anchor, &mut seen));
        let a = anchor.expect("kept");
        assert!((a.time - 5.002).abs() < 1e-12 && a.index == 41, "{a:?}");
        live.numbering = std::mem::take(&mut live.numbering).taken_over(None);
        assert!(live.follow_numbering(&mut anchor, &mut seen));
        assert!(anchor.is_none());
        let mut anchor = Some(a);
        live.restart();
        assert!(live.follow_numbering(&mut anchor, &mut seen));
        assert!(anchor.is_none());
    }
}
