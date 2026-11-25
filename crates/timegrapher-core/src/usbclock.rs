//! The sound card's clock error, read from the recording itself.
//!
//! A USB microphone hears its own bus: the host sends a start-of-frame
//! packet every millisecond, and the card's supply carries it into the
//! audio as faint lines at exact multiples of 1 kHz. Those lines run on the
//! host's clock, so where they land on the card's clock gives the card's
//! error with no system-clock log at all. On the stored takes the lines
//! from 3 to 10 kHz agree with one another within 0.1 ppm and with the
//! NTP clock logs within about 1 ppm (0.1 s/d), the host's own crystal
//! error. A card with no such lines (a non-USB input, a clean supply) gives
//! no answer.
//!
//! The lines time the card against whichever crystal makes the 1 ms
//! frames: behind a hub, dock or Thunderbolt device with its own USB
//! controller, that one's, not the computer's. A reading no sound card's
//! crystal could give ([`MAX_CARD_PPM`]) is kept, but not applied
//! ([`UsbClock::applies`], #2283).

use realfft::num_complex::Complex;
use serde::Serialize;

/// The card's error from the USB lines. Written with a `withheld` reason,
/// [`IMPLAUSIBLE`], when it is not applied ([`UsbClock::applies`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsbClock {
    /// Positive: the card's clock runs fast, as in [`crate::clockstore`].
    pub ppm: f64,
    /// Lines that stood clear of the noise and agreed.
    pub lines: usize,
    /// Largest difference between those lines, ppm.
    pub spread_ppm: f64,
}

/// Largest sound-card clock error, ppm either way, the USB lines are
/// applied with (Norm on #2283): an ordinary crystal's tolerance,
/// temperature and ageing approach but stay under it, where cards are
/// typically 10 to 50 ppm off ([`crate::clock`]). A reading past it times
/// the card against another crystal, such as a hub's or a Thunderbolt
/// device's USB controller (#2291 read +151.1 ppm through one; #2292, the
/// same microphone plugged in direct, +9.3 ppm). It lies inside the
/// search, the ±200 ppm `SEARCH_PPM`.
pub const MAX_CARD_PPM: f64 = 100.0;

/// Why a USB-line reading is not applied: it is past [`MAX_CARD_PPM`].
pub const IMPLAUSIBLE: &str = "usb_lines_implausible";

impl Serialize for UsbClock {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let withheld = self.withheld();
        let mut st = s.serialize_struct("UsbClock", 3 + usize::from(withheld.is_some()))?;
        st.serialize_field("ppm", &self.ppm)?;
        st.serialize_field("lines", &self.lines)?;
        st.serialize_field("spread_ppm", &self.spread_ppm)?;
        if let Some(w) = withheld {
            st.serialize_field("withheld", w)?;
        }
        st.end()
    }
}

impl UsbClock {
    /// Whether the reading corrects the rates: the one check every surface
    /// takes it through, live and saved alike (#2283). Kept within
    /// [`MAX_CARD_PPM`] either way, exactly at it included.
    pub fn applies(&self) -> bool {
        self.ppm.abs() <= MAX_CARD_PPM
    }

    /// The reading when it applies ([`UsbClock::applies`]).
    pub fn applied(self) -> Option<UsbClock> {
        self.applies().then_some(self)
    }

