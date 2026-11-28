//! Audio input without a screen: the sound input devices and what they
//! offer, capture from one of them, a file replayed at its own speed, and
//! the level of a block of samples. The desktop app is one user of this;
//! the CLI or an agent setting up a microphone can make the same calls.
//!
//! Device access needs the `capture` feature, which links the platform's
//! audio library (ALSA on Linux, CoreAudio, WASAPI); file replay and level
//! checks work without it.

use crate::audio;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Validate an acquisition length without looking up or opening a device.
pub fn checked_duration(seconds: f64) -> Result<Duration, String> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err("must be a finite positive number of seconds".into());
    }
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| "is outside the clock's representable duration range".to_string())?;
    if duration.is_zero() {
        return Err("is too short to represent as a duration".into());
    }
    Ok(duration)
}

/// The buffered microphone check allows two seconds for delivery. Check
/// both the grace-period addition and the platform's monotonic clock.
pub fn checked_recording_deadline(seconds: f64) -> Result<Instant, String> {
    recording_deadline_from(Instant::now(), checked_duration(seconds)?)
}

fn recording_deadline_from(now: Instant, duration: Duration) -> Result<Instant, String> {
    let duration = duration
        .checked_add(Duration::from_secs(2))
        .ok_or("with its delivery grace period exceeds the clock's duration range")?;
    now.checked_add(duration)
        .ok_or_else(|| "with its delivery grace period exceeds the clock's deadline range".into())
}

/// Convert a length at the selected device/file rate, without saturating
/// a float-to-integer cast or silently accepting a zero-sample recording.
pub fn checked_sample_count(seconds: f64, sample_rate: u32) -> Result<usize, String> {
    checked_duration(seconds)?;
    if sample_rate == 0 {
        return Err("the input sample rate must be positive".into());
    }
    let samples = (seconds * f64::from(sample_rate)).round();
    if !samples.is_finite() || samples >= (usize::MAX as u128 + 1) as f64 {
        return Err(format!(
            "exceeds the sample-count range at {sample_rate} Hz"
        ));
    }
    if samples < 1.0 {
        return Err(format!("is shorter than one sample at {sample_rate} Hz"));
    }
    Ok(samples as usize)
}

#[cfg(any(feature = "capture", test))]
fn checked_buffer_capacity(samples: usize) -> Result<(), String> {
    if samples > isize::MAX as usize / std::mem::size_of::<f32>() {
        return Err("exceeds the buffered recording's byte-capacity range".into());
    }
    Ok(())
}

/// A block of mono samples in [-1, 1] and the system time it arrived.
#[derive(Debug, Clone)]
pub struct Block {
    pub samples: Vec<f32>,
    pub at: SystemTime,
}

#[derive(Debug)]
pub enum Event {
    Audio(Block),
    /// A live input dropped this many frames of sound just before the next
    /// block, because the queue ([`QUEUE_BLOCKS`]) was full: nothing took
    /// its blocks for a while. The stretch is gone, so whatever reads on
    /// treats it as a break and never splices the sound either side.
    Lost(u64),
    /// A live input glitched: its sound card or system reported a buffer
    /// underrun or overrun (cpal's `Xrun`), so a moment of sound may be
    /// missing here, of a length not known, and the input carries on.
    /// Whatever reads on treats it as a break, as [`Event::Lost`] (#1046).
    Glitch,
    /// A replayed file has reached its end.
    End,
    Error(String),
}

/// How many blocks a capture's queue holds before an input has to drop
/// sound (a live input, see [`Event::Lost`]) or wait (a replayed file). At
/// the 10 ms blocks inputs commonly send that is two minutes of sound, or
/// about 23 MB at 48 kHz, so memory stays bounded however long nothing
/// takes the blocks (#483).
pub const QUEUE_BLOCKS: usize = 12_000;

/// Why a reading that can't hold a gap stops at [`Event::Lost`].
pub const LOST_TEXT: &str = "the input dropped sound because nothing took it in time";

/// Why a reading that can't hold a gap stops at [`Event::Glitch`].
pub const GLITCH_TEXT: &str =
    "the computer fell behind reading this input for a moment, and some sound may be lost";

/// How many seconds of sound a live input may send ahead of the wall
/// clock before it is refused: room for a sound card's first buffer, and
/// for a burst after the computer was slow to call for the sound (#2379).
pub const FAST_INPUT_AHEAD_S: f64 = 2.0;

/// The share of the time listened that a live input may run ahead, beyond
/// [`FAST_INPUT_AHEAD_S`]: 1,000 parts per million, ten times a cheap sound
/// card's clock error, so a sound card's clock is never mistaken for a
/// fast input over a take of days (#2379).
pub const FAST_INPUT_AHEAD_SHARE: f64 = 0.001;

/// Why a live input is refused when it sends sound faster than real time,
/// as a null input does that makes up silence as fast as it is asked
/// (#2379).
pub const FAST_TEXT: &str = "this input sends sound faster than real time, so a watch can't be \
     timed from it. Choose another input";

/// The sending side of a live input's queue. A block that doesn't fit is
/// dropped and counted, and the count goes as [`Event::Lost`] ahead of the
/// next block that fits, so the loss is known where it fell. An input that
/// sends sound faster than real time ([`FAST_INPUT_AHEAD_S`]) is refused:
/// [`Event::Error`] with [`FAST_TEXT`] goes once, and nothing after it.
pub struct LiveQueue {
    tx: SyncSender<Event>,
    lost: u64,
    /// The input's frames per second.
    rate: u32,
    /// When the first block arrived, and the frames sent since then,
    /// counted with that first block.
    first: Option<Instant>,
    frames: u64,
    /// Refused as too fast; and whether the refusal has gone.
    fast: bool,
    told: bool,
}

impl LiveQueue {
    pub fn new(tx: SyncSender<Event>, rate: u32) -> Self {
        LiveQueue {
            tx,
            lost: 0,
            rate,
            first: None,
            frames: 0,
            fast: false,
            told: false,
        }
    }

    /// Queue `b`, or drop it and count its frames when the queue is full.
    /// Never waits, as an audio callback mustn't.
    pub fn send(&mut self, b: Block) {
        self.send_at(b, Instant::now());
    }

