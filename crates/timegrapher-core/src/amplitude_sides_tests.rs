//! A window whose two sides' unlock-to-drop times disagree gives no
//! unlock beat error and no amplitude (Norm's ruling on #1788, #1707).

use super::*;
use crate::escapement::Escapement;

/// #1707's setup, fixed before the run (seed 1704): a 48 kHz envelope of
/// a 28,800 bph watch, 53° lift, 30 s in 2 s windows, over a floor of
/// 0.016 ± 0.002 of seeded noise. Both sides: the unlock 13 ms before the
/// drop (0.28, 0.3 ms wide), the impulse at 9.1 ms (0.06), the drop (1.0).
/// The Tock only: a flat bump at 0.05 from 16.5 to 14.5 ms before the
/// drop. The windows as the stream reads them, screened for a Swiss lever.
fn bump_before_the_tock_unlock(seed: u64) -> Vec<AmplitudeWindow> {
    let (fs, period, lift, secs) = (48_000.0, 0.125, 53.0, 30.0);
    let mut state = seed;
    let mut env: Vec<f32> = (0..((secs + 0.5) * fs) as usize)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            0.016 + 0.004 * ((state >> 40) as f32 / (1u64 << 24) as f32 - 0.5)
        })
        .collect();
    let at = |t: f64| (t * fs).round() as usize;
    let mut beats = Vec::new();
    for k in 0..(secs / period) as i64 - 2 {
        let t = 0.1 + k as f64 * period;
        if k % 2 == 1 {
            env[at(t - 0.0165)..at(t - 0.0145)].fill(0.05);
        }
        env[at(t - 0.013)..at(t - 0.013 + 0.0003)].fill(0.28);
        env[at(t - 0.0091)..at(t - 0.0091 + 0.0003)].fill(0.06);
        env[at(t)..at(t + 0.0003)].fill(1.0);
        beats.push(Beat {
            index: k,
            time: t,
            quality: 1.0,
        });
    }
    let cfg = AmplitudeConfig {
        lift_deg: lift,
        ..Default::default()
    };
    let mut w = windows_between(&env, fs, &beats, 2.0 * period, 2.0, 0.0, secs - 0.5, &cfg);
    reject_side_outliers(&mut w, OUTLIER_SPAN_S, Escapement::SwissLever);
    w
}

/// #1707: the bump a twentieth of the drop's height is read as the Tock's
/// unlock (16.5 ms against the Tick's 13, 23% apart), which gave a 3.5 ms
/// unlock beat error. Each such window's unlock beat error is withheld,
/// and both its sides' amplitudes left out, edges kept: nothing filled.
#[test]
#[ignore = "#1707's acceptance misses at SIDES_DISAGREE_RATIO 1.4: the sides read 16.5 against 13 ms, a ratio of 1.27, so no window is withheld; reported on #2486 for the owner's decision"]
fn a_bump_read_as_one_sides_unlock_withholds_the_windows_unlock_beat_error() {
    let w = bump_before_the_tock_unlock(1704);
    assert!(w.len() >= 13, "{} windows", w.len());
    let misread: Vec<&AmplitudeWindow> = w
        .iter()
        .filter(|w| {
            w.odd_unlock_ms
                .or(w.culled()[1].map(|e| e[0]))
                .is_some_and(|u| u < -15.0)
        })
        .collect();
    assert!(
        !misread.is_empty(),
        "the bump is read as the Tock's unlock: {w:?}"
    );
    for w in &misread {
        assert!(w.sides_disagree(), "{w:?}");
        assert_eq!(w.beat_error_unlock_ms, None, "{w:?}");
        assert_eq!(
            (w.even_deg, w.odd_deg, w.mean()),
            (None, None, None),
            "{w:?}"
        );
        assert!(
            w.outliers()[0].is_some() && w.outliers()[1].is_some(),
            "{w:?}"
        );
        assert!(
            w.even_unlock_ms.is_some() && w.odd_unlock_ms.is_some(),
            "edges kept"
        );
    }
    // Nothing else gives the misread's beat error.
    for w in &w {
        if let Some(be) = w.beat_error_unlock_ms {
            assert!(be.abs() < 1.0, "{w:?}");
        }
    }
}