    /// Why the reading is not applied, [`IMPLAUSIBLE`]; `None` when it is.
    pub fn withheld(&self) -> Option<&'static str> {
        (!self.applies()).then_some(IMPLAUSIBLE)
    }

    /// The card's error read, whole ppm, with no sign.
    fn whole_ppm(&self) -> String {
        format!("{:.0}", self.ppm.abs())
    }

    /// What a reader is told of a reading not applied, in a line (Hugh on
    /// #2283); `None` when it applies.
    pub fn not_used_short(&self) -> Option<String> {
        self.withheld()?;
        Some(format!(
            "USB lines not used: card read {} ppm off",
            self.whole_ppm()
        ))
    }

    /// [`UsbClock::not_used_short`] in full, with what the user can do.
    pub fn not_used_text(&self) -> Option<String> {
        self.withheld()?;
        Some(format!(
            "The USB lines read the card {} ppm off, more than a sound card's clock should be, so they are not used. If the mic is plugged into a hub, dock or Thunderbolt device, plug it straight into the computer.",
            self.whole_ppm()
        ))
    }

    /// The correction and source text used in analysis documents.
    pub fn correction(&self) -> (f64, String) {
        (
            self.ppm,
            format!(
                "from {} USB lines in the recording, {:.1} ppm apart",
                self.lines, self.spread_ppm
            ),
        )
    }
}

/// Lines searched: harmonics of 1 kHz in this range, Hz.
const LOWEST_HZ: u32 = 3000;
const HIGHEST_HZ: u32 = 10000;
/// Search this far either side of each harmonic, ppm.
const SEARCH_PPM: f64 = 200.0;
/// A line counts when its peak is this far above the median of its search
/// range, dB.
const MIN_SNR_DB: f64 = 20.0;
/// Lines further than this from their median are dropped, ppm.
const AGREE_PPM: f64 = 1.0;
/// Shortest recording worth measuring, seconds.
pub const MIN_DURATION_S: f64 = 30.0;
/// Longest stretch measured, seconds: ten minutes already gives 0.1 ppm,
/// and the search cost grows with the square of the length.
pub const MAX_DURATION_S: f64 = 600.0;
/// Rate the baseband is decimated to, Hz: wide enough for the search range
/// at 10 kHz (±2 Hz).
const BASEBAND_HZ: f64 = 8.0;

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Frequency offset of the line nearest `f0`, ppm, and its height above
/// the noise, dB.
fn line(x: &[f32], fs: u32, f0: u32) -> Option<(f64, f64)> {
    // Mix down to baseband with an exact table (f0 and fs are whole Hz,
    // so the phase repeats every fs / gcd samples), summing blocks.
    let period = (fs / gcd(fs, f0)) as usize;
    let table: Vec<Complex<f64>> = (0..period)
        .map(|i| {
            let a = -2.0 * std::f64::consts::PI * f0 as f64 * i as f64 / fs as f64;
            Complex::new(a.cos(), a.sin())
        })
        .collect();
    let block = (fs as f64 / BASEBAND_HZ).round() as usize;
    let n = x.len() / block;
    if n < 8 {
        return None;
    }
    let mut z: Vec<Complex<f64>> = Vec::with_capacity(n);
    let mut k = 0usize;
    for b in 0..n {
        let mut acc = Complex::new(0.0, 0.0);
        for &v in &x[b * block..(b + 1) * block] {
            acc += table[k] * v as f64;
            k += 1;
            if k == period {
                k = 0;
            }
        }
        // Hann window over the whole recording.
        let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * (b as f64 + 0.5) / n as f64).cos();
        z.push(acc * w);
    }
    let dt = block as f64 / fs as f64;
    let t_total = n as f64 * dt;
    let power = |df: f64| -> f64 {
        let step = Complex::from_polar(1.0, -2.0 * std::f64::consts::PI * df * dt);
        let mut rot = Complex::from_polar(1.0, -std::f64::consts::PI * df * dt);
        let mut s = Complex::new(0.0, 0.0);
        for &v in &z {
            s += v * rot;
            rot *= step;
        }
        s.norm_sqr()
    };
    // Coarse grid at a quarter of the resolution, then a parabola on the
    // log power through the peak and its neighbours.
    let span = f0 as f64 * SEARCH_PPM * 1e-6;
    let step = 0.25 / t_total;
    let m = (span / step).ceil() as i64;
    let grid: Vec<f64> = (-m..=m).map(|i| i as f64 * step).collect();
    let p: Vec<f64> = grid.iter().map(|&d| power(d)).collect();
    let (i, &peak) = p.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    let floor = crate::dsp::upper_median(&mut p.clone());
    if floor <= 0.0 || i == 0 || i + 1 == p.len() {
        return None;
    }
    let snr_db = 10.0 * (peak / floor).log10();
    let (a, b, c) = (p[i - 1].ln(), peak.ln(), p[i + 1].ln());
    let den = a - 2.0 * b + c;
    let frac = if den < 0.0 { 0.5 * (a - c) / den } else { 0.0 };
    let df = grid[i] + frac * step;
    // A line at f0 true hertz is heard at f0 × card_scale card hertz
    // ([`crate::clock::card_scale`]), so df / f0 is card_scale − 1, which
    // is −ppm × 1e-6: taken from df directly, with no 1 + df / f0 to round.
    Some((-df / f0 as f64 * 1e6, snr_db))
}

