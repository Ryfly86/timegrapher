//! Which stretches of a take settle right through and read nothing
//! (Norm on #1980): split from `take.rs` by concern.

use super::*;

/// Whether entry `e`, a position of `length_s` in the take `m`, settling
/// for [`SETTLE_S`] as none is given, settles right through and reads
/// nothing: one of several stretches of one recording ([`cut_from_one`])
/// no longer than [`SETTLE_S`]. Such a stretch, a position pressed and left
/// at once, is the watch being handled, not a position with a rate (Norm
/// on #1980). A recording of its own keeps [`measured`]'s halving, so a
/// short take still reads, alone or in a session of such takes.
pub fn settles_through(e: &RecordingEntry, m: &Manifest, length_s: f64) -> bool {
    cut_from_one(e, m) && length_s <= SETTLE_S
}

/// Whether entry `e` is one of several stretches the take `m` cuts from
/// one recording: another entry names its file too. Such a stretch settles
/// in full, never halved, as the watch was turned within the recording.
pub fn cut_from_one(e: &RecordingEntry, m: &Manifest) -> bool {
    m.recordings.iter().filter(|r| r.file == e.file).count() > 1
}

/// Whether entry `e` of the take `m` reads nothing as its session file
/// lists it, read with no settling given: [`reads_nothing`], or a stretch
/// with both ends listed that [`settles_through`] with no settling of its
/// own or carried on. Its position is then not one of the take's.
pub fn listed_unread(e: &RecordingEntry, m: &Manifest) -> bool {
    if reads_nothing(e) {
        return true;
    }
    let (Some(a), Some(b)) = (e.start_s, e.end_s) else {
        return false;
    };
    let settle = e
        .settle_s
        .or(continues_position(e, m).then_some(0.0))
        .or(entry_times(e, m).0);
    settle.is_none() && settles_through(e, m, b - a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(position: &str, start_s: f64, end_s: Option<f64>) -> RecordingEntry {
        RecordingEntry {
            file: ".".into(),
            position: Some(position.into()),
            start_s: Some(start_s),
            end_s,
            ..Default::default()
        }
    }

    /// A position pressed and left at once, in a take of more than one,
    /// reads nothing and is not one of its positions; a take of one entry
    /// still reads a short stretch, and a stretch past the settle reads
    /// from its end (Norm on #1980).
    #[test]
    fn a_stretch_no_longer_than_the_settle_reads_nothing_in_a_take_of_several() {
        let m = Manifest {
            recordings: vec![entry("CU", 0.0, Some(2.0)), entry("DD", 2.0, None)],
            ..Default::default()
        };
        let [cu, dd] = [&m.recordings[0], &m.recordings[1]];
        assert!(listed_unread(cu, &m) && !listed_unread(dd, &m));
        assert!(matches!(
            reading_window(cu, &m, None, 600.0),
            Err(NoReading::Nothing)
        ));
        let w = reading_window(dd, &m, None, 600.0).unwrap();
        assert_eq!((w.from_s, w.to_s), (2.0 + SETTLE_S, 600.0));
        // At the settle exactly, nothing; just past it, what is past it.
        let at = |len: f64| Manifest {
            recordings: vec![entry("CU", 0.0, Some(len)), entry("DD", len, None)],
            ..Default::default()
        };
        assert!(listed_unread(&at(SETTLE_S).recordings[0], &at(SETTLE_S)));
        let m = at(SETTLE_S + 11.0);
        let w = reading_window(&m.recordings[0], &m, None, 600.0).unwrap();
        assert_eq!((w.from_s, w.to_s), (SETTLE_S, SETTLE_S + 11.0));
        // A settle given stands: the reader's, or the session file's.
        let m = at(2.0);
        assert!(reading_window(&m.recordings[0], &m, Some(0.0), 600.0).is_ok());
        let given = Manifest {
            settle_s: Some(0.0),
            ..at(2.0)
        };
        assert!(!listed_unread(&given.recordings[0], &given));
        // Takes of their own, each its own file, in one session file
        // halve theirs as each did alone (`timegrapher record`).
        let own = |file: &str| RecordingEntry {
            file: file.into(),
            ..entry("DU", 0.0, Some(2.0))
        };
        let apart = Manifest {
            recordings: vec![own("a"), own("b")],
            ..Default::default()
        };
        assert!(!listed_unread(&apart.recordings[0], &apart));
        let w = reading_window(&apart.recordings[0], &apart, None, 600.0).unwrap();
        assert_eq!((w.from_s, w.to_s), (1.0, 2.0));
        // One entry alone halves its settle, so a short take reads.
        let one = Manifest {
            recordings: vec![entry("DU", 0.0, Some(2.0))],
            ..Default::default()
        };
        assert!(!listed_unread(&one.recordings[0], &one));
        let w = reading_window(&one.recordings[0], &one, None, 600.0).unwrap();
        assert_eq!((w.from_s, w.to_s), (1.0, 2.0));
    }
}