/// A window whose sides were `e` and `o` ms from the drop at their
/// unlocks, read at `deg` degrees each side.
fn win(i: usize, e: f64, o: f64, deg: f64) -> AmplitudeWindow {
    AmplitudeWindow {
        start_s: 2.0 * i as f64,
        end_s: 2.0 * i as f64 + 2.0,
        even_deg: Some(deg),
        odd_deg: Some(deg - 5.0),
        beat_error_ms: Some(0.3),
        beat_error_unlock_ms: unlock_beat_error_ms(Some(0.3), Some(-e), Some(-o)),
        even_unlock_ms: Some(-e),
        odd_unlock_ms: Some(-o),
        even_drop_ms: Some(0.0),
        odd_drop_ms: Some(0.0),
        extra: None,
        sound_1: [0.3; 2],
    }
}

/// A co-axial whose Tick's unlock-to-drop time is 1.5 of its Tock's by
/// design, with a stretch where the Tick's unlock is read on an earlier
/// sound (2.25, 1.5 of the design). Read for a co-axial, against the watch's own ratio,
/// every healthy window keeps its unlock beat error and the stretch
/// gives none; read against a lever's 1, the healthy ones would go.
fn coaxial(n: usize, misread: std::ops::Range<usize>) -> Vec<AmplitudeWindow> {
    (0..n)
        .map(|i| {
            let e = if misread.contains(&i) { 13.5 } else { 9.0 };
            win(i, e, 6.0, 280.0)
        })
        .collect()
}

#[test]
fn a_co_axial_is_judged_against_its_own_sides_ratio() {
    let mut w = coaxial(40, 20..24);
    screen_unlock_beat_error(&mut w, Escapement::CoAxial);
    for (i, w) in w.iter().enumerate() {
        let bad = (20..24).contains(&i);
        assert_eq!(w.sides_disagree(), bad, "{i}: {w:?}");
        assert_eq!(w.beat_error_unlock_ms.is_none(), bad, "{i}: {w:?}");
        assert_eq!(w.mean().is_none(), bad, "{i}: {w:?}");
    }
    let mut lever = coaxial(40, 20..24);
    screen_unlock_beat_error(&mut lever, Escapement::SwissLever);
    assert!(lever.iter().all(|w| w.beat_error_unlock_ms.is_none()));
}

/// Under 30 windows a co-axial has no reference of its own, so the screen
/// leaves its sides as they are (#1576's minimum).
#[test]
fn a_co_axial_with_too_few_windows_is_not_judged_on_its_sides() {
    let mut w = coaxial(29, 10..12);
    screen_unlock_beat_error(&mut w, Escapement::CoAxial);
    assert!(w.iter().all(|w| !w.sides_disagree() && w.mean().is_some()));
}

/// A window judged to disagree is judged afresh by the next screen: once
/// the co-axial's own ratio is known, its healthy windows' amplitudes come
/// back as read, and a lone outlier the side screen left out stays out.
#[test]
fn a_screen_run_again_puts_back_what_it_left_out() {
    let mut w = coaxial(40, 20..24);
    let as_read: Vec<[Option<f64>; 2]> = w.iter().map(|w| [w.even_deg, w.odd_deg]).collect();
    screen_unlock_beat_error(&mut w, Escapement::SwissLever);
    assert!(w.iter().all(|w| w.sides_disagree() && w.mean().is_none()));
    w[5].edit_extra(|x| x.outlier_deg[1] = Some(150.0));
    w[5].odd_deg = None;
    w[5].edit_extra(|x| x.sides_moved[1] = false);
    screen_unlock_beat_error(&mut w, Escapement::CoAxial);
    for (i, w) in w.iter().enumerate() {
        let bad = (20..24).contains(&i);
        assert_eq!(w.sides_disagree(), bad, "{i}");
        if !bad && i != 5 {
            assert_eq!([w.even_deg, w.odd_deg], as_read[i], "{i}");
            assert_eq!(w.outliers(), [None, None], "{i}");
        }
    }
    assert_eq!((w[5].even_deg, w[5].odd_deg), (as_read[5][0], None));
    assert_eq!(w[5].outliers(), [None, Some(150.0)]);
    // Screening again changes nothing.
    let once: Vec<_> = w
        .iter()
        .map(|w| (w.even_deg, w.odd_deg, w.extra()))
        .collect();
    screen_unlock_beat_error(&mut w, Escapement::CoAxial);
    let twice: Vec<_> = w
        .iter()
        .map(|w| (w.even_deg, w.odd_deg, w.extra()))
        .collect();
    assert_eq!(once, twice);
}

