//! A WAV left by a program that stopped mid-take (#2054): its header
//! gives the length of the sound at the last flush, but more sound lies
//! on disk after it. Every reader reads all of a take's own sound file
//! (`audio-NNN.wav`), in whole frames; a file whose header is right, one
//! carrying a tag or pad byte after its sound, and any WAV not named as a
//! take's sound, reads as its header says.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use timegrapher_core::{audio, excerpt, flac};

const RATE: u32 = 8000;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tg-stale-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Sample `i` of channel `c`, a value the reader can be checked against.
fn sample(i: u32, c: u32) -> i32 {
    ((i * 7 + c * 1000) % 30_000) as i32 - 15_000
}

/// A 16-bit WAV of `channels` written as the recorder writes one: a flush
/// after `flushed` frames writes the header, `more` frames follow, and
/// the program stops before the next flush. `tail` raw bytes end it, as
/// a frame cut part way.
fn killed(path: &Path, channels: u16, flushed: u32, more: u32, tail: &[u8]) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::new(File::create(path).unwrap(), spec).unwrap();
    for i in 0..flushed + more {
        for c in 0..u32::from(channels) {
            w.write_sample(sample(i, c) as i16).unwrap();
        }
        if i + 1 == flushed {
            w.flush().unwrap();
        }
    }
    // Killed: no flush or finalize writes the header again.
    std::mem::forget(w);
    File::options()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(tail)
        .unwrap();
}

/// The header's data length, as `hound` alone reads it.
fn header_frames(path: &Path) -> u32 {
    hound::WavReader::open(path).unwrap().duration()
}

fn mono(frames: u32, channels: u16) -> Vec<f32> {
    (0..frames)
        .map(|i| {
            let ch = u32::from(channels);
            let sum: i32 = (0..ch).map(|c| sample(i, c)).sum();
            sum as f32 / 32768.0 / ch as f32
        })
        .collect()
}

#[test]
fn a_killed_takes_wav_is_read_to_the_end_of_its_sound() {
    let d = dir("killed");
    for channels in [1u16, 2] {
        let wav = d.join(format!("audio-00{channels}.wav"));
        // 1.5 s flushed, 0.75 s after it, then half a frame.
        killed(&wav, channels, 12_000, 6_000, &[0x11]);
        assert_eq!(header_frames(&wav), 12_000, "the header is stale");
        let want = mono(18_000, channels);

        let a = audio::load(&wav).unwrap();
        assert_eq!(a.samples.len(), 18_000);
        assert_eq!(a.samples, want);
        assert_eq!(audio::info(&wav).unwrap().frames, Some(18_000));
        let mut streamed = Vec::new();
        audio::stream(&wav, 1000, |b| streamed.extend_from_slice(b)).unwrap();
        assert_eq!(streamed, want);

        let pcm = excerpt::read(&wav).unwrap();
        assert_eq!(pcm.samples.len(), 18_000 * usize::from(channels));

        // Kept as FLAC, it keeps the last seconds too, and checks.
        let kept = d.join(format!("audio-00{channels}.flac"));
        assert_eq!(flac::encode_wav(&wav, &kept).unwrap(), 18_000);
        assert!(flac::same_samples(&wav, &kept).unwrap());
        assert_eq!(audio::load(&kept).unwrap().samples, want);
    }
}

#[test]
fn a_wav_whose_header_is_right_reads_as_before() {
    let d = dir("right");
    let wav = d.join("audio-001.wav");
    killed(&wav, 1, 8000, 0, &[]);
    assert_eq!(header_frames(&wav), 8000);
    assert_eq!(audio::load(&wav).unwrap().samples, mono(8000, 1));
}

#[test]
fn a_tag_after_the_sound_is_not_read_as_sound() {
    let d = dir("tag");
    let wav = d.join("audio-001.wav");
    let mut tag = b"LIST".to_vec();
    let body = b"INFOISFT\x06\x00\x00\x00tg 1.0";
    tag.extend_from_slice(&(body.len() as u32).to_le_bytes());
    tag.extend_from_slice(body);
    killed(&wav, 1, 8000, 0, &tag);
    assert_eq!(audio::load(&wav).unwrap().samples, mono(8000, 1));
    assert_eq!(audio::info(&wav).unwrap().frames, Some(8000));
}

/// A WAV not named as a take's sound reads as its header says: a stale
/// header, or bytes after the sound that are no chunk (a raw ID3 tag,
/// zeros), are not read as sound.
#[test]
fn a_wav_from_elsewhere_reads_as_its_header_says() {
    let d = dir("elsewhere");
    let stale = d.join("take.wav");
    killed(&stale, 1, 12_000, 6_000, &[]);
    assert_eq!(audio::info(&stale).unwrap().frames, Some(12_000));
    assert_eq!(audio::load(&stale).unwrap().samples, mono(12_000, 1));
    let kept = d.join("take.flac");
    assert_eq!(flac::encode_wav(&stale, &kept).unwrap(), 12_000);
    assert!(flac::same_samples(&stale, &kept).unwrap());

    let mut id3 = b"ID3\x03\x00\x00\x00\x00\x01\x0f".to_vec();
    id3.resize(10 + 143, 0x41);
    for (name, tail) in [("id3.wav", id3), ("zeros.wav", vec![0u8; 4096])] {
        let wav = d.join(name);
        killed(&wav, 1, 8000, 0, &tail);
        assert_eq!(audio::info(&wav).unwrap().frames, Some(8000), "{name}");
        assert_eq!(audio::load(&wav).unwrap().samples, mono(8000, 1), "{name}");
    }
}

