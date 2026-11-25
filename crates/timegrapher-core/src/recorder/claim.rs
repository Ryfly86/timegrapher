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
use std::io;
use std::path::{Path, PathBuf};

impl Recorder {
    /// Give the first saved copy a take folder of its own in `parent` (a
    /// watch's takes), named `base`, or `base 2` and on where that is
    /// taken, rather than by its start, watch and calibre. The folder is
    /// made now ([`crate::library::make_unused_dir`]), so no other take
    /// is given it before the first save writes there. A copy already
    /// given its folder in `parent` (a first save that failed, tried
    /// again) keeps it and makes no other. Returns the copy's folder.
    pub fn claim_first_copy(&mut self, parent: &Path, base: &str) -> io::Result<PathBuf> {
        if let Some(dir) = self
            .saved_name
            .as_ref()
            .map(|n| parent.join(n))
            .filter(|d| d.is_dir())
        {
            return Ok(dir);
        }
        let dir = crate::library::make_unused_dir(parent, base)?;
        self.saved_name = dir.file_name().map(ToOwned::to_owned);
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::save_fixture;
    use std::fs;

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