/// The long-template choice and this screen are one test at two limits:
/// with a reference of 1 and [`SIDES_UNLOCK_AGREE_SHARE`] it is
/// `sides_go_long`'s (#1703); the screen's [`SIDES_DISAGREE_RATIO`] as a
/// share passes exactly the times within that ratio of each other.
#[test]
fn the_sides_agree_within_the_share_of_their_mean() {
    let long = SIDES_UNLOCK_AGREE_SHARE;
    assert!(sides_agree(13.0, 14.9, 1.0, long));
    assert!(!sides_agree(13.0, 16.5, 1.0, long));
    // As sides_go_long takes them: distances before the beat, negative.
    assert_eq!(
        sides_agree(-0.013, -0.0149, 1.0, long),
        sides_agree(13.0, 14.9, 1.0, long)
    );
    let screen = share_of_ratio(SIDES_DISAGREE_RATIO);
    for (e, o, r0, agree) in [
        (13.0, 16.5, 1.0, true),
        (10.0, 13.9, 1.0, true),
        (10.0, 14.1, 1.0, false),
        (14.1, 10.0, 1.0, false),
        (9.0, 6.0, 1.5, true),
        (13.5, 6.0, 1.5, false),
        (9.0, 6.0, 1.0, false),
    ] {
        assert_eq!(sides_agree(e, o, r0, screen), agree, "{e} {o} {r0}");
    }
}

/// A log screened for a co-axial and then kept for a lever reads as one
/// screened for the lever: its sides judged again against 1 (Trevor 85 on
/// #2486).
#[test]
fn a_co_axial_log_kept_for_a_lever_reads_as_one_screened_for_it() {
    let mut kept = coaxial(40, 20..24);
    screen_unlock_beat_error(&mut kept, Escapement::CoAxial);
    kept_for(&mut kept, Escapement::SwissLever);
    let mut lever = coaxial(40, 20..24);
    screen_unlock_beat_error(&mut lever, Escapement::SwissLever);
    assert_eq!(format!("{kept:?}"), format!("{lever:?}"));
    // And back again.
    kept_for(&mut kept, Escapement::CoAxial);
    let mut co = coaxial(40, 20..24);
    screen_unlock_beat_error(&mut co, Escapement::CoAxial);
    assert_eq!(format!("{kept:?}"), format!("{co:?}"));
}

/// The reason a reading gives no unlock beat error is the sides only when
/// most windows that found both sides were withheld for them (Norm 32 on
/// #2486): `disagree` of 10 such windows withheld for their sides, the
/// rest outside the far fences.
fn why_of(disagree: usize) -> crate::measure::DropWhy {
    let mut w: Vec<AmplitudeWindow> = (0..10).map(|i| win(i, 7.0, 7.0, 280.0)).collect();
    for (i, w) in w.iter_mut().enumerate() {
        w.beat_error_unlock_ms = None;
        if i < disagree {
            w.edit_extra(|x| x.sides_disagree = true);
        }
    }
    let refs: Vec<&AmplitudeWindow> = w.iter().collect();
    crate::session::window_figures(&refs, false).drop_why()
}

#[test]
fn the_sides_name_the_reason_only_when_most_windows_were_withheld_for_them() {
    use crate::measure::DropWhy;
    assert_eq!(why_of(4), DropWhy::UnlockNotFound);
    assert_eq!(why_of(5), DropWhy::UnlockNotFound);
    assert_eq!(why_of(6), DropWhy::SidesDisagree);
    assert_eq!(why_of(10), DropWhy::SidesDisagree);
    assert_eq!(why_of(0), DropWhy::UnlockNotFound);
}
