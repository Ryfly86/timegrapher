//! When the beat template kept from the start of a take has moved to
//! another sound on one side (#1819, Norm's rule). The template is built
//! once and kept, so the beat's reference point stays put; but as a watch
//! runs down a side's sounds change, and on Peter's DayDate the kept
//! template timed the Tick 7 ms early, on the impulse, in the run-down's
//! last hours, which read a 7 ms drop beat error. Each chunk is also timed
//! with a template built from that chunk alone; a side whose beats sit
//! [`beats::SIDE_SOUND_GAP_S`] or more apart on the two has moved, and two
//! chunks in a row that agree rebuild the template there, with a break.

use crate::beats::{self, Beat};
use crate::stream::{first_beats_in, FirstBeats};

/// Chunks in a row that must show the same side moved, the same way and
/// within [`MOVE_AGREE_S`], before the template is built again: one chunk
/// can go wrong on noise, near the stop or on an empty stand; two that
/// agree is the least that is not a single draw (Norm, #1819).
pub const MOVE_CHUNKS: usize = 2;

/// How close two chunks' moves of one side must be to agree, seconds: a
/// side moved to one other sound moves by about the same in both.
pub const MOVE_AGREE_S: f64 = 0.001;

/// One side of a chunk found on another sound by its own template.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    /// The time of one of the side's beats on the kept template, seconds,
    /// so the side can be told in the next chunk: a whole number of beat
    /// pairs from it.
    pub at: f64,
    /// The median of its beats' times on the chunk's own template less
    /// their times on the kept one, seconds.
    pub by: f64,
}

/// The sides that moved over `from` to `to` seconds, between the beats
/// `kept` (timed on the kept template, sides by their index) and `own`
/// (timed on a template built from the same sound), at a beat of `beat`
/// seconds. Each `own` beat is paired with the nearest `kept` beat within
/// half a beat; a side with fewer than [`beats::SIDE_LINE_MIN`] pairs is
/// not judged.
pub fn side_moves(kept: &[Beat], own: &[Beat], from: f64, to: f64, beat: f64) -> Vec<Move> {
    let mut d: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    let mut at: [Option<f64>; 2] = [None, None];
    for b in own.iter().filter(|b| b.time >= from && b.time < to) {
        let i = kept.partition_point(|k| k.time < b.time);
        let near = [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter_map(|j| kept.get(j))
            .min_by(|x, y| (x.time - b.time).abs().total_cmp(&(y.time - b.time).abs()));
        let Some(k) = near.filter(|k| (k.time - b.time).abs() < 0.5 * beat) else {
            continue;
        };
        let side = k.index.rem_euclid(2) as usize;
        d[side].push(b.time - k.time);
        at[side].get_or_insert(k.time);
    }
    d.into_iter()
        .zip(at)
        .filter(|(d, _)| d.len() >= beats::SIDE_LINE_MIN)
        .filter_map(|(mut d, at)| {
            let by = crate::dsp::median(&mut d);
            (by.abs() >= beats::SIDE_SOUND_GAP_S).then_some(Move { at: at?, by })
        })
        .collect()
}

/// A chunk's beats on a template built from its own sound, as the first
/// chunk's is ([`first_beats_in`]): None where its sound is not heard as
/// a watch, which is never compared. `env` is its envelope, `bph` the
/// rate set (None, guessed), `window_s` the amplitude window.
pub fn own_beats(env: &[f32], fs: f64, bph: Option<u32>, window_s: f64) -> Option<FirstBeats> {
    first_beats_in(env, fs, bph, window_s).filter(|f| f.template.is_some())
}

/// The sides of a chunk that moved: its envelope `env`, starting `off`
/// seconds into the take, timed on the `kept` template at `bph` and on
/// its `own` ([`own_beats`]), over its core `from` to `to` seconds from
/// `env`'s start. The moves' times are the take's. None where its own
/// beats are at another rate, which is never compared. The kept template
/// carries no level from the sound before, so a saved read and live,
/// which read that sound in other pieces, judge a chunk alike.
pub fn chunk_moves(
    env: &[f32],
    fs: f64,
    (bph, kept): (u32, &[f32]),
    own: &FirstBeats,
    (from, to): (f64, f64),
    off: f64,
) -> Option<Vec<Move>> {
    if own.bph != bph || own.template.is_none() {
        return None;
    }
    let (k, _) = beats::detect_after(env, fs, bph, kept, None);
    let mut m = side_moves(&k, &own.beats, from, to, 3600.0 / bph as f64);
    m.iter_mut().for_each(|m| m.at += off);
    Some(m)
}

/// Whether `later`, a chunk's move, is the same as `first`, the chunk
/// before's: the same side (a whole number of beat pairs apart at a beat
/// of `beat` seconds), the same way, and within [`MOVE_AGREE_S`].
pub fn agrees(first: &Move, later: &Move, beat: f64) -> bool {
    let pairs = (later.at - first.at) / (2.0 * beat);
    (pairs - pairs.round()).abs() < 0.25
        && first.by.signum() == later.by.signum()
        && (first.by - later.by).abs() <= MOVE_AGREE_S
}

/// Whether some move in `later` agrees with some move in `first`.
pub fn any_agree(first: &[Move], later: &[Move], beat: f64) -> bool {
    first
        .iter()
        .any(|a| later.iter().any(|b| agrees(a, b, beat)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beats_at(n: i64, beat: f64, tick: f64, tock: f64) -> Vec<Beat> {
        (0..n)
            .map(|i| Beat {
                index: i,
                time: 1.0 + i as f64 * beat + if i % 2 == 0 { tick } else { tock },
                quality: 1.0,
            })
            .collect()
    }

    /// A side 7 ms off on its own template has moved; one 2 ms off, under
    /// the 3 ms of another sound, has not; nor has a side with too few
    /// beats to judge.
    #[test]
    fn a_side_moves_when_its_own_template_is_another_sound_away() {
        let beat = 0.125;
        let kept = beats_at(100, beat, 0.0, 0.0);
        let own = beats_at(100, beat, -0.007, 0.0);
        let m = side_moves(&kept, &own, 0.0, 100.0, beat);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!((m[0].by + 0.007).abs() < 1e-9, "{m:?}");
        assert!(side_moves(&kept, &beats_at(100, beat, -0.002, 0.0), 0.0, 100.0, beat).is_empty());
        let few = beats_at(16, beat, -0.007, 0.0);
        assert!(side_moves(&kept, &few, 0.0, 100.0, beat).is_empty());
    }

    /// Two chunks agree on the same side, the same way, within 1 ms; not
    /// on the other side, the other way, or 2 ms apart.
    #[test]
    fn two_moves_agree_on_side_way_and_size() {
        let beat = 0.125;
        let a = Move {
            at: 1.0,
            by: -0.007,
        };
        assert!(agrees(
            &a,
            &Move {
                at: 1.0 + 200.0 * beat,
                by: -0.0075
            },
            beat
        ));
        assert!(!agrees(
            &a,
            &Move {
                at: 1.0 + 201.0 * beat,
                by: -0.007
            },
            beat
        ));
        assert!(!agrees(
            &a,
            &Move {
                at: 1.0 + 200.0 * beat,
                by: 0.007
            },
            beat
        ));
        assert!(!agrees(
            &a,
            &Move {
                at: 1.0 + 200.0 * beat,
                by: -0.009
            },
            beat
        ));
    }
}
