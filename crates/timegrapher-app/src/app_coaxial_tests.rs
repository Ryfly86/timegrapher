//! A co-axial calibre in the app (#2313, beta): what it is read for, live
//! and opened, and what its card says.

use super::*;
use timegrapher_core::escapement::Escapement::{CoAxial, SwissLever};

/// A co-axial calibre picked is listened to as one: a session started
/// with it, and one listening when it is picked, read for a co-axial; a
/// lever's picked after it reads for a lever again.
#[test]
fn a_co_axial_calibre_is_listened_to_as_one() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    assert_eq!(app.escapement(), SwissLever);
    app.set_calibre("Omega 3861".into());
    assert_eq!(app.escapement(), CoAxial);
    assert_eq!(app.live_config().analysis.amplitude.escapement, CoAxial);
    app.reset_session(48000, "syn".into());
    let read_for = |app: &TimegrapherApp| {
        app.bench
            .recording
            .live
            .as_ref()
            .unwrap()
            .config()
            .analysis
            .amplitude
            .escapement
    };
    assert_eq!(read_for(&app), CoAxial);
    app.set_calibre("ETA 2824-2".into());
    app.keep_escapement();
    assert_eq!(read_for(&app), SwissLever);
    app.set_calibre("Omega 8500".into());
    app.keep_escapement();
    assert_eq!(read_for(&app), CoAxial);
}

/// A take's session file's `escapement` outranks its calibre's, as core
/// reads it, and is saved back with the take; a microphone setup put back
/// has none.
#[test]
fn a_session_files_escapement_outranks_the_calibres() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    let mic = app.watch_setup();
    let m = Manifest {
        calibre: Some("ETA 2824-2".into()),
        escapement: Some(CoAxial),
        ..Default::default()
    };
    app.apply_manifest_placed(&m, &[], None, None);
    assert_eq!(app.escapement(), CoAxial);
    assert_eq!(app.manifest_header().escapement, Some(CoAxial));
    let m = Manifest {
        calibre: Some("Omega 3861".into()),
        escapement: Some(SwissLever),
        ..Default::default()
    };
    app.apply_manifest_placed(&m, &[], None, None);
    assert_eq!(app.escapement(), SwissLever);
    // A file with no key: the calibre's.
    let m = Manifest {
        calibre: Some("Omega 3861".into()),
        ..Default::default()
    };
    app.apply_manifest_placed(&m, &[], None, None);
    assert_eq!(app.escapement(), CoAxial);
    assert_eq!(app.manifest_header().escapement, None);
    app.set_watch_setup(mic);
    assert_eq!(app.bench.take.escapement_set, None);
    assert_eq!(app.escapement(), SwissLever);
}

/// A co-axial's card sets its lift angle as a lever's does, where a pin
/// lever's or a detent's says it is not supported in its place; its
/// calibre says how it is read.
#[test]
fn a_co_axial_calibres_lift_is_set_as_a_levers() {
    let mut app = TimegrapherApp::with_devices(Vec::new(), None);
    app.set_calibre("Omega 3861".into());
    assert!(app.escapement_unsupported().is_none());
    assert!(app.lift_settable(), "the 3861 has no lift in the table");
    let x = app.cal.find("Omega 3861").unwrap().escapement().unwrap();
    assert_eq!(x.tag(), timegrapher_core::calibres::Escapement::COAXIAL_TAG);
    assert_eq!(x.sentence(), timegrapher_core::calibres::COAXIAL_SENTENCE);
}

/// The amplitude's co-axial caveat keeps the owner's full words wherever
/// they fit, and tries core's shorter ones before the count (Hugh on
/// #2319).
#[test]
fn the_estimate_chip_tries_shorter_words_before_the_count() {
    use timegrapher_core::measure::{Caveat, COAXIAL_ESTIMATE_SHORTER};
    let c = Caveat::CoaxialEstimate;
    let one = [(c.short(52.0, 0).unwrap(), c.long(None).unwrap())];
    assert_eq!(one[0].0, "co-axial escapement estimate (beta)");
    let words = |room: usize| readouts::limits_chip_words(&one, |w| w.len() <= room);
    assert_eq!(words(99), "Co-axial Escapement Estimate (beta)");
    assert_eq!(words(30), "Co-axial Estimate (beta)");
    assert_eq!(words(10), "1 limit");
    assert_eq!(
        COAXIAL_ESTIMATE_SHORTER.len(),
        "Co-axial Estimate (beta)".len()
    );
    let two = [one[0].clone(), one[0].clone()];
    let words = |room: usize| readouts::limits_chip_words(&two, |w| w.len() <= room);
    assert_eq!(words(30), "Co-axial Estimate (beta) +1");
}
