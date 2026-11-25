//! A read of sound held in memory resumed after its first chunk (#2261).
//!
//! Live reads everything it has heard again as a file, up to two chunks
//! of it ([`crate::live::AS_FILE_MAX_S`]). Once a chunk's own stretch and
//! its margin are heard, that chunk's analysis can't change: every later
//! chunk carries on from the state it left, and the sound after it never
//! reaches it. So a read of more of the same sound starts from that state
//! and analyses only the sound after it: the same chunks, read the same
//! way, give the same log to the bit as a read from the start. The last,
//! partial chunk and its margin are always read again.

use super::*;
use std::sync::Arc;

/// Whether a read keeps its state as its first chunk left it, and that
/// state once kept ([`State::first`]).
#[derive(Clone, Debug, Default)]
pub(super) enum First {
    /// Not kept: a file's read, or a read resumed.
    #[default]
    Off,
    /// To be kept once the first chunk is whole.
    Wanted,
    /// Kept: its amplitude windows are added as they come
    /// ([`First::windows_found`]).
    Kept(Box<State>),
}

impl First {
    /// The kept first chunk's amplitude windows, from among those found
    /// during the read (`found`) and after it (`wins`), as found in turn.
    pub(super) fn windows_found(&mut self, found: &[Found], wins: &[Found]) {
        if let First::Kept(st) = self {
            let n = st.chunks;
            for f in found.iter().chain(wins).filter(|f| f.0 < n) {
                st.windows_found(f.clone());
            }
        }
    }
}

impl State {
    /// After a chunk whose own stretch ends at `core_b` is analysed: this
    /// state kept, if wanted and that chunk is the first and whole.
    pub(super) fn keep_first(&mut self, core_b: f64) {
        if matches!(self.first, First::Wanted) && self.chunks == 1 && core_b.is_finite() {
            let mut kept = self.clone();
            kept.first = First::Off;
            self.first = First::Kept(Box::new(kept));
        }
    }
}

/// A read's state after its first chunk, with what that chunk read: the
/// sound, its margin included, the sample rate and the settings. A read
/// resumes from it only when all three are the same
/// ([`analyze_samples_resumed`]).
pub struct Resume {
    sample_rate: u32,
    /// The settings as printed, every field.
    cfg: String,
    /// The first chunk's sound, its margin included.
    sound: Vec<f32>,
    st: State,
}