    /// [`LiveQueue::send`], for a block arriving at `now`.
    fn send_at(&mut self, b: Block, now: Instant) {
        let frames = b.samples.len() as u64;
        if !self.fast {
            self.fast = self.runs_ahead(frames, now);
        }
        if self.fast {
            if !self.told {
                self.told = self.tx.try_send(Event::Error(FAST_TEXT.into())).is_ok();
            }
            return;
        }
        if self.lost > 0 {
            match self.tx.try_send(Event::Lost(self.lost)) {
                Ok(()) => self.lost = 0,
                Err(TrySendError::Full(_)) => {
                    self.lost += frames;
                    return;
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
        if let Err(TrySendError::Full(_)) = self.tx.try_send(Event::Audio(b)) {
            self.lost += frames;
        }
    }

    /// Whether, with `frames` more arriving at `now`, the input has sent
    /// more sound than the wall clock allows since its first block.
    fn runs_ahead(&mut self, frames: u64, now: Instant) -> bool {
        let first = *self.first.get_or_insert(now);
        self.frames += frames;
        let listened = now.saturating_duration_since(first).as_secs_f64();
        let sent = self.frames as f64 / f64::from(self.rate.max(1));
        sent - listened > FAST_INPUT_AHEAD_S + listened * FAST_INPUT_AHEAD_SHARE
    }
}

/// A running source of audio. Dropping it stops the source.
pub struct Capture {
    pub rx: Receiver<Event>,
    pub sample_rate: u32,
    /// Bits per sample the source delivers (16 for the usual USB
    /// timegrapher microphones), for the format of a saved recording.
    pub bits: u16,
    /// Device name or file name.
    pub label: String,
    pub is_file: bool,
    stop: Arc<AtomicBool>,
    /// A replayed file's speed, times its own, as f32 bits.
    speed: Arc<AtomicU32>,
    #[cfg(feature = "capture")]
    _stream: Option<device::Held>,
}

impl Capture {
    /// Play a replayed file faster or slower than its own speed (1.0),
    /// from the next block on. No effect on a device.
    pub fn set_speed(&self, speed: f32) {
        self.speed
            .store(speed.clamp(0.1, 100.0).to_bits(), Ordering::Relaxed);
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Peak and RMS of some audio, and how much of it is clipped.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Level {
    pub samples: usize,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    /// Fraction of samples at or within 0.1% of full scale.
    pub clipped_fraction: f64,
}

impl Level {
    pub fn of(x: &[f32]) -> Level {
        if x.is_empty() {
            return Level {
                peak_dbfs: f64::NEG_INFINITY,
                rms_dbfs: f64::NEG_INFINITY,
                ..Default::default()
            };
        }
        let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs())) as f64;
        let ms = x.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / x.len() as f64;
        let clipped = x.iter().filter(|v| v.abs() >= 0.999).count();
        Level {
            samples: x.len(),
            peak_dbfs: 20.0 * peak.max(1e-12).log10(),
            rms_dbfs: 10.0 * ms.max(1e-24).log10(),
            clipped_fraction: clipped as f64 / x.len() as f64,
        }
    }
}

/// Replay a WAV or FLAC file at its own speed, as if the watch were on the
/// microphone; [`Capture::set_speed`] plays it faster or slower.
pub fn replay_file(path: &Path) -> Result<Capture, String> {
    let label = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    replay_files(&[path.to_path_buf()], label)
}

/// Replay several files one after the other as one recording, as a take
/// saved in parts was heard: they must share a sample rate.
pub fn replay_files(paths: &[PathBuf], label: String) -> Result<Capture, String> {
    let first = paths.first().ok_or("No recordings to replay")?;
    // Each error names its file when there are several.
    let named = |p: &Path, e: String| match paths.len() {
        1 => e,
        _ => format!("{}: {e}", p.display()),
    };
    let info = audio::info(first).map_err(|e| named(first, e.to_string()))?;
    let fs = info.sample_rate;
    for p in &paths[1..] {
        let i = audio::info(p).map_err(|e| named(p, e.to_string()))?;
        if i.sample_rate != fs {
            return Err(format!(
                "{} is at {} Hz but {} is at {fs} Hz",
                p.display(),
                i.sample_rate,
                first.display()
            ));
        }
    }
    // A file waits for room rather than dropping sound.
    let (tx, rx) = sync_channel(QUEUE_BLOCKS);
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let speed = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let speed2 = speed.clone();
    let paths = paths.to_vec();
    std::thread::spawn(move || replay(&paths, fs, &tx, &stop2, &speed2));
    Ok(Capture {
        rx,
        sample_rate: fs,
        bits: (info.bytes_per_frame.max(2) * 8).min(24) as u16,
        label,
        is_file: true,
        stop,
        speed,
        #[cfg(feature = "capture")]
        _stream: None,
    })
}

/// Send the sound of `paths` to `tx` at its own speed times `speed`, then
/// [`Event::End`]; or an [`Event::Error`] naming the file that could not
/// be read. Reading stops within the block in hand once `stop` is set or
/// nothing takes the blocks any more (the [`Capture`] dropped), so a
/// stopped replay does not read on to the end of its file (#2414).
/// Returns how many blocks were read.
fn replay(
    paths: &[PathBuf],
    fs: u32,
    tx: &SyncSender<Event>,
    stop: &AtomicBool,
    speed: &AtomicU32,
) -> usize {
    let several = paths.len() > 1;
    let start = Instant::now();
    // When the sound sent so far is due, at the speed each block went
    // out at.
    let mut due = Duration::ZERO;
    let mut read = 0;
    for p in paths {
        let r = audio::stream_while(p, (fs / 50).max(1) as usize, |b| {
            read += 1;
            if stop.load(Ordering::Relaxed) {
                return false;
            }
            let x = f32::from_bits(speed.load(Ordering::Relaxed)) as f64;
            due += Duration::from_secs_f64(b.len() as f64 / fs as f64 / x);
            let block = Block {
                samples: b.to_vec(),
                at: SystemTime::now(),
            };
            if tx.send(Event::Audio(block)).is_err() {
                return false;
            }
            if let Some(wait) = due.checked_sub(start.elapsed()) {
                std::thread::sleep(wait);
            }
            !stop.load(Ordering::Relaxed)
        });
        if let Err(e) = r {
            let _ = tx.send(Event::Error(if several {
                format!("{}: {e}", p.display())
            } else {
                e.to_string()
            }));
            return read;
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
    }
    let _ = tx.send(Event::End);
    read
}

/// A range of formats a device offers.
#[derive(Debug, Clone, Serialize)]
pub struct InputConfig {
    pub min_channels: u16,
    pub max_channels: u16,
    pub min_sample_rate: u32,
    pub max_sample_rate: u32,
    pub sample_format: String,
}

/// A sound input device and the formats it offers.
#[derive(Debug, Clone, Serialize)]
pub struct InputDevice {
    /// Stable identifier; it, or part of the name, selects the device.
    /// It is cpal's device id, `<host>:<device>`; on Linux the device part
    /// is the ALSA PCM name, as in `alsa:hw:CARD=Device,DEV=0`, so the
    /// sound card (for its mixer) can be read from it.
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    pub is_default: bool,
    /// The device's own default configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_config: Option<InputConfig>,
    pub supported: Vec<InputConfig>,
}

/// What a short recording was made with.
#[derive(Debug, Clone, Serialize)]
pub struct Recorded {
    pub device: InputDevice,
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: String,
    pub started_utc: String,
}

/// The sample rate asked of a device when it offers it. USB timegrapher
/// microphones commonly do 44.1 or 48 kHz; the recordings so far are 48 kHz.
pub const PREFERRED_RATE: u32 = 48000;

#[cfg_attr(not(feature = "capture"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Match {
    Exact,
    Partial,
}

/// How a device query matches an input, case-insensitively: exactly (the
/// whole id, the id without its `host:` prefix, or the whole name), as part
/// of the id or name, or not at all.
#[cfg_attr(not(feature = "capture"), allow(dead_code))]
fn matches(id: &str, name: &str, query: &str) -> Option<Match> {
    let (id, name, q) = (id.to_lowercase(), name.to_lowercase(), query.to_lowercase());
    let bare = id.split_once(':').map_or(id.as_str(), |(_, rest)| rest);
    if id == q || bare == q || name == q {
        Some(Match::Exact)
    } else if id.contains(&q) || name.contains(&q) {
        Some(Match::Partial)
    } else {
        None
    }
}

/// One entry of a device menu for a person to choose from.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InputChoice {
    /// The device id to open (as in [`InputDevice::id`]).
    pub id: String,
    pub label: String,
    /// One or two sentences on when to use it.
    pub detail: String,
    pub is_default: bool,
}

/// The inputs worth offering a person, from a device list (see `list`).
///
/// On Linux, ALSA lists every card several times over (front, dsnoop,
/// sysdefault, surround, null and so on) and names the sound server's
/// default input "Default ALSA Output". This keeps the system default
/// (normally the desktop sound server, which shares the microphone with
/// other programs) and each card's direct `hw:` device, labelled as such.
/// Other systems' lists are kept as they are, the default first.
pub fn choices(devices: &[InputDevice]) -> Vec<InputChoice> {
    let alsa: Vec<&InputDevice> = devices
        .iter()
        .filter(|d| d.id.starts_with("alsa:"))
        .collect();
    if alsa.is_empty() {
        let mut out: Vec<InputChoice> = devices
            .iter()
            .map(|d| InputChoice {
                id: d.id.clone(),
                label: if d.is_default {
                    format!("{} (default)", d.name)
                } else {
                    d.name.clone()
                },
                detail: d.manufacturer.clone().unwrap_or_default(),
                is_default: d.is_default,
            })
            .collect();
        out.sort_by_key(|c| !c.is_default);
        return out;
    }
    let server = |name: &str| {
        let n = name.to_lowercase();
        if n.contains("pipewire") {
            Some("PipeWire")
        } else if n.contains("pulse") {
            Some("PulseAudio")
        } else {
            None
        }
    };
    let shared = "Whichever input the desktop's sound settings choose, shared with other \
                  programs. Use this while the desktop sound server is running.";
    let mut out = Vec::new();
    let mut have_default = false;
    for d in &alsa {
        if &d.id[5..] == "default" {
            have_default = true;
            out.push(InputChoice {
                id: d.id.clone(),
                label: match server(&d.name) {
                    Some(s) => format!("System default (via {s})"),
                    None => "System default".into(),
                },
                detail: shared.into(),
                is_default: true,
            });
        }
    }
    for d in &alsa {
        let bare = &d.id[5..];
        if (bare == "pipewire" || bare == "pulse") && !have_default {
            out.push(InputChoice {
                id: d.id.clone(),
                label: format!(
                    "{} sound server",
                    if bare == "pipewire" {
                        "PipeWire"
                    } else {
                        "PulseAudio"
                    }
                ),
                detail: shared.into(),
                is_default: false,
            });
        }
    }
    let direct = direct_devices(&alsa);
    for d in &direct {
        // The device named where its card has several of one name.
        let card_of = |id: &str| hw_parts(id).map(|p| p.0);
        let twins = direct
            .iter()
            .filter(|o| o.name == d.name && card_of(&o.id) == card_of(&d.id))
            .count();
        let dev = d.id.rsplit("DEV=").next().filter(|_| twins > 1);
        // Two cards of one name (two of the same USB adapter) with the
        // same device: told apart by the card.
        let same = direct
            .iter()
            .filter(|o| {
                o.name == d.name && hw_parts(&o.id).map(|p| p.1) == hw_parts(&d.id).map(|p| p.1)
            })
            .count();
        let card = hw_parts(&d.id).map(|p| p.0).filter(|_| same > 1);
        out.push(InputChoice {
            id: d.id.clone(),
            label: match (card, dev) {
                (Some(c), Some(n)) => format!("{} #{n} ({c}, direct)", d.name),
                (Some(c), None) => format!("{} ({c}, direct)", d.name),
                (None, Some(n)) => format!("{} #{n} (direct)", d.name),
                (None, None) => format!("{} (direct)", d.name),
            },
            detail: "Straight to the sound card, bypassing the desktop sound server. \
                     It can't open while the sound server holds the card; then choose \
                     the system default."
                .into(),
            is_default: false,
        });
    }
    out
}

/// The (card, device) of a `hw:CARD=..,DEV=..` id.
fn hw_parts(id: &str) -> Option<(String, String)> {
    let rest = id.strip_prefix("alsa:hw:")?;
    let mut card = None;
    let mut dev = None;
    for kv in rest.split(',') {
        match kv.split_once('=') {
            Some(("CARD", v)) => card = Some(v.to_string()),
            Some(("DEV", v)) => dev = Some(v.to_string()),
            _ => {}
        }
    }
    Some((card?, dev?))
}

/// Each card's direct `hw:` inputs, one per device. ALSA can list the same
/// device twice, once by the card's name (`hw:CARD=PCH,DEV=0`) and once by
/// its number (`hw:CARD=0,DEV=0`). This keeps the named one, which stays
/// the same when cards are numbered differently after a restart. Only a
/// named entry and a numbered one, of the same device name and `DEV=`,
/// are taken for the same device, each paired once: two named cards (or
/// two numbered) are two cards, such as two of the same USB adapter
/// (`CARD=Device` and `CARD=Device_1`), and both are kept.
fn direct_devices<'a>(alsa: &[&'a InputDevice]) -> Vec<&'a InputDevice> {
    let numbered =
        |id: &str| hw_parts(id).is_some_and(|(card, _)| card.chars().all(|c| c.is_ascii_digit()));
    // Each kept entry, and whether a twin of the other kind has joined it.
    let mut out: Vec<(&InputDevice, bool)> = Vec::new();
    for d in alsa
        .iter()
        .copied()
        .filter(|d| d.id[5..].starts_with("hw:"))
    {
        let dev = hw_parts(&d.id).map(|p| p.1);
        let twin = out.iter().position(|(o, paired)| {
            !paired
                && o.name == d.name
                && dev.is_some()
                && hw_parts(&o.id).map(|p| p.1) == dev
                && numbered(&o.id) != numbered(&d.id)
        });
        match twin {
            // The named one is kept, whichever came first.
            Some(i) if numbered(&out[i].0.id) => out[i] = (d, true),
            Some(i) => out[i].1 = true,
            None => out.push((d, false)),
        }
    }
    out.into_iter().map(|(d, _)| d).collect()
}