/// The card's clock error from the USB lines in `x`, or `None` when fewer
/// than two lines stand clear and agree.
pub fn measure(x: &[f32], fs: u32) -> Option<UsbClock> {
    if (x.len() as f64) < MIN_DURATION_S * fs as f64 {
        return None;
    }
    let x = &x[..x.len().min((MAX_DURATION_S * fs as f64) as usize)];
    // Below 2 kHz sampling no line fits under the Nyquist limit.
    let top = HIGHEST_HZ.min((fs / 2).saturating_sub(1000));
    let mut ppm: Vec<f64> = (LOWEST_HZ..=top)
        .step_by(1000)
        .filter_map(|f0| line(x, fs, f0))
        .filter(|&(_, snr)| snr >= MIN_SNR_DB)
        .map(|(p, _)| p)
        .collect();
    if ppm.len() < 2 {
        return None;
    }
    let mid = crate::dsp::median(&mut ppm.clone());
    ppm.retain(|p| (p - mid).abs() <= AGREE_PPM);
    if ppm.len() < 2 {
        return None;
    }
    let lo = ppm.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = ppm.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(UsbClock {
        ppm: crate::dsp::median(&mut ppm.clone()),
        lines: ppm.len(),
        spread_ppm: hi - lo,
    })
}

/// The lengths of sound, seconds, at which a [`Live`] reading measures the
/// USB lines: from [`MIN_DURATION_S`], doubling, to [`MAX_DURATION_S`],
/// past which a saved take reads no more. Each measurement is the one a
/// take saved at that length reads ([`measure`] on the same samples); the
/// doubling keeps their cost, which grows with the square of the length,
/// to about twice the last one's.
pub const LIVE_READS_S: [f64; 6] = [MIN_DURATION_S, 60.0, 120.0, 240.0, 480.0, MAX_DURATION_S];

/// Where a live reading's USB-line clock stands (#1721).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LiveClock {
    /// Too little sound for the lines yet: `heard_s` of [`MIN_DURATION_S`].
    NotMeasured { heard_s: f64 },
    /// Measured on the first `over_s` of sound, as a take saved at that
    /// length reads them.
    Measured { clock: UsbClock, over_s: f64 },
    /// The first `over_s` of sound holds no lines that stand clear and
    /// agree, so a take saved then reads none either.
    NoLines { over_s: f64 },
    /// The lines of the first `over_s` of sound read a card error past
    /// [`MAX_CARD_PPM`]: kept, and not applied, as a take saved then
    /// keeps them (#2283).
    Implausible { clock: UsbClock, over_s: f64 },
}

impl LiveClock {
    /// The lines' correction, ppm, when they were found and apply.
    pub fn usb_ppm(&self) -> Option<f64> {
        match self {
            LiveClock::Measured { clock, .. } => Some(clock.ppm),
            _ => None,
        }
    }