impl Resume {
    /// Whether a read of `samples` at `sample_rate` with `cfg` begins
    /// with the same first chunk read the same way.
    fn fits(&self, samples: &[f32], sample_rate: u32, cfg: &StreamConfig) -> bool {
        self.sample_rate == sample_rate
            && self.cfg == format!("{cfg:?}")
            && samples.len() >= self.sound.len()
            && (samples.iter().zip(&self.sound)).all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// [`analyze_samples`], resumed from `from` where it fits (the same
/// sample rate, settings and first chunk of sound), else read from the
/// start: the same log to the bit either way. With the state to resume
/// the next read of more of this sound from: `from` if it was used, else
/// this read's own, none while the first chunk isn't whole.
pub fn analyze_samples_resumed(
    samples: &[f32],
    sample_rate: u32,
    cfg: &StreamConfig,
    from: Option<Arc<Resume>>,
) -> (BeatLog, Option<Arc<Resume>>) {
    let (st, total, resume) = read_samples(samples, sample_rate, cfg, from);
    (st.finish(sample_rate, total), resume)
}

/// [`analyze_samples_resumed`] before the log is put together: the state
/// once every chunk is analysed, and the samples read.
pub(super) fn read_samples(
    samples: &[f32],
    sample_rate: u32,
    cfg: &StreamConfig,
    from: Option<Arc<Resume>>,
) -> (State, usize, Option<Arc<Resume>>) {
    let fs = sample_rate as f64;
    let mut cutter = Cutter::new(fs, cfg);
    let read = |begin: (State, Cutter), at: usize| {
        let fed = |push: &mut dyn FnMut(&[f32]) -> bool| {
            samples[at..].chunks(1 << 16).all(push);
            Ok(())
        };
        read_fed(begin, prepare_threads(), fed, |_| {}, |_| {}, false)
            .expect("sound held in memory reads without error")
    };
    if let Some(r) = from.filter(|r| r.fits(samples, sample_rate, cfg)) {
        // The cutter as the first chunk left it: the next chunk's own
        // stretch next, the sound from its margin on still to come.
        cutter.next = cutter.core;
        cutter.buf0 = cutter.core.saturating_sub(cutter.margin);
        cutter.total = cutter.buf0;
        let at = cutter.buf0;
        let (st, total) = read((r.st.clone(), cutter), at);
        return (st, total, Some(r));
    }
    let mut st = State::new(sample_rate, cfg);
    st.first = First::Wanted;
    let whole = cutter.core + cutter.margin;
    let (mut st, total) = read((st, cutter), 0);
    let resume = match std::mem::take(&mut st.first) {
        First::Kept(kept) => Some(Arc::new(Resume {
            sample_rate,
            cfg: format!("{cfg:?}"),
            sound: samples[..whole].to_vec(),
            st: *kept,
        })),
        _ => None,
    };
    (st, total, resume)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{generate, SynthConfig};

    /// A state's analysis, without what only says when the amplitude
    /// windows came back (in any order, as fast as the threads ran):
    /// every field a later chunk or the log reads.
    fn analysed(mut st: State) -> String {
        st.amp_out.clear();
        st.amp_back.clear();
        st.amp_found_to_s = 0.0;
        st.first = First::Off;
        format!("{st:?}")
    }

    /// A read of each length of `sound` in turn, each resumed from the
    /// last read's state as live's reads are, against a read from the
    /// start: the log, and the state before it, the same to the bit.
    /// The lengths the resume was used for.
    fn resumed_as_full(sound: &[f32], cfg: &StreamConfig, lens_s: &[f64]) -> Vec<f64> {
        let mut from = None;
        let mut used = Vec::new();
        for &s in lens_s {
            let x = &sound[..((s * 48_000.0) as usize).min(sound.len())];
            let had = from.clone();
            let (st, total, r) = read_samples(x, 48_000, cfg, from.take());
            if (had.as_ref().zip(r.as_ref())).is_some_and(|(a, b)| Arc::ptr_eq(a, b)) {
                used.push(s);
            }
            let (full, full_total, _) = read_samples(x, 48_000, cfg, None);
            assert_eq!(total, full_total, "{s} s");
            let (a, b) = (analysed(st.clone()), analysed(full.clone()));
            assert!(a == b, "the state after {s} s differs from a full read's");
            let got = format!("{:?}", st.finish(48_000, total));
            let want = format!("{:?}", full.finish(48_000, full_total));
            assert!(got == want, "the log of {s} s differs from a full read's");
            from = r;
        }
        used
    }

    fn synth(bph: u32, seed: u64, duration_s: f64) -> Vec<f32> {
        let cfg = SynthConfig {
            duration_s,
            bph,
            seed,
            ..Default::default()
        };
        generate(&cfg, |t| 280.0 - 0.3 * t, |_| 0.0).samples
    }

    /// The synthetic feed read again at asks as live makes them: before
    /// the first chunk and its margin are whole (48 s), across that end,
    /// and up to and past two chunks (94 s). The same log and state to
    /// the bit as a read from the start, resumed at every ask after the
    /// one that first heard 48 s, or that read from the start past it.
    #[test]
    fn a_resumed_read_is_a_full_reads_to_the_bit() {
        let sound = synth(28_800, 1, 100.0);
        let cfg = StreamConfig::default();
        let asks = [47.9, 48.0, 48.1, 94.0, 95.0, 100.0];
        let used = resumed_as_full(&sound, &cfg, &asks);
        assert_eq!(used, [48.1, 94.0, 95.0, 100.0]);
        // A resume kept by a read from the start after the second chunk
        // has amplitude windows, as after a setting changed at 60 s: only
        // the first chunk's go onto the kept state (Vernon on #2265).
        let used = resumed_as_full(&sound, &cfg, &[60.0, 70.0]);
        assert_eq!(used, [70.0]);
    }

    /// Chunks of 12 s with 1 s margins, so that more kinds of sound can
    /// be read across the chunk ends quickly.
    fn short_chunks(bph: Option<u32>) -> StreamConfig {
        StreamConfig {
            analysis: crate::analysis::AnalysisConfig {
                bph,
                ..Default::default()
            },
            chunk_s: 12.0,
            margin_s: 1.0,
        }
    }

    /// Asks across the first short chunk's end (13 s) and the second's
    /// (24 s).
    const SHORT_ASKS_S: [f64; 6] = [12.9, 13.1, 18.0, 24.0, 25.5, 30.0];

    /// Another beat rate, the watch first heard in the second chunk
    /// (silence before it), a gap across the first chunk's end, and a set
    /// beat rate: each read resumed is a full read's to the bit.
    #[test]
    fn a_resumed_read_of_a_late_watch_or_a_gap_is_a_full_reads() {
        let mut late = vec![0f32; 14 * 48_000];
        late.extend_from_slice(&synth(28_800, 4, 16.0));
        let mut gap = synth(28_800, 5, 30.0);
        gap[(11 * 48_000)..(14 * 48_000)].fill(0.0);
        for (name, sound, cfg) in [
            ("21600", synth(21_600, 2, 30.0), short_chunks(None)),
            ("late", late, short_chunks(None)),
            ("gap", gap, short_chunks(None)),
            (
                "set rate",
                synth(28_800, 6, 30.0),
                short_chunks(Some(28_800)),
            ),
        ] {
            let used = resumed_as_full(&sound, &cfg, &SHORT_ASKS_S);
            assert_eq!(used, SHORT_ASKS_S[2..], "{name}: resumed past 13 s");
        }
    }

    /// A resume is used only for the same sound, sample rate and settings:
    /// any change reads from the start, and the next resume is the new
    /// read's own.
    #[test]
    fn a_resume_is_dropped_on_any_change_of_its_inputs() {
        let sound = synth(28_800, 7, 20.0);
        let cfg = short_chunks(None);
        let (_, r) = analyze_samples_resumed(&sound[..15 * 48_000], 48_000, &cfg, None);
        let r = r.expect("the first chunk is whole");
        let mut lift = cfg.clone();
        lift.analysis.amplitude.lift_deg = 44.0;
        let mut margin = cfg.clone();
        margin.margin_s = 0.5;
        let mut other = sound.clone();
        other[5 * 48_000] += 0.01;
        for (name, x, fs, cfg) in [
            ("lift", &sound, 48_000, &lift),
            ("rate", &sound, 48_000, &short_chunks(Some(21_600))),
            ("chunks", &sound, 48_000, &margin),
            ("sample rate", &sound, 44_100, &cfg),
            ("sound", &other, 48_000, &cfg),
        ] {
            let (log, next) = analyze_samples_resumed(x, fs, cfg, Some(r.clone()));
            assert!(
                !next.as_ref().is_some_and(|n| Arc::ptr_eq(n, &r)),
                "{name}: read from the start"
            );
            assert_eq!(
                format!("{log:?}"),
                format!("{:?}", analyze_samples(x, fs, cfg)),
                "{name}"
            );
        }
        // Less than the first chunk: read whole, nothing to resume from.
        let short = &sound[..10 * 48_000];
        let (log, next) = analyze_samples_resumed(short, 48_000, &cfg, Some(r));
        assert!(next.is_none());
        assert_eq!(
            format!("{log:?}"),
            format!("{:?}", analyze_samples(short, 48_000, &cfg))
        );
    }
}
