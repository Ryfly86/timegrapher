//! A recording folder its program left without finishing it (a crash, a
//! kill, power lost): how much sound it holds, read from the sound
//! itself, and its `session.json` brought up to that (#2412, #2431).
//! While it records, `session.json` follows the sound only every
//! clock line, so after a stop it can say less than the files hold.

use super::*;

/// Seconds of sound the recording folder `dir` holds, from its sound
/// files as Open reads them ([`crate::recording_order::recordings_in`],
/// each WAV to the end of its sound, [`crate::audio::open_wav`]). None
/// when that can't be known: no sound file, one that can't be read, or
/// an hour its `session.json` lists that is not in the folder (kept in
/// its take alone, #2040), so never a length short of the sound.
pub fn sound_on_disk_s(dir: &Path) -> Option<f64> {
    let listed = read_record(dir).map_or_else(Vec::new, |a| listed_segments(&a));
    if listed.iter().any(|s| !segment_here(dir, s)) {
        return None;
    }
    let files = crate::recording_order::recordings_in(dir, &listed).ok()?;
    let seconds = |f: &PathBuf| {
        let i = crate::audio::info(f).ok()?;
        Some(i.frames? as f64 / f64::from(i.sample_rate.max(1)))
    };
    files.iter().map(seconds).sum()
}

/// Bring the `session.json` of the recording folder `dir`, left by a
/// program that stopped, up to the sound it holds: its `duration_s` the
/// sound's length ([`sound_on_disk_s`]), and its `segments` naming every
/// sound file in the folder, an hour begun since its last write
/// included, so the take reads the same length wherever it goes next
/// (Open, Move To…, a watch). Written whole ([`crate::library::write_atomic`]);
/// left as it is while the folder is recorded into ([`in_use`]), or when
/// the length can't be known. Returns the sound's length.
pub fn mend_stopped(dir: &Path) -> io::Result<Option<f64>> {
    let Some(length_s) = sound_on_disk_s(dir) else {
        return Ok(None);
    };
    let Some(mut about) = read_record(dir).filter(|_| !in_use(dir)) else {
        return Ok(Some(length_s));
    };
    let mut segments = listed_segments(&about);
    let files = crate::recording_order::recordings_in(dir, &segments).map_err(io::Error::other)?;
    for f in &files {
        let name = f.file_name().unwrap_or_default().to_string_lossy();
        if !segments.iter().any(|s| same_hour(s, &name)) {
            segments.push(name.into_owned());
        }
    }
    let said = about["duration_s"].as_f64();
    if said == Some(length_s) && listed_segments(&about) == segments {
        return Ok(Some(length_s));
    }
    about["duration_s"] = serde_json::json!(length_s);
    about["segments"] = serde_json::json!(segments);
    let text = serde_json::to_string_pretty(&about).map_err(io::Error::other)?;
    crate::library::write_atomic(&dir.join(take::RECORD), text.as_bytes())
        .map_err(io::Error::other)?;
    Ok(Some(length_s))
}

fn read_record(dir: &Path) -> Option<serde_json::Value> {
    let bytes = fs::read(dir.join(take::RECORD)).ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .filter(serde_json::Value::is_object)
}

fn listed_segments(about: &serde_json::Value) -> Vec<String> {
    about["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

/// Whether the hour `name` is in `dir`, as itself or as the FLAC it was
/// kept as.
fn segment_here(dir: &Path, name: &str) -> bool {
    let p = dir.join(name);
    p.is_file() || (crate::flac::is_wav(&p) && crate::flac::flac_path(&p).is_file())
}

/// Whether the listed hour `listed` is the file `name`: itself, or the
/// FLAC its WAV was kept as.
fn same_hour(listed: &str, name: &str) -> bool {
    listed == name || as_flac(listed) == name
}