/// An odd data length is followed by its pad byte, which is not sound,
/// in a take's own file too.
#[test]
fn a_pad_byte_after_an_odd_length_is_not_read_as_sound() {
    let d = dir("pad");
    let wav = d.join("audio-001.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 8,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for i in 0..8001 {
        w.write_sample((i % 100) as i8).unwrap();
    }
    w.finalize().unwrap();
    // The RIFF pad byte, which `hound` leaves out but other writers add.
    File::options()
        .append(true)
        .open(&wav)
        .unwrap()
        .write_all(&[0])
        .unwrap();
    assert_eq!(audio::info(&wav).unwrap().frames, Some(8001));
    assert_eq!(audio::load(&wav).unwrap().samples.len(), 8001);
}

/// The recorder's held hours (a save's copy of each closed hour) are read
/// through `flac::encode_wav_file` and `same_samples_file`, by file and
/// not by name: they read a killed hour to the end of its sound too, so a
/// saved recovered take keeps its last seconds.
#[test]
fn a_held_killed_hour_is_kept_as_flac_whole() {
    let d = dir("held");
    // Not named as a take's sound: the held file is always the take's own.
    let wav = d.join("held.wav");
    killed(&wav, 1, 12_000, 6_000, &[]);
    let held = File::open(&wav).unwrap();
    let kept = d.join("held.flac");
    assert_eq!(flac::encode_wav_file(&held, &kept).unwrap(), 18_000);
    assert!(flac::same_samples_file(&held, &kept).unwrap());
    assert_eq!(audio::load(&kept).unwrap().samples, mono(18_000, 1));
}

/// A quiet tail (a watch that stopped, an empty stand) is sound, not a
/// chunk: zero bytes give no chunk name.
#[test]
fn a_quiet_tail_is_read_as_sound() {
    let d = dir("quiet");
    let wav = d.join("audio-001.wav");
    killed(&wav, 1, 8000, 0, &vec![0u8; 2 * 4000]);
    let mut want = mono(8000, 1);
    want.resize(12_000, 0.0);
    assert_eq!(audio::info(&wav).unwrap().frames, Some(12_000));
    assert_eq!(audio::load(&wav).unwrap().samples, want);
}

/// Sound whose first bytes happen to be letters is still sound when the
/// length they would give runs past the end of the file.
#[test]
fn letters_whose_length_runs_past_the_end_are_sound() {
    let d = dir("letters");
    let wav = d.join("audio-001.wav");
    let mut tail = b"abcd".to_vec();
    tail.extend_from_slice(&u32::MAX.to_le_bytes());
    tail.resize(8 + 2 * 4000, 0);
    killed(&wav, 1, 8000, 0, &tail);
    assert_eq!(audio::info(&wav).unwrap().frames, Some(12_004));
}

/// A WAV made byte by byte: 16-bit mono at [`RATE`], `before` (whole
/// chunks) between `fmt ` and `data`, a data length of `len`, then `data`.
fn made(path: &Path, block_align: u16, before: &[u8], len: u32, data: &[u8]) {
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&block_align.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(before);
    b.extend_from_slice(b"data");
    b.extend_from_slice(&len.to_le_bytes());
    b.extend_from_slice(data);
    std::fs::write(path, b).unwrap();
}

/// A take's own file whose `fmt ` gives a block align of 0 is not read
/// to the end (no frame size to round to): it reads, or is refused, as
/// `hound` judges it, and never panics.
#[test]
fn a_block_align_of_zero_does_not_panic() {
    let d = dir("align0");
    let wav = d.join("audio-001.wav");
    made(&wav, 0, &[], 2 * 100, &[0x10; 2 * 300]);
    let read = std::panic::catch_unwind(|| audio::load(&wav).map(|a| a.samples.len()));
    let read = read.expect("no panic");
    assert!(!matches!(read, Ok(300)), "{read:?}");
}

/// A chunk name may hold any printable character, such as Adobe's XMP
/// `_PMX`: that tag after a take's own sound is not read as sound.
#[test]
fn an_xmp_tag_after_the_sound_is_not_read_as_sound() {
    let d = dir("xmp");
    let wav = d.join("audio-001.wav");
    let mut tag = b"_PMX".to_vec();
    tag.extend_from_slice(&6u32.to_le_bytes());
    tag.extend_from_slice(b"<x:x/>");
    killed(&wav, 1, 8000, 0, &tag);
    assert_eq!(audio::info(&wav).unwrap().frames, Some(8000));
}