    /// What a live rate must say beside it while the recording's own clock
    /// is still to be read; `None` once it is read, lines or none, when the
    /// rate is the one the take reads saved and opened again. `kept`: a
    /// correction known for the input is applied meanwhile.
    pub fn not_yet(&self, kept: bool) -> Option<String> {
        let LiveClock::NotMeasured { heard_s } = self else {
            return None;
        };
        let on = if kept {
            "the correction kept for this input"
        } else {
            "the sound card's clock"
        };
        Some(format!(
            "For now this rate is on {on}. The recording's own clock (its USB lines) is read once {MIN_DURATION_S:.0} s of sound are in: {:.0} s so far.",
            heard_s.max(0.0).floor()
        ))
    }

    /// The length of sound measured, once it is.
    fn over_s(&self) -> Option<f64> {
        match *self {
            LiveClock::Measured { over_s, .. }
            | LiveClock::NoLines { over_s }
            | LiveClock::Implausible { over_s, .. } => Some(over_s),
            LiveClock::NotMeasured { .. } => None,
        }
    }

    /// What a live rate must say beside it once the lines are read (#2283):
    /// for lines not applied, [`UsbClock::not_used_text`]; for lines that
    /// put the rate on another clock than `kept_ppm`'s (the correction
    /// kept for the input, or none: the card's own), by how much the shown
    /// rate moved, when that is 0.1 s/d or more as written. `None` before
    /// the lines are read ([`LiveClock::not_yet`] says so then), with no
    /// lines, or when the rate stays where it was.
    pub fn read_note(&self, kept_ppm: Option<f64>) -> Option<String> {
        match self {
            LiveClock::Implausible { clock, .. } => clock.not_used_text(),
            LiveClock::Measured { clock, .. } => step_note(clock.ppm, kept_ppm),
            _ => None,
        }
    }
}

/// [`LiveClock::read_note`] for lines read at `ppm` fast, after a rate on
/// `kept_ppm`'s clock: the card's error whole and signed, and the shown
/// rate's move, signed, at the rate's place ([`crate::figure_text`]).
fn step_note(ppm: f64, kept_ppm: Option<f64>) -> Option<String> {
    use crate::clock::rate_error_s_per_day;
    use crate::figure_text::{base_digits, figure, signed_text};
    // A rate corrected by a clock moves by minus its rate error.
    let moved = rate_error_s_per_day(kept_ppm.unwrap_or(0.0)) - rate_error_s_per_day(ppm);
    let moved = figure(moved, None, base_digits::RATE, true);
    if moved.bytes().all(|b| b == b'0' || b == b'.') {
        return None;
    }
    Some(format!(
        "USB lines corrected the clock (card {} ppm), moving the rate by {moved} s/d",
        signed_text(ppm, 0)
    ))
}

/// The USB-line clock read while the sound is heard (#1721): the first
/// [`MAX_DURATION_S`] are kept, as a saved take's reading keeps them, and
/// measured at each length of [`LIVE_READS_S`] reached. [`Live::due`]
/// says when a measurement is due, [`Live::samples`] gives its sound (to
/// measure off the audio thread with [`measure_at`]) and [`Live::set`]
/// takes its answer; [`Live::clock`] is the latest.
#[derive(Debug, Clone)]
pub struct Live {
    fs: u32,
    keep: usize,
    x: Vec<f32>,
    heard: usize,
    clock: Option<LiveClock>,
}

impl Live {
    pub fn new(fs: u32) -> Self {
        Live {
            fs,
            keep: (MAX_DURATION_S * f64::from(fs)) as usize,
            x: Vec::new(),
            heard: 0,
            clock: None,
        }
    }

    /// The samples after those heard so far.
    pub fn push(&mut self, block: &[f32]) {
        if self.x.len() < self.keep {
            let take = (self.keep - self.x.len()).min(block.len());
            self.x.extend_from_slice(&block[..take]);
        }
        self.heard += block.len();
    }

