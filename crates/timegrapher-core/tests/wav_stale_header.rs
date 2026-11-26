//! A WAV left by a program that stopped mid-take (#2054): its header
//! gives the length of the sound at the last flush, but more sound lies
//! on disk after it. Every reader reads all of it, in whole frames; a
//! file whose header is right, or which carries a tag after its sound,
//! reads as its header says.

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
        let wav = d.join(format!("audio-{channels}.wav"));
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
        let kept = d.join(format!("audio-{channels}.flac"));
        assert_eq!(flac::encode_wav(&wav, &kept).unwrap(), 18_000);
        assert!(flac::same_samples(&wav, &kept).unwrap());
        assert_eq!(audio::load(&kept).unwrap().samples, want);
    }
}

#[test]
fn a_wav_whose_header_is_right_reads_as_before() {
    let d = dir("right");
    let wav = d.join("audio.wav");
    killed(&wav, 1, 8000, 0, &[]);
    assert_eq!(header_frames(&wav), 8000);
    assert_eq!(audio::load(&wav).unwrap().samples, mono(8000, 1));
}

#[test]
fn a_tag_after_the_sound_is_not_read_as_sound() {
    let d = dir("tag");
    let wav = d.join("audio.wav");
    let mut tag = b"LIST".to_vec();
    let body = b"INFOISFT\x06\x00\x00\x00tg 1.0";
    tag.extend_from_slice(&(body.len() as u32).to_le_bytes());
    tag.extend_from_slice(body);
    killed(&wav, 1, 8000, 0, &tag);
    assert_eq!(audio::load(&wav).unwrap().samples, mono(8000, 1));
    assert_eq!(audio::info(&wav).unwrap().frames, Some(8000));
}