/// How [`find`]'s error starts when there is no default input to open.
const NO_DEFAULT_INPUT: &str = "no default sound input";

/// How [`find`]'s error starts when no input matches the name asked for.
const NO_MATCHING_INPUT: &str = "no sound input matches";

/// cpal's `DeviceNotAvailable` error, as its `Display` words it: the
/// input asked for is not there (on a computer with no sound card, #2103).
const CPAL_DEVICE_NOT_AVAILABLE: &str = "audio device is not available";

/// cpal's error for an input unplugged while open, which
/// [`explain_error`] also reads as a missing input.
const CPAL_DEVICE_UNPLUGGED: &str = "no longer available";

/// The words that mark an error as there being no sound input to open:
/// [`find`]'s two errors, built from the same constants, and cpal's for
/// an input that is not there or was unplugged. A busy input is not
/// among them: it is there, only held by another program.
const NO_INPUT_PHRASES: [&str; 4] = [
    CPAL_DEVICE_NOT_AVAILABLE,
    NO_DEFAULT_INPUT,
    NO_MATCHING_INPUT,
    CPAL_DEVICE_UNPLUGGED,
];

/// [`find`]'s error when there is no default input.
#[cfg(any(test, feature = "capture"))]
fn no_default_input_error() -> String {
    format!("{NO_DEFAULT_INPUT}; name one (see the list of devices)")
}

/// [`find`]'s error when no input matches `query`.
#[cfg(any(test, feature = "capture"))]
fn no_matching_input_error(query: &str) -> String {
    format!("{NO_MATCHING_INPUT} '{query}'")
}

/// Whether an error from opening or finding an input, `error`, says there
/// was no sound input to open, so that a surface can point at the list of
/// inputs (#2103). The phrases are core's own and cpal's, kept here beside
/// the messages they match so that rewording one cannot silently stop it.
pub fn no_input(error: &str) -> bool {
    let lower = error.to_lowercase();
    NO_INPUT_PHRASES.iter().any(|p| lower.contains(p))
}

/// A plainer account of an error from opening or reading an input.
pub fn explain_error(e: &str) -> String {
    let l = e.to_lowercase();
    if e == FAST_TEXT {
        // Said after "Input stopped.", so as a sentence of its own.
        let mut c = e.chars();
        c.next()
            .map(|f| f.to_uppercase().chain(c).collect())
            .unwrap_or_default()
    } else if l.contains("no longer available")
        || l.contains("unplugged")
        || l.contains("busy")
        || l.contains("not available")
    {
        format!(
            "This input is busy or missing. If the desktop sound server is running it \
             is probably holding it: choose the system default instead, or close what \
             is using it. Otherwise check the microphone is plugged in. ({e})"
        )
    } else if l.contains("underrun") || l.contains("overrun") {
        format!("The computer fell behind reading this input, and sound was lost. ({e})")
    } else if l.contains("alsa") || l.contains("htstamp") {
        format!(
            "The sound card refused this input. Choose the system default, then pick \
             the jack in the desktop's sound settings. ({e})"
        )
    } else {
        e.to_string()
    }
}