    /// The sound kept: the first [`MAX_DURATION_S`].
    pub fn kept(&self) -> &[f32] {
        &self.x
    }

    fn heard_s(&self) -> f64 {
        self.heard as f64 / f64::from(self.fs)
    }

    fn over_s(&self) -> Option<f64> {
        self.clock.as_ref().and_then(LiveClock::over_s)
    }

    /// The longest length of [`LIVE_READS_S`] heard and not yet measured.
    pub fn due(&self) -> Option<f64> {
        let heard = self.heard_s();
        LIVE_READS_S
            .iter()
            .copied()
            .rev()
            .find(|&t| t <= heard)
            .filter(|&t| self.over_s().is_none_or(|o| t > o))
    }

    /// The first `over_s` of sound kept, to measure.
    pub fn samples(&self, over_s: f64) -> &[f32] {
        let n = (over_s * f64::from(self.fs)).round() as usize;
        &self.x[..n.min(self.x.len())]
    }

    /// A measurement's answer, kept unless a longer one is already in.
    pub fn set(&mut self, clock: LiveClock) {
        let over = |c: &LiveClock| c.over_s().unwrap_or(f64::NEG_INFINITY);
        if self.clock.as_ref().is_none_or(|c| over(&clock) > over(c)) {
            self.clock = Some(clock);
        }
    }

    /// The latest measurement, or how much sound is in until the first.
    pub fn clock(&self) -> LiveClock {
        self.clock.unwrap_or(LiveClock::NotMeasured {
            heard_s: self.heard_s(),
        })
    }

    /// Measure what is due now, on this thread: for a caller with no
    /// thread of its own to measure on, and for tests.
    pub fn measure_due(&mut self) -> LiveClock {
        if let Some(t) = self.due() {
            let c = measure_at(self.samples(t), self.fs, t);
            self.set(c);
        }
        self.clock()
    }
}

/// [`measure`] on `x`, the first `over_s` of a reading's sound, as a
/// [`LiveClock`], judged by [`UsbClock::applies`] as a saved take's is.
pub fn measure_at(x: &[f32], fs: u32, over_s: f64) -> LiveClock {
    judged(measure(x, fs), over_s)
}

/// A measurement of the first `over_s` of sound as a [`LiveClock`].
fn judged(clock: Option<UsbClock>, over_s: f64) -> LiveClock {
    match clock {
        Some(clock) if clock.applies() => LiveClock::Measured { clock, over_s },
        Some(clock) => LiveClock::Implausible { clock, over_s },
        None => LiveClock::NoLines { over_s },
    }
}

#[cfg(test)]
mod tests {
    /// Sound sampled too slowly to hold any USB line reads no clock, rather
    /// than overflowing on the line search's upper end.
    #[test]
    fn slow_sampling_reads_no_clock() {
        for fs in [100, 1000, 1999, 2000] {
            assert!(
                super::measure(&vec![0.0; 120 * fs as usize], fs).is_none(),
                "{fs} Hz"
            );
        }
    }

    use super::*;

