//! A take's lift angle: whether its session file's was supplied, the
//! one rule every reader reads it by (#685, #2226), and a kept log read
//! at a lift ([`kept_at_lift`]). Split from `take.rs` by concern.

use super::*;

/// Whether the lift a session file `m` gives was supplied, or `None`
/// when it gives none: its `lift_known` where it says, else (an older
/// file, which did not record it) supplied unless it is the default
/// [`DEFAULT_LIFT_DEG`] and its calibre `cal` gives no lift (#685). Core,
/// the CLI and the desktop take it from here.
pub fn file_lift_known(m: &Manifest, cal: Option<&Calibre>) -> Option<bool> {
    let lift = m.lift?;
    Some(
        m.lift_known
            .unwrap_or(lift != DEFAULT_LIFT_DEG || cal.is_some_and(|c| c.lift_angle_deg.is_some())),
    )
}

/// A take's lift angle, whether it was supplied, and where it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiftSetting {
    pub lift_deg: f64,
    pub known: bool,
    pub from: SettingFrom,
}

/// The one rule for a take's lift angle (#2226), which core, the CLI and
/// the desktop's take opening ask (#2232): the lift `given` for this command, else the session
/// file `m`'s where [`file_lift_known`] says it was supplied, else its
/// calibre `cal`'s, else the lift the session file assumed, else
/// [`DEFAULT_LIFT_DEG`]. Only the last two are not known: an assumed lift
/// never outranks a calibre's.
pub fn lift_setting(
    given: Option<f64>,
    m: Option<&Manifest>,
    cal: Option<&Calibre>,
) -> LiftSetting {
    let set = |lift_deg, known, from| LiftSetting {
        lift_deg,
        known,
        from,
    };
    let kept = m.and_then(|m| m.lift);
    let supplied = m.and_then(|m| file_lift_known(m, cal)) == Some(true);
    match (given, kept, cal.and_then(|c| c.lift_angle_deg)) {
        (Some(l), ..) => set(l, true, SettingFrom::Given),
        (None, Some(l), _) if supplied => set(l, true, SettingFrom::Take),
        (None, _, Some(l)) => set(l, true, SettingFrom::Calibre),
        (None, Some(l), None) => set(l, false, SettingFrom::Take),
        (None, None, None) => set(DEFAULT_LIFT_DEG, false, SettingFrom::Default),
    }
}

/// A kept take's beat log with its amplitudes at `lift_deg`, the lift this
/// reading uses, where the analysis was kept at another: each window's
/// sides are read again from their edges at the new lift and gated there
/// ([`crate::amplitude::at_lift`]), so a side the kept lift culled can
/// come back (#1333), then screened for lone wrong-sound sides once over
/// the log ([`crate::amplitude::reject_side_outliers`]), as a read at
/// that lift is. Read for `escapement` (#2313,
/// [`crate::amplitude::kept_for`]), so a log kept for either escapement
/// reads as one read for this one.
pub fn kept_at_lift(
    log: &BeatLog,
    lift_deg: f64,
    escapement: crate::escapement::Escapement,
) -> BeatLog {
    let mut log = log.clone();
    if lift_deg > 0.0 && log.lift_deg > 0.0 && lift_deg != log.lift_deg && log.bph > 0 {
        // The balance's own period, as the windows were read (#2228).
        let osc = 2.0 * crate::timing::own_beat_period_s(&log.beats, log.bph);
        crate::amplitude::relift(
            &mut log.amplitude_windows,
            osc,
            log.lift_deg,
            lift_deg,
            escapement,
        );
        log.lift_deg = lift_deg;
    } else {
        crate::amplitude::kept_for(&mut log.amplitude_windows, escapement);
    }
    log.escapement = escapement;
    log
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A take's assumed lift gives way to its calibre's, and one it was
    /// given doesn't (#2226).
    #[test]
    fn a_calibres_lift_outranks_a_takes_assumed_one() {
        let cal = Calibre {
            name: "Test".into(),
            bph: None,
            lift_angle_deg: Some(59.0),
            lift_set_aside_deg: None,
            escape_teeth: None,
            escapement: None,
            wheels: Vec::new(),
            meshes: Vec::new(),
        };
        let file = |text: &str| -> Manifest { toml::from_str(text).unwrap() };
        let read = |m: &Manifest| {
            let cfg = stream_settings(m, Some(&cal), Given::default());
            let lift = lift_setting(None, Some(m), Some(&cal));
            assert_eq!(
                (cfg.analysis.amplitude.lift_deg, cfg.analysis.lift_known),
                (lift.lift_deg, lift.known)
            );
            (lift.lift_deg, lift.known, lift.from)
        };
        let assumed = file("lift = 52\nlift_known = false");
        assert_eq!(read(&assumed), (59.0, true, SettingFrom::Calibre));
        let supplied = file("lift = 52\nlift_known = true");
        assert_eq!(read(&supplied), (52.0, true, SettingFrom::Take));
        // With no calibre lift, the assumed one stands, unknown.
        assert_eq!(
            lift_setting(None, Some(&assumed), None),
            LiftSetting {
                lift_deg: 52.0,
                known: false,
                from: SettingFrom::Take
            }
        );
        assert_eq!(
            lift_setting(Some(48.0), Some(&assumed), Some(&cal)).from,
            SettingFrom::Given
        );
        assert_eq!(
            lift_setting(None, None, None),
            LiftSetting {
                lift_deg: DEFAULT_LIFT_DEG,
                known: false,
                from: SettingFrom::Default
            }
        );
    }
}