#[cfg(feature = "capture")]
pub use device::{find, host, list, open, open_for_duration, open_input, play, record, Playback};

#[cfg(feature = "capture")]
mod device {
    use super::*;
    use crate::audio::Audio;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SampleFormat, SizedSample};
    use std::sync::mpsc::channel;

    fn describe(dev: &cpal::Device, default_id: Option<&str>) -> InputDevice {
        let id = dev.id().map(|i| i.to_string()).unwrap_or_default();
        let desc = dev.description().ok();
        let supported = dev
            .supported_input_configs()
            .map(|it| {
                merge(it.map(|c| InputConfig {
                    min_channels: c.channels(),
                    max_channels: c.channels(),
                    min_sample_rate: c.min_sample_rate(),
                    max_sample_rate: c.max_sample_rate(),
                    sample_format: c.sample_format().to_string(),
                }))
            })
            .unwrap_or_default();
        let default_config = dev.default_input_config().ok().map(|c| InputConfig {
            min_channels: c.channels(),
            max_channels: c.channels(),
            min_sample_rate: c.sample_rate(),
            max_sample_rate: c.sample_rate(),
            sample_format: c.sample_format().to_string(),
        });
        InputDevice {
            is_default: default_id == Some(id.as_str()),
            name: desc
                .as_ref()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|| id.clone()),
            manufacturer: desc.and_then(|d| d.manufacturer().map(str::to_string)),
            id,
            default_config,
            supported,
        }
    }

    /// Fold configurations that differ only by channel count into ranges:
    /// virtual devices list every count from 1 to 32 for every format.
    fn merge(configs: impl Iterator<Item = InputConfig>) -> Vec<InputConfig> {
        let mut out: Vec<InputConfig> = Vec::new();
        for c in configs {
            match out.iter_mut().find(|o| {
                o.sample_format == c.sample_format
                    && o.min_sample_rate == c.min_sample_rate
                    && o.max_sample_rate == c.max_sample_rate
            }) {
                Some(o) => {
                    o.min_channels = o.min_channels.min(c.min_channels);
                    o.max_channels = o.max_channels.max(c.max_channels);
                }
                None => out.push(c),
            }
        }
        out
    }

    fn default_id(host: &cpal::Host) -> Option<String> {
        host.default_input_device()
            .and_then(|d| d.id().ok())
            .map(|i| i.to_string())
    }

    /// The audio host in use (ALSA, CoreAudio, WASAPI, ...).
    pub fn host() -> String {
        format!("{:?}", cpal::default_host().id())
    }

    /// Every input the default audio host offers, the default first.
    pub fn list() -> Result<Vec<InputDevice>, String> {
        let host = cpal::default_host();
        let def = default_id(&host);
        let devs = host.input_devices().map_err(|e| e.to_string())?;
        let mut out: Vec<InputDevice> = devs.map(|d| describe(&d, def.as_deref())).collect();
        out.sort_by_key(|d| !d.is_default);
        Ok(out)
    }

    /// The input whose id or name is, or else uniquely contains, `query`
    /// (case-insensitive), or the default input when `query` is `None`.
    /// The id may be given with or without its host prefix, so
    /// `hw:CARD=Device,DEV=0` picks `alsa:hw:CARD=Device,DEV=0` over
    /// `alsa:plughw:CARD=Device,DEV=0`.
    pub fn find(query: Option<&str>) -> Result<(cpal::Device, InputDevice), String> {
        let host = cpal::default_host();
        let def = default_id(&host);
        let Some(q) = query else {
            let d = host
                .default_input_device()
                .ok_or_else(super::no_default_input_error)?;
            let info = describe(&d, def.as_deref());
            return Ok((d, info));
        };
        let mut hits = Vec::new();
        for d in host.input_devices().map_err(|e| e.to_string())? {
            let info = describe(&d, def.as_deref());
            match super::matches(&info.id, &info.name, q) {
                Some(super::Match::Exact) => return Ok((d, info)),
                Some(super::Match::Partial) => hits.push((d, info)),
                None => {}
            }
        }
        match hits.len() {
            1 => Ok(hits.pop().expect("one hit")),
            0 => Err(super::no_matching_input_error(q)),
            _ => Err(format!(
                "'{q}' matches several inputs: {}; give the whole id",
                hits.iter()
                    .map(|h| h.1.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// The configuration used for a device: `rate` (48 kHz by default) if
    /// it offers it, in as few channels as possible, 16-bit if it has it;
    /// otherwise its own default.
    fn pick(dev: &cpal::Device, rate: u32) -> Result<cpal::SupportedStreamConfig, String> {
        let ranges: Vec<_> = dev
            .supported_input_configs()
            .map_err(|e| e.to_string())?
            .collect();
        let best = ranges
            .iter()
            .filter(|r| r.min_sample_rate() <= rate && r.max_sample_rate() >= rate)
            .min_by_key(|r| (r.channels(), format_rank(r.sample_format())))
            .map(|r| r.with_sample_rate(rate));
        match best {
            Some(c) => Ok(c),
            None => dev.default_input_config().map_err(|e| e.to_string()),
        }
    }

    /// Prefer 16-bit integer, the native format of most timegrapher microphones.
    fn format_rank(f: SampleFormat) -> u8 {
        match f {
            SampleFormat::I16 => 0,
            SampleFormat::I32 => 1,
            SampleFormat::F32 => 2,
            _ => 3,
        }
    }

    /// Start capturing from an input found by `query` (see [`find`]).
    pub fn open_input(query: Option<&str>, rate: Option<u32>) -> Result<Capture, String> {
        let (dev, info) = find(query)?;
        open(&dev, &info, rate).map(|(c, _)| c)
    }

    /// Start capturing from `dev`, mixed down to mono, as blocks on a channel.
    pub fn open(
        dev: &cpal::Device,
        info: &InputDevice,
        rate: Option<u32>,
    ) -> Result<(Capture, Recorded), String> {
        let supported = pick(dev, rate.unwrap_or(PREFERRED_RATE))?;
        open_config(dev, info, supported)
    }

    /// Query the actual format and check its requested sample count before
    /// opening the input stream. Device-independent validation runs first.
    pub fn open_for_duration(
        dev: &cpal::Device,
        info: &InputDevice,
        rate: Option<u32>,
        seconds: f64,
    ) -> Result<(Capture, Recorded, usize), String> {
        checked_duration(seconds)?;
        let supported = pick(dev, rate.unwrap_or(PREFERRED_RATE))?;
        let want = checked_sample_count(seconds, supported.config().sample_rate)?;
        let (capture, recorded) = open_config(dev, info, supported)?;
        Ok((capture, recorded, want))
    }

    fn open_config(
        dev: &cpal::Device,
        info: &InputDevice,
        supported: cpal::SupportedStreamConfig,
    ) -> Result<(Capture, Recorded), String> {
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.config();
        let (tx, rx) = sync_channel(QUEUE_BLOCKS);
        let (dev, cfg) = (dev.clone(), config);
        let stream = Held::start(move || {
            match format {
                SampleFormat::I16 => build::<i16>(&dev, &cfg, tx),
                SampleFormat::U16 => build::<u16>(&dev, &cfg, tx),
                SampleFormat::I32 => build::<i32>(&dev, &cfg, tx),
                SampleFormat::F32 => build::<f32>(&dev, &cfg, tx),
                SampleFormat::I8 => build::<i8>(&dev, &cfg, tx),
                SampleFormat::U8 => build::<u8>(&dev, &cfg, tx),
                SampleFormat::F64 => build::<f64>(&dev, &cfg, tx),
                other => return Err(format!("unsupported sample format {other}")),
            }
            .map_err(|e| e.to_string())
        })?;
        let rec = Recorded {
            device: info.clone(),
            sample_rate: config.sample_rate,
            channels: config.channels,
            sample_format: format.to_string(),
            started_utc: crate::recorder::utc(SystemTime::now()),
        };
        Ok((
            Capture {
                rx,
                sample_rate: config.sample_rate,
                bits: if format.sample_size() <= 2 { 16 } else { 24 },
                label: info.name.clone(),
                is_file: false,
                stop: Arc::new(AtomicBool::new(false)),
                speed: Arc::new(AtomicU32::new(1.0f32.to_bits())),
                _stream: Some(stream),
            },
            rec,
        ))
    }

    /// Record `seconds` from `dev` (configured as for [`open`]), mixed to mono.
    pub fn record(
        dev: &cpal::Device,
        info: InputDevice,
        seconds: f64,
    ) -> Result<(Audio, Recorded), String> {
        checked_recording_deadline(seconds)?;
        let supported = pick(dev, PREFERRED_RATE)?;
        let want = checked_sample_count(seconds, supported.config().sample_rate)?;
        checked_buffer_capacity(want)?;
        let mut samples = Vec::new();
        samples
            .try_reserve_exact(want)
            .map_err(|e| format!("could not reserve the recording buffer: {e}"))?;
        let (cap, rec) = open_config(dev, &info, supported)?;
        let deadline = checked_recording_deadline(seconds)?;
        while samples.len() < want {
            let left = deadline.saturating_duration_since(Instant::now());
            match cap.rx.recv_timeout(left) {
                Ok(Event::Audio(b)) => {
                    let take = b.samples.len().min(want - samples.len());
                    samples.extend_from_slice(&b.samples[..take]);
                }
                Ok(Event::Error(e)) => return Err(format!("recording failed: {e}")),
                // One stretch of samples can't hold a gap.
                Ok(Event::Lost(_)) => return Err(LOST_TEXT.into()),
                Ok(Event::Glitch) => return Err(GLITCH_TEXT.into()),
                Ok(Event::End) => break,
                Err(_) => return Err("the input stopped delivering audio".into()),
            }
        }
        samples.truncate(want);
        Ok((
            Audio {
                samples,
                sample_rate: rec.sample_rate,
            },
            rec,
        ))
    }

    /// A stream kept on a thread of its own, so that what holds it can move
    /// between threads, which some systems' streams can't. Dropping it
    /// stops the stream and waits until it has.
    pub struct Held {
        stop: Option<std::sync::mpsc::Sender<()>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Held {
        /// Build a stream with `make` on a new thread and start it.
        pub(crate) fn start(
            make: impl FnOnce() -> Result<cpal::Stream, String> + Send + 'static,
        ) -> Result<Held, String> {
            let (ready_tx, ready_rx) = channel();
            let (stop, stop_rx) = channel::<()>();
            let thread = std::thread::Builder::new()
                .name("audio stream".into())
                .spawn(move || {
                    let stream = make().and_then(|s| {
                        s.play().map_err(|e| e.to_string())?;
                        Ok(s)
                    });
                    match stream {
                        Ok(s) => {
                            let _ = ready_tx.send(Ok(()));
                            // Until the holder lets go.
                            let _ = stop_rx.recv();
                            drop(s);
                        }
                        Err(e) => {
                            let _ = ready_tx.send(Err(e));
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            let ready = ready_rx
                .recv()
                .map_err(|_| "the audio stream's thread stopped".to_string())
                .and_then(|r| r);
            let held = Held {
                stop: Some(stop),
                thread: Some(thread),
            };
            ready.map(|()| held)
        }
    }

    impl Drop for Held {
        fn drop(&mut self) {
            drop(self.stop.take());
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    /// Sound playing on the default output. Dropping it stops the sound.
    pub struct Playback {
        /// Samples of the source played so far.
        played: Arc<std::sync::atomic::AtomicUsize>,
        len: usize,
        rate: u32,
        _stream: Held,
    }

    impl Playback {
        /// How far through the sound it has played, seconds.
        pub fn position_s(&self) -> f64 {
            self.played.load(Ordering::Relaxed).min(self.len) as f64 / self.rate as f64
        }

        pub fn finished(&self) -> bool {
            self.played.load(Ordering::Relaxed) >= self.len
        }
    }

    /// Play mono `samples` at `rate` on the default output, on every channel,
    /// resampled if the output can't run at `rate`.
    pub fn play(samples: Arc<Vec<f32>>, rate: u32) -> Result<Playback, String> {
        let dev = cpal::default_host()
            .default_output_device()
            .ok_or("no sound output found")?;
        let supported = dev
            .supported_output_configs()
            .map_err(|e| e.to_string())?
            .filter(|c| c.min_sample_rate() <= rate && rate <= c.max_sample_rate())
            .max_by_key(|c| (c.sample_format() == SampleFormat::F32, c.channels() <= 2))
            .map(|c| c.with_sample_rate(rate));
        let supported = match supported {
            Some(c) => c,
            None => dev.default_output_config().map_err(|e| e.to_string())?,
        };
        let config: cpal::StreamConfig = supported.config();
        let played = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let feed = Feed {
            samples: samples.clone(),
            pos: 0.0,
            step: rate as f64 / config.sample_rate as f64,
            channels: config.channels.max(1) as usize,
            played: played.clone(),
        };
        let format = supported.sample_format();
        let cfg = config;
        let stream = Held::start(move || {
            match format {
                SampleFormat::F32 => build_output::<f32>(&dev, &cfg, feed),
                SampleFormat::I16 => build_output::<i16>(&dev, &cfg, feed),
                SampleFormat::U16 => build_output::<u16>(&dev, &cfg, feed),
                SampleFormat::I32 => build_output::<i32>(&dev, &cfg, feed),
                SampleFormat::F64 => build_output::<f64>(&dev, &cfg, feed),
                other => return Err(format!("unsupported output format {other}")),
            }
            .map_err(|e| e.to_string())
        })?;
        Ok(Playback {
            played,
            len: samples.len(),
            rate,
            _stream: stream,
        })
    }

    /// The source as the output callback reads it.
    struct Feed {
        samples: Arc<Vec<f32>>,
        /// Position in source samples.
        pos: f64,
        /// Source samples per output frame.
        step: f64,
        channels: usize,
        played: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Feed {
        fn next(&mut self) -> f32 {
            let i = self.pos as usize;
            let s = match (self.samples.get(i), self.samples.get(i + 1)) {
                (Some(&a), Some(&b)) => {
                    let f = (self.pos - i as f64) as f32;
                    a + (b - a) * f
                }
                (Some(&a), None) => a,
                _ => 0.0,
            };
            self.pos += self.step;
            s
        }
    }

    fn build_output<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mut feed: Feed,
    ) -> Result<cpal::Stream, cpal::Error>
    where
        T: SizedSample + FromSample<f32>,
    {
        device.build_output_stream(
            *config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let ch = feed.channels;
                for frame in data.chunks_mut(ch) {
                    let s = T::from_sample(feed.next());
                    frame.iter_mut().for_each(|o| *o = s);
                }
                feed.played.store(
                    (feed.pos as usize).min(feed.samples.len()),
                    Ordering::Relaxed,
                );
            },
            |_| {},
            None,
        )
    }

    /// What an error from a running input becomes: a glitch (cpal's
    /// `Xrun`, after which the stream carries on) or a stop.
    fn event_of(e: &cpal::Error) -> Event {
        match e.kind() {
            cpal::ErrorKind::Xrun => Event::Glitch,
            _ => Event::Error(e.to_string()),
        }
    }

    fn build<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        tx: SyncSender<Event>,
    ) -> Result<cpal::Stream, cpal::Error>
    where
        T: SizedSample,
        f32: FromSample<T>,
    {
        let ch = config.channels.max(1) as usize;
        let err_tx = tx.clone();
        let mut queue = LiveQueue::new(tx, config.sample_rate);
        device.build_input_stream(
            *config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let samples: Vec<f32> = data
                    .chunks(ch)
                    .map(|f| {
                        f.iter()
                            .map(|&s| <f32 as FromSample<T>>::from_sample_(s))
                            .sum::<f32>()
                            / ch as f32
                    })
                    .collect();
                queue.send(Block {
                    samples,
                    at: SystemTime::now(),
                });
            },
            move |e: cpal::Error| {
                // Never waits: an error that doesn't fit is dropped, and
                // the input then reads as sending no sound. A glitch is a
                // gap, not a stop: the stream carries on after it (#1046).
                let _ = err_tx.try_send(event_of(&e));
            },
            None,
        )
    }

    #[cfg(test)]
    mod event_tests {
        use super::*;

        /// A buffer underrun or overrun is a glitch the take goes on
        /// through; any other stream error stops it.
        #[test]
        fn a_glitch_carries_on_and_other_errors_stop() {
            assert!(matches!(
                event_of(&cpal::ErrorKind::Xrun.into()),
                Event::Glitch
            ));
            match event_of(&cpal::ErrorKind::DeviceNotAvailable.into()) {
                Event::Error(e) => assert!(!e.is_empty()),
                other => panic!("{other:?}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn acquisition_durations_reject_invalid_and_unrepresentable_values() {
        for seconds in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -1.0,
            f64::MAX,
            f64::MIN_POSITIVE,
        ] {
            assert!(checked_duration(seconds).is_err(), "{seconds}");
        }
        for seconds in [5.0, 120.0, 1200.0] {
            assert_eq!(checked_duration(seconds).unwrap().as_secs_f64(), seconds);
        }
    }

    #[test]
    fn acquisition_deadline_checks_grace_and_platform_clock_overflow() {
        let now = Instant::now();
        let grace_error = recording_deadline_from(now, Duration::MAX).unwrap_err();
        assert!(grace_error.contains("duration range"), "{grace_error}");
        let deadline_error =
            recording_deadline_from(now, Duration::from_secs(u64::MAX - 2)).unwrap_err();
        assert!(
            deadline_error.contains("deadline range"),
            "{deadline_error}"
        );
        assert_eq!(
            recording_deadline_from(now, Duration::from_secs(5)).unwrap(),
            now + Duration::from_secs(7)
        );
    }

    #[test]
    fn acquisition_sample_counts_check_the_actual_rate_before_casting() {
        assert_eq!(checked_sample_count(60.0, 48000).unwrap(), 2_880_000);
        assert_eq!(checked_sample_count(0.01, 100).unwrap(), 1);
        assert!(checked_sample_count(1e-9, 48000)
            .unwrap_err()
            .contains("one sample"));
        assert!(checked_sample_count(1.0, 0).is_err());
        assert!(checked_sample_count((usize::MAX as u128 + 1) as f64 / 48000.0, 48000).is_err());
    }

    #[test]
    fn acquisition_buffer_capacity_is_checked_without_allocating() {
        let maximum = isize::MAX as usize / std::mem::size_of::<f32>();
        assert!(checked_buffer_capacity(maximum).is_ok());
        assert!(checked_buffer_capacity(maximum + 1).is_err());
    }

    #[test]
    fn device_id_without_host_prefix_is_exact() {
        let m = |id| {
            matches(
                id,
                "USB PnP Sound Device, USB Audio",
                "hw:CARD=Device,DEV=0",
            )
        };
        assert_eq!(m("alsa:hw:CARD=Device,DEV=0"), Some(Match::Exact));
        assert_eq!(m("alsa:plughw:CARD=Device,DEV=0"), Some(Match::Partial));
        assert_eq!(m("alsa:hw:CARD=PCH,DEV=0"), None);
        assert_eq!(
            matches("alsa:default", "Default ALSA Output", "ALSA:DEFAULT"),
            Some(Match::Exact)
        );
        assert_eq!(
            matches("alsa:x", "USB PnP Sound Device", "usb pnp"),
            Some(Match::Partial)
        );
    }

    use super::*;

    fn dev(id: &str, name: &str, is_default: bool) -> InputDevice {
        InputDevice {
            id: id.into(),
            name: name.into(),
            manufacturer: None,
            is_default,
            default_config: None,
            supported: Vec::new(),
        }
    }

    /// A replay at 8 times its own speed takes an eighth of the time.
    #[test]
    fn replay_speed_sets_the_pace() {
        let dir = std::env::temp_dir().join(format!("tg-replay-speed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("two-seconds.wav");
        let audio = audio::Audio {
            samples: vec![0.0; 16_000],
            sample_rate: 8000,
        };
        audio::write_wav(&f, &audio).unwrap();
        let c = replay_file(&f).unwrap();
        c.set_speed(8.0);
        let start = Instant::now();
        let mut samples = 0;
        loop {
            match c.rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::Audio(b) => samples += b.samples.len(),
                Event::End => break,
                Event::Error(e) => panic!("{e}"),
                Event::Lost(n) => panic!("a file waits, but lost {n} frames"),
                Event::Glitch => panic!("a file never glitches"),
            }
        }
        let took = start.elapsed().as_secs_f64();
        assert_eq!(samples, 16_000);
        assert!(took < 1.0, "2 s at 8x took {took:.2} s");
        assert!(took > 0.15, "2 s at 8x took only {took:.2} s");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A replay stopped after its first block reads a block or two more,
    /// not the rest of the file, whether it is a WAV or a FLAC: when the
    /// capture is dropped, when nothing takes its blocks any more, and
    /// when it is told to stop with its queue still taken (#2414).
    #[test]
    fn a_stopped_replay_stops_reading() {
        let dir = std::env::temp_dir().join(format!("tg-replay-stop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A minute at 8 kHz is 3000 blocks of 20 ms.
        let wav = dir.join("minute.wav");
        let audio = audio::Audio {
            samples: (0..480_000)
                .map(|i| ((i % 80) as f32 / 80.0) - 0.5)
                .collect(),
            sample_rate: 8000,
        };
        audio::write_wav(&wav, &audio).unwrap();
        let flac = dir.join("minute.flac");
        crate::flac::encode_wav(&wav, &flac).unwrap();
        let speed = AtomicU32::new(100.0f32.to_bits());
        for f in [&wav, &flac] {
            let paths = [f.clone()];
            // Dropped: the queue (of one block here) is no longer taken.
            let stop = AtomicBool::new(false);
            let (tx, rx) = sync_channel(1);
            let read = std::thread::scope(|s| {
                let job = s.spawn(|| replay(&paths, 8000, &tx, &stop, &speed));
                assert!(matches!(rx.recv().unwrap(), Event::Audio(_)));
                stop.store(true, Ordering::Relaxed);
                drop(rx);
                job.join().unwrap()
            });
            assert!(read <= 4, "{}: {read} blocks read", f.display());
            // Nothing takes the blocks any more, stop never set: the
            // failed send alone ends it.
            let stop = AtomicBool::new(false);
            let (tx, rx) = sync_channel(1);
            let read = std::thread::scope(|s| {
                let job = s.spawn(|| replay(&paths, 8000, &tx, &stop, &speed));
                assert!(matches!(rx.recv().unwrap(), Event::Audio(_)));
                drop(rx);
                job.join().unwrap()
            });
            assert!(read <= 4, "{}: {read} blocks read, stop unset", f.display());
            // Told to stop, the queue still taken: it ends there.
            let stop = AtomicBool::new(false);
            let (tx, rx) = sync_channel(1);
            let (read, after) = std::thread::scope(|s| {
                let job = s.spawn(|| replay(&paths, 8000, &tx, &stop, &speed));
                assert!(matches!(rx.recv().unwrap(), Event::Audio(_)));
                stop.store(true, Ordering::Relaxed);
                let mut after = 0;
                while let Ok(e) = rx.recv() {
                    match e {
                        Event::Audio(_) => after += 1,
                        Event::End => break,
                        _ => panic!("only sound and its end"),
                    }
                }
                (job.join().unwrap(), after)
            });
            assert!(read <= 4, "{}: {read} blocks read", f.display());
            assert!(after <= 2, "{}: {after} blocks after the stop", f.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A take saved in parts replays as one recording, the parts in the
    /// order given; parts at different sample rates are refused.
    #[test]
    fn parts_replay_in_order() {
        let dir = std::env::temp_dir().join(format!("tg-replay-parts-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let part = |name: &str, value: f32, rate: u32| {
            let f = dir.join(name);
            let audio = audio::Audio {
                samples: vec![value; 4000],
                sample_rate: rate,
            };
            audio::write_wav(&f, &audio).unwrap();
            f
        };
        let a = part("take_part01-001.wav", 0.25, 8000);
        let b = part("take_part01-002.wav", -0.5, 8000);
        let c = replay_files(&[a.clone(), b], "take".into()).unwrap();
        assert_eq!(c.label, "take");
        c.set_speed(100.0);
        let mut got = Vec::new();
        loop {
            match c.rx.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::Audio(b) => got.extend(b.samples),
                Event::End => break,
                Event::Error(e) => panic!("{e}"),
                Event::Lost(n) => panic!("a file waits, but lost {n} frames"),
                Event::Glitch => panic!("a file never glitches"),
            }
        }
        assert_eq!(got.len(), 8000);
        assert!((got[100] - 0.25).abs() < 1e-3 && (got[7900] + 0.5).abs() < 1e-3);
        let odd = part("take_part01-003.wav", 0.0, 11025);
        let e = replay_files(&[a, odd], "take".into()).err().unwrap();
        assert!(e.contains("11025 Hz"), "{e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn menu_keeps_the_default_and_direct_devices() {
        // As cpal lists them on a Linux workstation (some of them).
        let usb = "USB PnP Sound Device, USB Audio";
        let list = vec![
            dev(
                "alsa:default",
                "Default ALSA Output (currently PipeWire Media Server)",
                true,
            ),
            dev("alsa:pipewire", "PipeWire Sound Server", false),
            dev("alsa:null", "Discard all samples", false),
            dev("alsa:sysdefault:CARD=Device", usb, false),
            dev("alsa:front:CARD=Device,DEV=0", usb, false),
            dev("alsa:dsnoop:CARD=Device,DEV=0", usb, false),
            dev("alsa:hw:CARD=Device,DEV=0", usb, false),
            dev("alsa:plughw:CARD=Device,DEV=0", usb, false),
            dev(
                "alsa:hw:CARD=PCH,DEV=0",
                "HDA Intel PCH, ALC897 Analog",
                false,
            ),
            dev(
                "alsa:hw:CARD=PCH,DEV=2",
                "HDA Intel PCH, ALC897 Analog",
                false,
            ),
            // The same devices again, by the cards' numbers.
            dev("alsa:hw:CARD=1,DEV=0", usb, false),
            dev("alsa:plughw:CARD=1,DEV=0", usb, false),
            dev(
                "alsa:hw:CARD=0,DEV=0",
                "HDA Intel PCH, ALC897 Analog",
                false,
            ),
            dev(
                "alsa:hw:CARD=0,DEV=2",
                "HDA Intel PCH, ALC897 Analog",
                false,
            ),
        ];
        let c = choices(&list);
        let labels: Vec<&str> = c.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "System default (via PipeWire)",
                "USB PnP Sound Device, USB Audio (direct)",
                "HDA Intel PCH, ALC897 Analog #0 (direct)",
                "HDA Intel PCH, ALC897 Analog #2 (direct)",
            ]
        );
        assert_eq!(c[0].id, "alsa:default");
        assert!(c[0].is_default);
        assert_eq!(c[1].id, "alsa:hw:CARD=Device,DEV=0");
        assert_eq!(c[3].id, "alsa:hw:CARD=PCH,DEV=2");
    }

    /// A device ALSA lists only by the card's number keeps that id, and a
    /// numbered twin listed first gives way to the named one.
    #[test]
    fn menu_lists_each_direct_device_once() {
        let analog = "HDA Intel PCH, ALC1220 Analog";
        let alt = "HDA Intel PCH, ALC1220 Alt Analog";
        let list = vec![
            dev("alsa:hw:CARD=0,DEV=0", analog, false),
            dev("alsa:hw:CARD=PCH,DEV=0", analog, false),
            dev("alsa:hw:CARD=PCH,DEV=2", alt, false),
            dev("alsa:hw:CARD=0,DEV=2", alt, false),
            dev("alsa:hw:CARD=2,DEV=0", "Card 2", false),
        ];
        let c = choices(&list);
        let ids: Vec<&str> = c.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "alsa:hw:CARD=PCH,DEV=0",
                "alsa:hw:CARD=PCH,DEV=2",
                "alsa:hw:CARD=2,DEV=0"
            ]
        );
        assert_eq!(c[1].label, "HDA Intel PCH, ALC1220 Alt Analog (direct)");
    }

    /// Two of the same USB adapter are two cards of one name: both stay
    /// listed, each once, told apart by the card, whether ALSA lists them
    /// by name first or by number first.
    #[test]
    fn two_identical_cards_stay_two_inputs() {
        let usb = "USB Audio Device, USB Audio";
        let named = [
            dev("alsa:hw:CARD=Device,DEV=0", usb, false),
            dev("alsa:hw:CARD=Device_1,DEV=0", usb, false),
        ];
        let numbered = [
            dev("alsa:hw:CARD=1,DEV=0", usb, false),
            dev("alsa:hw:CARD=2,DEV=0", usb, false),
        ];
        let want = ["alsa:hw:CARD=Device,DEV=0", "alsa:hw:CARD=Device_1,DEV=0"];
        for list in [
            [named.clone(), numbered.clone()].concat(),
            [numbered.clone(), named.clone()].concat(),
        ] {
            let c = choices(&list);
            let ids: Vec<&str> = c.iter().map(|c| c.id.as_str()).collect();
            assert_eq!(ids, want);
            assert_eq!(c[0].label, "USB Audio Device, USB Audio (Device, direct)");
            assert_eq!(c[1].label, "USB Audio Device, USB Audio (Device_1, direct)");
        }
        // Each card with two devices: the card and the device both named.
        let two =
            |card: &str| [0, 1].map(|n| dev(&format!("alsa:hw:CARD={card},DEV={n}"), usb, false));
        let labels: Vec<String> = choices(&[two("Device"), two("Device_1")].concat())
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(
            labels,
            [
                "USB Audio Device, USB Audio #0 (Device, direct)",
                "USB Audio Device, USB Audio #1 (Device, direct)",
                "USB Audio Device, USB Audio #0 (Device_1, direct)",
                "USB Audio Device, USB Audio #1 (Device_1, direct)",
            ]
        );
        // A named entry takes one numbered twin only: a second numbered
        // card of the same name is another card, and stays.
        let list = [named[0].clone(), numbered[0].clone(), numbered[1].clone()];
        let ids: Vec<String> = choices(&list).into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["alsa:hw:CARD=Device,DEV=0", "alsa:hw:CARD=2,DEV=0"]);
        // Two listed only by number are two cards too.
        let ids: Vec<String> = choices(&numbered).into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["alsa:hw:CARD=1,DEV=0", "alsa:hw:CARD=2,DEV=0"]);
    }

    #[test]
    fn sound_card_errors_read_plainly() {
        let e = "get_htstamp `0.0` was earlier than get_trigger_htstamp `37477.441341653`";
        assert!(explain_error(e).starts_with("The sound card refused this input."));
        assert!(explain_error(e).ends_with(&format!("({e})")));
        let alsa = "ALSA function 'snd_pcm_open' failed with error 'Invalid argument (22)'";
        assert!(explain_error(alsa).contains("Choose the system default"));
        assert!(explain_error("A buffer underrun or overrun occurred.")
            .starts_with("The computer fell behind reading this input"));
        assert_eq!(explain_error("something else"), "something else");
        // A fast input's refusal reads as a sentence after "Input stopped."
        // (#2379), its words kept.
        let fast = explain_error(FAST_TEXT);
        assert!(fast.starts_with("This input sends sound faster than real time"));
        assert_eq!(fast.to_lowercase(), FAST_TEXT.to_lowercase());
    }

    /// `no_input` counts core's own two errors as `find` builds them, and
    /// cpal's for an input not there or unplugged; a busy input, or any
    /// other error, is not counted (#2103).
    #[test]
    fn no_input_counts_a_missing_input_only() {
        for missing in [
            no_default_input_error(),
            no_matching_input_error("Scarlett"),
            "The requested audio device is not available. It may have been disconnected."
                .to_string(),
            "The requested device is no longer available. For example, it has been unplugged."
                .to_string(),
        ] {
            assert!(no_input(&missing), "{missing}");
        }
        for other in [
            "The device is busy: another program is using it.",
            "'USB' matches several inputs: a, b; give the whole id",
            "A buffer underrun or overrun occurred.",
        ] {
            assert!(!no_input(other), "{other}");
        }
    }

    #[test]
    fn other_systems_keep_their_list() {
        let list = vec![
            dev("wasapi:a", "Mic", false),
            dev("wasapi:b", "USB Mic", true),
        ];
        let c = choices(&list);
        assert_eq!(c[0].label, "USB Mic (default)");
        assert_eq!(c.len(), 2);
        assert!(explain_error(
            "The requested device is no longer available. For example, it has been unplugged."
        )
        .starts_with("This input is busy or missing"));
    }

    #[test]
    fn level_of_a_sine() {
        let x: Vec<f32> = (0..48000).map(|i| 0.5 * (i as f32 * 0.1).sin()).collect();
        let l = Level::of(&x);
        assert!((l.peak_dbfs + 6.02).abs() < 0.05, "{}", l.peak_dbfs);
        assert!((l.rms_dbfs + 9.03).abs() < 0.05, "{}", l.rms_dbfs);
        assert_eq!(l.clipped_fraction, 0.0);
    }

    #[test]
    fn a_full_queue_drops_sound_and_says_how_much_before_the_next_block() {
        let (tx, rx) = sync_channel(2);
        let mut feed = LiveQueue::new(tx, 48_000);
        let at = SystemTime::UNIX_EPOCH;
        let block = |k: usize, n: usize| Block {
            samples: vec![k as f32; n],
            at,
        };
        // Nothing takes the blocks: two fit, the next three (30 + 40 + 50
        // frames) are dropped.
        for (k, n) in [(0, 10), (1, 20), (2, 30), (3, 40), (4, 50)] {
            feed.send(block(k, n));
        }
        let got = |rx: &Receiver<Event>| -> Vec<String> {
            rx.try_iter()
                .map(|e| match e {
                    Event::Audio(b) => format!("audio {}", b.samples[0]),
                    Event::Lost(n) => format!("lost {n}"),
                    other => format!("{other:?}"),
                })
                .collect()
        };
        assert_eq!(got(&rx), ["audio 0", "audio 1"]);
        // Room again: the loss goes first, then the block.
        feed.send(block(5, 60));
        assert_eq!(got(&rx), ["lost 120", "audio 5"]);
        // Room for the count but not the block after it: that block is
        // the next loss.
        feed.send(block(6, 10));
        feed.send(block(7, 10));
        feed.send(block(8, 10));
        assert_eq!(got(&rx), ["audio 6", "audio 7"]);
        feed.send(block(9, 10));
        assert_eq!(got(&rx), ["lost 10", "audio 9"]);
        // Nothing lost since: blocks only.
        feed.send(block(10, 10));
        feed.send(block(11, 10));
        assert_eq!(got(&rx), ["audio 10", "audio 11"]);
    }

    /// Blocks of `secs` seconds at 1 kHz, `n` of them, sent `every` seconds
    /// apart from one start, the queue drained after each: what came out.
    fn paced(secs: f64, every: f64, n: usize) -> Vec<String> {
        let (tx, rx) = sync_channel(4);
        let mut feed = LiveQueue::new(tx, 1_000);
        let start = Instant::now();
        let mut out = Vec::new();
        for k in 0..n {
            let at = start + Duration::from_secs_f64(k as f64 * every);
            feed.send_at(
                Block {
                    samples: vec![0.0; (secs * 1_000.0) as usize],
                    at: SystemTime::UNIX_EPOCH,
                },
                at,
            );
            out.extend(rx.try_iter().map(|e| match e {
                Event::Audio(_) => "audio".to_string(),
                Event::Error(e) => e,
                other => format!("{other:?}"),
            }));
        }
        out
    }

    /// A null input makes up silence as fast as it is asked (#2379): at
    /// 2.5 times real time it is refused once it is two seconds ahead, with
    /// one error, and nothing after it.
    #[test]
    fn an_input_faster_than_real_time_is_refused_once() {
        // 0.1 s blocks every 0.04 s: 0.06 s further ahead each block, so
        // the 33rd (3.3 s sent at 1.28 s) is the first over two seconds
        // ahead.
        let out = paced(0.1, 0.04, 200);
        assert_eq!(out.len(), 33, "{out:?}");
        assert!(out[..32].iter().all(|e| e == "audio"), "{out:?}");
        assert_eq!(out[32], FAST_TEXT);
    }

    /// A sound card at its own pace, or 999 parts per million fast over
    /// a whole day, is never refused; nor is a first buffer of nearly two
    /// seconds.
    #[test]
    fn an_input_at_its_own_pace_is_never_refused() {
        assert!(paced(0.01, 0.01, 10_000).iter().all(|e| e == "audio"));
        // A day in 1 s blocks, each 0.999 ms short of a second apart.
        assert!(paced(1.0, 0.999_001, 86_400).iter().all(|e| e == "audio"));
        assert!(paced(1.99, 1.0, 1).iter().all(|e| e == "audio"));
        // Exactly two seconds ahead is still within the limit.
        assert_eq!(paced(2.0, 1.0, 1), ["audio"]);
        assert_eq!(paced(2.01, 1.0, 1), [FAST_TEXT]);
    }

    /// Once refused, an input stays refused though it falls back within
    /// the limit: a 2.01 s first block, then 20 s at real time, by when
    /// the 0.1% share has grown past the 0.01 s it was over, gives the
    /// refusal and nothing else.
    #[test]
    fn a_refused_input_stays_refused() {
        let (tx, rx) = sync_channel(64);
        let mut feed = LiveQueue::new(tx, 1_000);
        let start = Instant::now();
        let block = |n: usize| Block {
            samples: vec![0.0; n],
            at: SystemTime::UNIX_EPOCH,
        };
        feed.send_at(block(2_010), start);
        for k in 1..=20 {
            feed.send_at(block(1_000), start + Duration::from_secs(k));
        }
        let got: Vec<Event> = rx.try_iter().collect();
        assert!(
            matches!(&got[..], [Event::Error(e)] if e == FAST_TEXT),
            "{got:?}"
        );
    }

    /// The refusal waits for room in a full queue, and goes once.
    #[test]
    fn the_refusal_waits_for_room_and_goes_once() {
        let (tx, rx) = sync_channel(1);
        let mut feed = LiveQueue::new(tx, 1_000);
        let at = Instant::now();
        let block = |n: usize| Block {
            samples: vec![0.0; n],
            at: SystemTime::UNIX_EPOCH,
        };
        feed.send_at(block(1_000), at);
        // Three seconds at once, with the queue full: refused, not told.
        feed.send_at(block(3_000), at);
        feed.send_at(block(10), at);
        assert!(matches!(rx.try_recv(), Ok(Event::Audio(_))));
        assert!(rx.try_recv().is_err());
        feed.send_at(block(10), at);
        feed.send_at(block(10), at);
        let got: Vec<Event> = rx.try_iter().collect();
        assert!(
            matches!(&got[..], [Event::Error(e)] if e == FAST_TEXT),
            "{got:?}"
        );
    }
}
