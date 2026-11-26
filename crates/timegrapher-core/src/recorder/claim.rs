//! A take's folder in a watch, made as it is named (#1988, #1862).
//!
//! A take recorded into a watch is named by the minute it started. Its
//! name used to be picked as the first free one, and its folder made only
//! at its first save: two takes started in the same minute (two benches,
//! or a take whose first save failed and another started) could be given
//! one name, and the second save would write into the first take. Now
//! the folder is made as the name is given, in one step, so a name once
//! given is never given again.

use super::Recorder;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

impl Recorder {
    /// Give the first saved copy a take folder of its own in `parent` (a
    /// watch's takes), named `base`, or `base 2` and on where that is
    /// taken, rather than by its start, watch and calibre. The folder is
    /// made now ([`crate::library::make_unused_dir`]), so no other take
    /// is given it before the first save writes there. A copy already
    /// given its folder in `parent` (a first save that failed, tried
    /// again, or the copy saved there before) keeps it and makes no
    /// other. Returns the copy's folder.
    pub fn claim_first_copy(&mut self, parent: &Path, base: &str) -> io::Result<PathBuf> {
        fs::create_dir_all(parent)?;
        let canonical = fs::canonicalize(parent)?;
        let name = match self.kept_in(&canonical) {
            Some(name) => name,
            None => self.claim(parent, &canonical, base)?,
        };
        Ok(parent.join(name))
    }

