//! Hours of saved recordings kept as FLAC after the fact (#503): the
//! takes still holding a WAV, and the conversion of each.

use super::{in_use, is_sound};
use crate::take;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// The recordings in `take` whose `session.json` lists an hour still
/// kept as a WAV, or kept as a [`crate::flac::bloated`] FLAC: `take`
/// itself, and the takes a session folder holds (`timegrapher record`
/// makes one per position). These were saved before hours were kept as
/// FLAC (#503), stopped before one was converted, or converted before
/// each block's size was checked. A recording being made ([`in_use`]) is
/// never listed.
pub fn holding_wav(take: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![take.to_path_buf()];
    if let Ok(entries) = fs::read_dir(take) {
        let mut inner: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.path())
            .collect();
        inner.sort();
        dirs.extend(inner);
    }
    dirs.retain(|d| !hours_to_keep(d).is_empty());
    dirs
}

/// The hours `session.json` in `dir` lists that are WAVs there now, or
/// [`crate::flac::bloated`] FLACs. None while it is being recorded
/// ([`in_use`]): its open hour is a WAV the recorder is still writing, and
/// its closed ones the recorder converts.
fn hours_to_keep(dir: &Path) -> Vec<String> {
    if in_use(dir) {
        return Vec::new();
    }
    let Ok(text) = fs::read_to_string(dir.join(take::RECORD)) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    v["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str())
        .filter(|n| is_sound(n) && Path::new(n).components().count() == 1)
        .filter(|n| {
            let p = dir.join(n);
            p.is_file() && (crate::flac::is_wav(&p) || crate::flac::bloated(&p))
        })
        .map(str::to_string)
        .collect()
}

/// Keep each hour of the saved recording in `dir` that is still a WAV as
/// FLAC, one at a time ([`crate::flac::keep_as_flac`]: encoded, checked
/// sample for sample, then the WAV deleted), and name the FLAC in
/// `session.json` (its `segments` and breaks) after each. An hour kept as
/// a [`crate::flac::bloated`] FLAC is written again
/// ([`crate::flac::shrink`]), under its own name. An hour that cannot be
/// converted stays as it was, whole, and the error says which. A
/// recording being made ([`in_use`]) is left alone. Returns how many hours
/// were converted.
pub fn keep_saved_as_flac(dir: &Path) -> Result<usize, String> {
    keep_saved_as_flac_unless(dir, &|| false)
}

/// [`keep_saved_as_flac`], asking `stop` before each hour: when it says
/// so, the hours not yet converted are left as WAVs, for another time (a
/// take opened in the app while its hours are kept, #503).
pub fn keep_saved_as_flac_unless(dir: &Path, stop: &dyn Fn() -> bool) -> Result<usize, String> {
    let record = dir.join(take::RECORD);
    let mut done = 0;
    for wav in hours_to_keep(dir) {
        if stop() {
            break;
        }
        let path = dir.join(&wav);
        if !crate::flac::is_wav(&path) {
            crate::flac::shrink(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            done += 1;
            continue;
        }
        let flac =
            crate::flac::keep_as_flac(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let flac = flac
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let text = fs::read_to_string(&record).map_err(|e| format!("{}: {e}", record.display()))?;
        let text = renamed_segment(&text, &wav, &flac)
            .ok_or_else(|| format!("{}: not a take's record", record.display()))?;
        crate::library::write_atomic(&record, text.as_bytes())?;
        done += 1;
    }
    Ok(done)
}

/// `text`, a `session.json`, with each reference to the hour `from` (an
/// entry of `segments`, or a break's `segment`) naming `to` instead. Only
/// those strings change: the watch's name, the notes and any field this
/// version does not know are kept byte for byte, even when they hold the
/// same text (#2419). None if `text` is not a JSON object.
fn renamed_segment(text: &str, from: &str, to: &str) -> Option<String> {
    let quoted = serde_json::Value::from(to).to_string();
    let mut out = String::with_capacity(text.len());
    let mut kept = 0;
    for r in segment_refs(text)? {
        if serde_json::from_str::<String>(&text[r.clone()]).ok()? == from {
            out.push_str(&text[kept..r.start]);
            out.push_str(&quoted);
            kept = r.end;
        }
    }
    out.push_str(&text[kept..]);
    Some(out)
}

/// Where in `text`, a `session.json`, each segment reference is: the
/// strings in `segments` and each break's `segment`, quotes included, in
/// the order they appear.
fn segment_refs(text: &str) -> Option<Vec<Range<usize>>> {
    let mut refs = Vec::new();
    let mut s = Scan { t: text, i: 0 };
    s.object(&mut |s, key| match key {
        "segments" if s.peek()? == b'[' => s.array(&mut |s| match s.peek()? {
            b'"' => s.string().map(|r| refs.push(r)),
            _ => s.skip(),
        }),
        "breaks" if s.peek()? == b'[' => s.array(&mut |s| match s.peek()? {
            b'{' => s.object(&mut |s, key| match (key, s.peek()?) {
                ("segment", b'"') => s.string().map(|r| refs.push(r)),
                _ => s.skip(),
            }),
            _ => s.skip(),
        }),
        _ => s.skip(),
    })?;
    s.ws();
    (s.i == text.len()).then_some(refs)
}

/// A walk through JSON text that keeps track of where each value lies.
struct Scan<'a> {
    t: &'a str,
    i: usize,
}

impl Scan<'_> {
    fn ws(&mut self) {
        let b = self.t.as_bytes();
        while matches!(b.get(self.i), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.t.as_bytes().get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> Option<()> {
        (self.peek()? == c).then(|| self.i += 1)
    }

    /// A string, quotes included.
    fn string(&mut self) -> Option<Range<usize>> {
        let start = self.i;
        self.eat(b'"')?;
        loop {
            match *self.t.as_bytes().get(self.i)? {
                b'\\' => self.i += 2,
                b'"' => break,
                _ => self.i += 1,
            }
        }
        self.i += 1;
        Some(start..self.i)
    }

    /// An object, calling `each` with its key at each value.
    fn object(&mut self, each: &mut dyn FnMut(&mut Self, &str) -> Option<()>) -> Option<()> {
        self.eat(b'{')?;
        if self.peek()? == b'}' {
            self.i += 1;
            return Some(());
        }
        loop {
            self.ws();
            let key = self.string()?;
            let key: String = serde_json::from_str(&self.t[key]).ok()?;
            self.eat(b':')?;
            each(self, &key)?;
            match self.peek()? {
                b',' => self.i += 1,
                b'}' => break,
                _ => return None,
            }
        }
        self.i += 1;
        Some(())
    }

    /// An array, calling `each` at each value.
    fn array(&mut self, each: &mut dyn FnMut(&mut Self) -> Option<()>) -> Option<()> {
        self.eat(b'[')?;
        if self.peek()? == b']' {
            self.i += 1;
            return Some(());
        }
        loop {
            each(self)?;
            match self.peek()? {
                b',' => self.i += 1,
                b']' => break,
                _ => return None,
            }
        }
        self.i += 1;
        Some(())
    }

    /// Any value, passed over.
    fn skip(&mut self) -> Option<()> {
        match self.peek()? {
            b'"' => self.string().map(drop),
            b'{' => self.object(&mut |s, _| s.skip()),
            b'[' => self.array(&mut |s| s.skip()),
            _ => {
                let start = self.i;
                let b = self.t.as_bytes();
                while !matches!(
                    b.get(self.i),
                    None | Some(b',' | b']' | b'}' | b' ' | b'\n' | b'\r' | b'\t')
                ) {
                    self.i += 1;
                }
                (self.i > start).then_some(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A saved take's record where the hour's name is also the watch's
    /// name, a note, a field this version does not know and a key, laid
    /// out by hand rather than as this version writes it.
    const RECORD: &str = r#"{
  "watch": "audio-001.wav",
  "segments": [ "audio-001.wav",  "audio-002.wav" ],
  "notes": [{"text": "audio-001.wav", "at_s": 1.50}],
  "breaks": [
    {"audio_s": 1.0, "segment": "audio-001.wav", "note": "audio-001.wav"},
    {"segment": "audio-002.wav"}
  ],
  "later": {"segments": ["audio-001.wav"], "x": [1e3, true, null, "a\"b\\"]},
  "audio-001.wav": "audio-001.wav"
}
"#;

    /// Keeping an hour as FLAC renames only the hour's references: the
    /// `segments` entry and the break's `segment` (#2419).
    #[test]
    fn only_the_segment_references_are_renamed() {
        let expected = RECORD
            .replacen(r#"[ "audio-001.wav","#, r#"[ "audio-001.flac","#, 1)
            .replacen(
                r#""segment": "audio-001.wav""#,
                r#""segment": "audio-001.flac""#,
                1,
            );
        assert_ne!(expected, RECORD);
        let got = renamed_segment(RECORD, "audio-001.wav", "audio-001.flac").unwrap();
        assert_eq!(got, expected);
        let v: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(v["watch"], "audio-001.wav");
        assert_eq!(v["notes"][0]["text"], "audio-001.wav");
        assert_eq!(v["later"]["segments"][0], "audio-001.wav");
        assert_eq!(v["segments"][1], "audio-002.wav");
        // An escaped name is still the same name.
        let escaped = r#"{"segments": ["audio-001.wav"], "watch": "audio-001.wav"}"#;
        assert_eq!(
            renamed_segment(escaped, "audio-001.wav", "audio-001.flac").unwrap(),
            r#"{"segments": ["audio-001.flac"], "watch": "audio-001.wav"}"#
        );
        // Not a record: nothing is guessed at.
        for bad in ["", "[]", r#"{"segments": ["a"]"#, r#"{"a": 1} x"#] {
            assert_eq!(renamed_segment(bad, "a", "b"), None, "{bad}");
        }
    }

    /// The round trip on disk: a saved take whose hour's name is also its
    /// watch's name, a note and an unknown field keeps all those as they
    /// were once the hour is a FLAC (#2419).
    #[test]
    fn keeping_an_hour_as_flac_keeps_the_user_text() {
        let dir = std::env::temp_dir().join(format!("tg-saved-flac-text-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let audio = crate::audio::Audio {
            samples: (0..8000).map(|i| ((i % 80) as f32 / 80.0) - 0.5).collect(),
            sample_rate: 8000,
        };
        crate::audio::write_wav(&dir.join("audio-001.wav"), &audio).unwrap();
        let record = RECORD.replace(r#",  "audio-002.wav""#, "");
        let record = record.replace(",\n    {\"segment\": \"audio-002.wav\"}", "");
        fs::write(dir.join(take::RECORD), &record).unwrap();
        assert_eq!(keep_saved_as_flac(&dir).unwrap(), 1);
        assert!(dir.join("audio-001.flac").is_file());
        assert!(!dir.join("audio-001.wav").exists());
        let got = fs::read_to_string(dir.join(take::RECORD)).unwrap();
        let expected = record
            .replacen(r#"[ "audio-001.wav" ]"#, r#"[ "audio-001.flac" ]"#, 1)
            .replacen(
                r#""segment": "audio-001.wav""#,
                r#""segment": "audio-001.flac""#,
                1,
            );
        assert_ne!(expected, record);
        assert_eq!(got, expected);
        fs::remove_dir_all(&dir).unwrap();
    }
}