    fn noise(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 11) as f64 / (1u64 << 53) as f64 - 0.5) as f32
            })
            .collect()
    }

    #[test]
    fn finds_a_slow_card_from_its_lines() {
        // A card 20 ppm slow counts fewer samples per true second, so a
        // true 5, 6 and 7 kHz line lands 20 ppm high on its clock.
        let fs = 48000u32;
        let n = fs as usize * 60;
        let mut x = noise(n, 7);
        for v in x.iter_mut() {
            *v *= 0.02;
        }
        for (f, a) in [(5000.0, 0.002), (6000.0, 0.003), (7000.0, 0.002)] {
            let f = f * (1.0 + 20e-6);
            for (i, v) in x.iter_mut().enumerate() {
                *v += (a * (2.0 * std::f64::consts::PI * f * i as f64 / fs as f64).sin()) as f32;
            }
        }
        let c = measure(&x, fs).expect("lines found");
        assert!((c.ppm + 20.0).abs() < 0.2, "ppm {}", c.ppm);
        assert!(c.lines >= 3, "lines {}", c.lines);
    }

    #[test]
    fn no_lines_no_answer() {
        let fs = 48000u32;
        assert_eq!(measure(&noise(fs as usize * 40, 3), fs), None);
        assert_eq!(measure(&noise(fs as usize * 5, 3), fs), None);
    }

    /// A live reading measures at each length of LIVE_READS_S the lines a
    /// take saved at that length reads, keeps the longest, and says the
    /// rate is not yet on the recording's clock before the first.
    #[test]
    fn a_live_reading_measures_what_a_saved_take_would() {
        let fs = 48000u32;
        let n = fs as usize * 130;
        let mut x = noise(n, 11);
        for v in x.iter_mut() {
            *v *= 0.02;
        }
        for (f, a) in [(5000.0, 0.002), (6000.0, 0.003), (7000.0, 0.002)] {
            let f = f * (1.0 - 19.3e-6);
            for (i, v) in x.iter_mut().enumerate() {
                *v += (a * (2.0 * std::f64::consts::PI * f * i as f64 / fs as f64).sin()) as f32;
            }
        }
        let mut live = Live::new(fs);
        let mut seen = Vec::new();
        for block in x.chunks(4800) {
            live.push(block);
            if let Some(t) = live.due() {
                seen.push(t);
                let c = measure_at(live.samples(t), fs, t);
                let saved = measure(&x[..(t * fs as f64) as usize], fs).expect("lines");
                assert_eq!(
                    c,
                    LiveClock::Measured {
                        clock: saved,
                        over_s: t
                    }
                );
                live.set(c);
            }
            if live.due().is_none() && seen.is_empty() {
                let note = live.clock().not_yet(false).expect("not measured yet");
                assert!(note.contains("sound card"), "{note}");
                assert_eq!(live.clock().usb_ppm(), None);
            }
        }
        assert_eq!(seen, [30.0, 60.0, 120.0]);
        assert!((live.clock().usb_ppm().unwrap() - 19.3).abs() < 0.2);
        assert_eq!(live.clock().not_yet(true), None);
        // A shorter answer arriving late does not replace a longer one.
        live.set(LiveClock::NoLines { over_s: 60.0 });
        assert!(matches!(
            live.clock(),
            LiveClock::Measured { over_s: 120.0, .. }
        ));
        assert_eq!(live.kept().len(), n);
    }

    fn at(ppm: f64) -> UsbClock {
        UsbClock {
            ppm,
            lines: 6,
            spread_ppm: 0.05,
        }
    }

    /// A reading `ppm` off that applies, live and saved alike.
    fn assert_kept(ppm: f64) {
        let c = at(ppm);
        assert!(c.applies(), "{ppm}");
        assert_eq!(c.applied(), Some(c));
        assert_eq!(c.withheld(), None);
        assert_eq!(c.not_used_text(), None);
        let live = judged(Some(c), 30.0);
        assert_eq!(
            live,
            LiveClock::Measured {
                clock: c,
                over_s: 30.0
            }
        );
        assert_eq!(live.usb_ppm(), Some(ppm));
        let json = serde_json::to_value(c).unwrap();
        assert!(json.get("withheld").is_none(), "{json}");
        assert!(crate::take::live_clock(&live, None, 300.0).is_some());
    }

    /// A reading `ppm` off, 100.1 either way, kept with its reason and
    /// applied nowhere, live and saved alike.
    fn assert_withheld(ppm: f64) {
        let c = at(ppm);
        assert!(!c.applies(), "{ppm}");
        assert_eq!(c.applied(), None);
        assert_eq!(c.withheld(), Some(IMPLAUSIBLE));
        let live = judged(Some(c), 30.0);
        assert_eq!(
            live,
            LiveClock::Implausible {
                clock: c,
                over_s: 30.0
            }
        );
        assert_eq!(live.usb_ppm(), None);
        assert_eq!(live.not_yet(false), None);
        // Nothing measured is lost.
        let json = serde_json::to_value(c).unwrap();
        assert_eq!(json["withheld"], "usb_lines_implausible");
        assert_eq!(json["ppm"], ppm);
        assert_eq!(json["lines"], 6);
        // The card's own clock, or a kept correction, stays.
        assert!(crate::take::live_clock(&live, None, 300.0).is_none());
        let kept = crate::take::live_clock(&live, Some(5.0), 300.0).expect("the kept one");
        assert_eq!(kept.ppm, 5.0);
        assert_eq!(
            live.read_note(None).as_deref(),
            Some(
                "The USB lines read the card 100 ppm off, more than a sound card's clock \
                 should be, so they are not used. If the mic is plugged into a hub, dock or \
                 Thunderbolt device, plug it straight into the computer."
            )
        );
        assert_eq!(
            c.not_used_short().as_deref(),
            Some("USB lines not used: card read 100 ppm off")
        );
    }

    /// Norm's bound on #2283, at its edge: a card read 100.0 ppm off either
    /// way is kept and applied; 100.1 ppm is kept with its reason and
    /// corrects nothing, live and saved alike.
    #[test]
    fn usb_lines_past_a_cards_largest_error_are_kept_and_not_applied() {
        for ppm in [MAX_CARD_PPM, -MAX_CARD_PPM, 100.0, -100.0] {
            assert_kept(ppm);
        }
        for ppm in [100.1, -100.1] {
            assert_withheld(ppm);
        }
        // A later read, judged alone, replaces a withheld one.
        let mut live = Live::new(48_000);
        live.set(judged(Some(at(151.1)), 30.0));
        live.set(judged(Some(at(9.3)), 60.0));
        assert_eq!(live.clock().usb_ppm(), Some(9.3));
        live.set(judged(Some(at(151.1)), 120.0));
        assert_eq!(live.clock().usb_ppm(), None);
    }

    /// Hugh's words on #2283: the rate's move when the lines move the
    /// clock, fast and slow, and none when it stays where it was.
    #[test]
    fn a_read_that_moves_the_clock_says_by_how_much() {
        let read = |ppm: f64| LiveClock::Measured {
            clock: UsbClock {
                ppm,
                lines: 6,
                spread_ppm: 0.16,
            },
            over_s: 30.0,
        };
        assert_eq!(
            read(9.3).read_note(None).as_deref(),
            Some("USB lines corrected the clock (card +9 ppm), moving the rate by +0.8 s/d")
        );
        assert_eq!(
            read(-19.3).read_note(None).as_deref(),
            Some("USB lines corrected the clock (card -19 ppm), moving the rate by -1.7 s/d")
        );
        // From a kept correction of +20 ppm to the lines' +9.3.
        assert_eq!(
            read(9.3).read_note(Some(20.0)).as_deref(),
            Some("USB lines corrected the clock (card +9 ppm), moving the rate by -0.9 s/d")
        );
        assert_eq!(read(9.3).read_note(Some(9.3)), None);
        assert_eq!(read(0.4).read_note(None), None, "under 0.05 s/d");
        assert_eq!(LiveClock::NoLines { over_s: 30.0 }.read_note(None), None);
        assert_eq!(
            LiveClock::NotMeasured { heard_s: 3.0 }.read_note(None),
            None
        );
    }

    #[test]
    fn a_live_reading_with_no_lines_says_so_once_measured() {
        let fs = 48000u32;
        let mut live = Live::new(fs);
        live.push(&noise(fs as usize * 31, 5));
        assert_eq!(live.measure_due(), LiveClock::NoLines { over_s: 30.0 });
        assert_eq!(live.clock().not_yet(false), None);
        assert_eq!(live.due(), None);
    }
}