    /// Where a save into `parent` writes: the copy saved there before
    /// (Save Again), or the folder made for it there and not yet written;
    /// else a new folder, named as the copy saved elsewhere or by its
    /// start, watch and `calibre`, with ` 2` and on where that is taken.
    /// A save into another folder never writes into a take already there
    /// (#2401). Refused where the copy would be, or hold, the live
    /// recording. Returns its name, its folder as the caller spelled
    /// `parent`, and whether it is a new one.
    pub(super) fn copy_dest(
        &self,
        parent: &Path,
        calibre: &str,
    ) -> io::Result<(OsString, PathBuf, bool)> {
        // Resolve identities only for these checks. Keep the caller's
        // spelling for filesystem work and paths returned or shown to them.
        let canonical = fs::canonicalize(parent)?;
        let source = fs::canonicalize(&self.dir)?;
        let separate = |name: &OsStr| -> io::Result<PathBuf> {
            let dest = parent.join(name);
            let resolved = if dest.exists() {
                fs::canonicalize(&dest)?
            } else {
                canonical.join(name)
            };
            if resolved.starts_with(&source) || source.starts_with(&resolved) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the saved copy must be separate from the live recording folder",
                ));
            }
            Ok(dest)
        };
        if let Some(name) = self.kept_in(&canonical) {
            let dest = separate(&name)?;
            return Ok((name, dest, false));
        }
        let base = self
            .saved_name
            .clone()
            .unwrap_or_else(|| self.copy_name(calibre));
        separate(&base)?;
        let free = crate::library::unused_dir(parent, &base.to_string_lossy());
        let name = free.file_name().map_or(base, ToOwned::to_owned);
        let dest = separate(&name)?;
        Ok((name, dest, true))
    }

    /// The name of this copy's folder in the folder resolved as
    /// `canonical`: the copy saved there, or one made there for it.
    fn kept_in(&self, canonical: &Path) -> Option<OsString> {
        if self.saved_parent.as_deref() == Some(canonical) {
            return self.saved_name.clone();
        }
        self.claimed
            .as_ref()
            .filter(|c| c.parent() == Some(canonical) && c.is_dir())
            .and_then(|c| c.file_name())
            .map(ToOwned::to_owned)
    }

    /// Make a new folder for this copy in `parent`, named `base` or with
    /// a number, and note it. Returns its name.
    fn claim(&mut self, parent: &Path, canonical: &Path, base: &str) -> io::Result<OsString> {
        let dir = crate::library::make_unused_dir(parent, base)?;
        let name = dir
            .file_name()
            .map(ToOwned::to_owned)
            .ok_or_else(|| io::Error::other("a take folder with no name"))?;
        self.claimed = Some(canonical.join(&name));
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::save_fixture;
    use crate::capture::Block;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, UNIX_EPOCH};

    fn files(dir: &Path) -> BTreeMap<std::ffi::OsString, Vec<u8>> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().unwrap().is_file())
            .map(|e| (e.file_name(), fs::read(e.path()).unwrap()))
            .collect()
    }

    /// Save As into another watch's takes, where a take of the same minute
    /// name is already saved, makes a folder of its own there and leaves
    /// that take's sound as it was; into the take's own watch it saves
    /// again in place, and into an empty folder it makes one (#2401).
    #[test]
    fn save_as_never_writes_into_another_takes_folder() {
        let (mut a, base) = save_fixture("save-as-a");
        let (mut b, other) = save_fixture("save-as-b");
        for (r, level) in [(&mut a, 0.25), (&mut b, -0.25)] {
            r.write(&Block {
                samples: vec![level; 8000],
                at: UNIX_EPOCH + Duration::from_secs(2),
            })
            .unwrap();
        }
        let name = "2026-11-26 05.30";
        let (a_takes, b_takes) = (base.join("A").join("takes"), base.join("B").join("takes"));
        a.claim_first_copy(&a_takes, name).unwrap();
        b.claim_first_copy(&b_takes, name).unwrap();
        let a_dir = a.save_copy(&a_takes, "", None).unwrap();
        let b_dir = b.save_copy(&b_takes, "", None).unwrap();
        assert_eq!(b_dir, b_takes.join(name));
        let b_before = files(&b_dir);
        let a_sound = fs::read(a_dir.join("audio-001.wav")).unwrap();
        assert_ne!(a_sound, b_before[std::ffi::OsStr::new("audio-001.wav")]);
        // Again into its own watch: the same folder.
        assert_eq!(a.save_copy(&a_takes, "", None).unwrap(), a_dir);
        // Into an empty folder: a take of its own there.
        let c_takes = base.join("C").join("takes");
        assert_eq!(a.save_copy(&c_takes, "", None).unwrap(), c_takes.join(name));
        // Into B's watch, already holding B's take of that name.
        let into_b = a.save_copy(&b_takes, "", None).unwrap();
        assert_eq!(into_b, b_takes.join(format!("{name} 2")));
        assert_eq!(fs::read(into_b.join("audio-001.wav")).unwrap(), a_sound);
        // Saved again there, it goes on into its own folder.
        assert_eq!(a.save_copy(&b_takes, "", None).unwrap(), into_b);
        assert_eq!(a.claim_first_copy(&b_takes, name).unwrap(), into_b);
        // Named for yet another watch, it is given a folder there.
        assert_eq!(
            a.claim_first_copy(&base.join("D"), name).unwrap(),
            base.join("D").join(name)
        );
        assert_eq!(files(&b_dir), b_before);
        assert_eq!(fs::read(a_dir.join("audio-001.wav")).unwrap(), a_sound);
        drop((a, b));
        let _ = fs::remove_dir_all(base);
        let _ = fs::remove_dir_all(other);
    }

    /// Two takes started in the same minute, each named before either is
    /// saved (two benches), get a folder each: the second save never
    /// writes into the first take.
    #[test]
    fn two_takes_named_in_one_minute_get_a_folder_each() {
        let (mut a, base) = save_fixture("claim-a");
        let (mut b, other) = save_fixture("claim-b");
        let takes = base.join("Pepsi").join("takes");
        let first = a.claim_first_copy(&takes, "2026-11-25 06.52").unwrap();
        let second = b.claim_first_copy(&takes, "2026-11-25 06.52").unwrap();
        assert_eq!(first, takes.join("2026-11-25 06.52"));
        assert_eq!(second, takes.join("2026-11-25 06.52 2"));
        assert!(first.is_dir() && second.is_dir());
        let who = |r: &mut super::Recorder, w: &'static [u8]| {
            r.save_copy_with_files(&takes, "", None, &[("who.txt", w)])
                .unwrap()
        };
        assert_eq!(who(&mut a, b"a"), first);
        assert_eq!(who(&mut b, b"b"), second);
        assert_eq!(fs::read(first.join("who.txt")).unwrap(), b"a");
        assert_eq!(fs::read(second.join("who.txt")).unwrap(), b"b");
        drop((a, b));
        let _ = fs::remove_dir_all(base);
        let _ = fs::remove_dir_all(other);
    }

    /// Asked again before its first save is written (a save that failed,
    /// tried again), a take keeps the folder it was given and makes no
    /// other; given a folder in another watch's takes, it is given one
    /// there.
    #[test]
    fn a_take_named_again_keeps_its_folder() {
        let (mut r, base) = save_fixture("claim-again");
        let takes = base.join("Pepsi").join("takes");
        let dir = r.claim_first_copy(&takes, "2026-11-25 06.52").unwrap();
        assert_eq!(r.claim_first_copy(&takes, "2026-11-25 06.52").unwrap(), dir);
        assert_eq!(fs::read_dir(&takes).unwrap().count(), 1);
        let elsewhere = base.join("Explorer").join("takes");
        let moved = r.claim_first_copy(&elsewhere, "2026-11-25 06.52").unwrap();
        assert_eq!(moved, elsewhere.join("2026-11-25 06.52"));
        assert!(moved.is_dir());
        drop(r);
        let _ = fs::remove_dir_all(base);
    }
}
