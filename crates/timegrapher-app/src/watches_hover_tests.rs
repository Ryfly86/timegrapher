//! The Watches takes row's hover and its one-line facts: what a take's
//! figures are of, and what read them. Split from `watches.rs` by concern.

use super::*;

/// A take's hover names the engine that read its kept reading, and
/// says nothing of an engine while it is being read or couldn't be
/// (#1521, Hugh).
#[test]
fn a_takes_hover_names_the_engine_that_read_it() {
    let (version, source) = timegrapher_core::library::engine();
    let line = format!("Read by Timegrapher {version}, engine {source}");
    assert_eq!(engine_line(), line);
    let take = Take {
        name: "test".into(),
        bytes: 0,
        audio: true,
        incomplete: false,
        started_utc: None,
        named_start: None,
        duration_s: None,
        positions: Vec::new(),
        at_end: None,
        power_reserve: None,
        wind_h: None,
    };
    let dir = PathBuf::from("watch/test");
    let read = Reading {
        position: Some("DU".into()),
        rate_s_per_day: Some(3.5),
        calibrated: false,
        amplitude_deg: Some(281.0),
        beat_error_ms: Some(0.4),
        beat_error_source: None,
        caveats: Vec::new(),
        watch_heard: true,
    };
    let hover = |state: TakeReading| {
        let readings = TakeReadings::known([(dir.clone(), 0, state)]);
        let cx = TakesContext {
            others: &[],
            writable: true,
            keeping: None,
            unsorted: false,
            picked: None,
            on_show: None,
            held: None,
            writing: None,
            readings: &readings,
        };
        take_hover(&take, &cx, &dir, None, readings.reading(&dir))
    };
    let kept = hover(TakeReading::Read(Ok(read.clone())));
    assert!(kept.contains(&format!("\n\n{line}\n\n")), "{kept}");
    for state in [
        TakeReading::Reading,
        TakeReading::Read(Err("no beats".into())),
    ] {
        let h = hover(state);
        assert!(!h.contains("Read by Timegrapher"), "{h}");
    }
}

/// A take of more than one position says which position its figures
/// are, and that they are its whole stretch after settling, not the
/// Bench's at Stop; its facts no longer run them on after every
/// position's name (#1979).
#[test]
fn a_takes_hover_names_the_position_its_figures_are_of() {
    let take = Take {
        name: "test".into(),
        bytes: 0,
        audio: true,
        incomplete: false,
        started_utc: None,
        named_start: None,
        duration_s: Some(120.0),
        positions: vec!["DU".into(), "CU".into()],
        at_end: None,
        power_reserve: None,
        wind_h: None,
    };
    let read = Reading {
        position: Some("CU".into()),
        rate_s_per_day: Some(-12.8),
        calibrated: false,
        amplitude_deg: Some(220.0),
        beat_error_ms: Some(0.03),
        beat_error_source: None,
        caveats: Vec::new(),
        watch_heard: true,
    };
    let dir = PathBuf::from("watch/test");
    let hover = |t: &Take| {
        let readings = TakeReadings::known([(dir.clone(), 0, TakeReading::Read(Ok(read.clone())))]);
        let cx = TakesContext {
            others: &[],
            writable: true,
            keeping: None,
            unsorted: false,
            picked: None,
            on_show: None,
            held: None,
            writing: None,
            readings: &readings,
        };
        take_hover(t, &cx, &dir, None, readings.reading(&dir))
    };
    let h = hover(&take);
    let first = h.lines().next().unwrap();
    assert!(!first.contains("s/d"), "{h}");
    assert!(
        h.contains(
            "\nCrown Up, its whole stretch after settling: \u{2212}12.8 s/d · 220° · 0.03 ms"
        ),
        "{h}"
    );
    let one = Take {
        positions: vec!["DU".into()],
        ..take.clone()
    };
    let h = hover(&one);
    assert!(
        h.contains("\nThe whole take after settling: \u{2212}12.8 s/d"),
        "{h}"
    );
    // A Positions Test's figures are its mean, said apart.
    let readings = TakeReadings::known([(dir.clone(), 0, TakeReading::Read(Ok(read.clone())))]);
    let cx = TakesContext {
        others: &[],
        writable: true,
        keeping: None,
        unsorted: false,
        picked: None,
        on_show: None,
        held: None,
        writing: None,
        readings: &readings,
    };
    let h = take_hover(&take, &cx, &dir, Some(2), readings.reading(&dir));
    assert!(!h.contains("after settling"), "{h}");
}

/// The row's hover says how many positions a test's figures are the
/// mean of: six only when six were measured.
#[test]
fn a_tests_hover_counts_the_positions_measured() {
    assert!(figures_hover(Some(6), false).contains("Mean of the six positions"));
    assert!(figures_hover(Some(2), false).contains("Mean of the 2 positions measured"));
    assert!(figures_hover(Some(1), false).contains("the one position measured"));
    assert_eq!(
        figures_hover(None, true),
        "\n\nRate, Amplitude and Beat Error are given hour by hour in the take: open it to \
         see them."
    );
    assert_eq!(figures_hover(None, false), "");
}

#[test]
fn a_take_line_says_what_it_measured_with_beat_error_as_a_size() {
    let t = Take {
        name: "2026-10-20 14.32 Dial Up".into(),
        bytes: 1_900_000,
        audio: false,
        incomplete: true,
        started_utc: Some("2026-10-20T21:32:00Z".into()),
        named_start: None,
        duration_s: Some(120.0),
        positions: vec!["DU".into(), "CL".into()],
        at_end: None,
        power_reserve: None,
        wind_h: None,
    };
    let reading = Reading {
        position: Some("CL".into()),
        rate_s_per_day: Some(3.5),
        calibrated: true,
        amplitude_deg: Some(281.0),
        beat_error_ms: Some(-0.4),
        beat_error_source: None,
        caveats: Vec::new(),
        watch_heard: true,
    };
    let r = Some(&reading);
    let line = take_line(&t, None, r);
    assert!(line.starts_with("Dial Up, Crown Left · "), "{line}");
    assert!(
        line.contains("+3.5 s/d · 281° · 0.40 ms · Analysis Only · Incomplete"),
        "{line}"
    );
    // Until the reading is worked out, no figures.
    let line = take_line(&t, None, None);
    assert!(!line.contains("s/d") && !line.contains('°'), "{line}");
    // A Power Reserve take leads with its run, not its length.
    let r = Take {
        positions: vec!["CL".into()],
        power_reserve: Some(timegrapher_core::power_reserve::Summary {
            run_s: 166_320.0,
            ending: timegrapher_core::power_reserve::Ending::Stopped,
            position: "CL".into(),
        }),
        ..t.clone()
    };
    let line = take_line(&r, None, Some(&reading));
    assert!(
        line.starts_with("Power Reserve 46 h 12 min · Crown Left · Analysis Only"),
        "{line}"
    );
    // Nor does a Positions Test's last window stand for the test.
    let line = take_line(&t, Some(TakeTest::Positions), Some(&reading));
    assert!(!line.contains("+3.5") && !line.contains("281°"), "{line}");
    assert!(local_minute("2026-10-20T21:32:00Z").is_some_and(|m| m.len() == 16));
    assert_eq!(local_minute("not a time"), None);
}
#[test]
fn positions_take_hover_explains_the_actual_beat_error_mean() {
    use crate::guided::{FullWind, KeptRow, RESULTS_FILE, RESULTS_FORMAT};
    use timegrapher_core::session::BeatErrorSource as S;
    for legacy in [true, false] {
        let dir =
            std::env::temp_dir().join(format!("tg-source-hover-{}-{legacy}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let full = FullWind {
            wind_h: None,
            against: None,
            listened: false,
            format: RESULTS_FORMAT.into(),
            began_utc: String::new(),
            began_unix: 0.0,
            watch: String::new(),
            calibre: String::new(),
            bph: None,
            lift_angle_deg: None,
            settle_s: None,
            measure_s: None,
            positions: [("DU", S::Unlock), ("DD", S::Drop), ("CU", S::Unlock)]
                .into_iter()
                .map(|(p, source)| KeptRow {
                    position: p.into(),
                    rate_s_per_day: Some(1.0),
                    amplitude_deg: Some(270.0),
                    beat_error_ms: Some(0.4),
                    beat_error_source: if legacy { None } else { Some(source) },
                })
                .collect(),
        };
        std::fs::write(dir.join(RESULTS_FILE), full.to_json()).unwrap();
        let readings = TakeReadings::default();
        let cx = TakesContext {
            others: &[],
            writable: true,
            keeping: None,
            unsorted: false,
            picked: None,
            on_show: None,
            held: None,
            writing: None,
            readings: &readings,
        };
        let take = Take {
            name: "test".into(),
            bytes: 0,
            audio: false,
            incomplete: false,
            started_utc: None,
            named_start: None,
            duration_s: None,
            positions: vec!["DU".into(), "DD".into(), "CU".into()],
            at_end: None,
            power_reserve: None,
            wind_h: None,
        };
        let hover = take_hover(&take, &cx, &dir, Some(3), None);
        assert!(hover.contains("Rate and Amplitude: Mean of the 3 positions measured"));
        if legacy {
            assert_eq!(positions_means(&dir).unwrap().1[2], None);
            assert!(
                hover.contains("no mean") && hover.contains("leaves out 3 readings"),
                "{hover}"
            );
        } else {
            assert!(
                hover.contains("Beat Error from the Unlock: mean of 2 of 3 positions"),
                "{hover}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The take the session on the Bench writes, recording or saved, is
/// tagged On Show and its hover says it is on the Bench, as a take opened
/// there is, not only once it is opened (#1981).
#[test]
fn the_take_on_the_bench_is_tagged_on_show_while_its_session_writes_it() {
    let take = Take {
        name: "2026-11-18 19.56".into(),
        bytes: 0,
        audio: true,
        incomplete: false,
        started_utc: None,
        named_start: None,
        duration_s: Some(60.0),
        positions: vec!["DU".into()],
        at_end: None,
        power_reserve: None,
        wind_h: None,
    };
    let e = WatchEntry {
        dir: "/nowhere/Pepsi".into(),
        watch: timegrapher_core::library::Watch::new("Pepsi"),
        takes: vec![take.clone()],
        photos: Vec::new(),
    };
    let dir = e.take_dir(&take);
    let readings = TakeReadings::default();
    let cx = |on_show, held| TakesContext {
        others: &[],
        writable: true,
        keeping: None,
        unsorted: false,
        picked: None,
        on_show,
        held,
        writing: None,
        readings: &readings,
    };
    for (on_show, held) in [(Some(dir.as_path()), None), (None, Some(dir.as_path()))] {
        let cx = cx(on_show, held);
        assert_eq!(take_tags(&e, &take, &cx), "On Show");
        assert!(take_hover(&take, &cx, &dir, None, None).contains("on show on the Bench"));
    }
    let cx = cx(None, None);
    assert_eq!(take_tags(&e, &take, &cx), "");
}

/// A take still being recorded on the Bench is tagged Recording, and its
/// row gives no length, figures or size from its first write: a dim dash,
/// with a hover saying they come once it stops. Stopped, its row is as
/// any other's (Walter, #2027).
#[test]
fn a_take_still_being_recorded_shows_no_first_write_figures() {
    let take = Take {
        name: "2026-11-18 19.56".into(),
        bytes: 1_200_000,
        audio: true,
        incomplete: false,
        started_utc: None,
        named_start: None,
        duration_s: Some(5.0),
        positions: vec!["DU".into()],
        at_end: None,
        power_reserve: None,
        wind_h: None,
    };
    let e = WatchEntry {
        dir: "/nowhere/Pepsi".into(),
        watch: timegrapher_core::library::Watch::new("Pepsi"),
        takes: vec![take.clone()],
        photos: Vec::new(),
    };
    let dir = e.take_dir(&take);
    let read = Reading {
        rate_s_per_day: Some(5.4),
        ..Default::default()
    };
    let readings = TakeReadings::known([(dir.clone(), 1_200_000, TakeReading::Read(Ok(read)))]);
    let cx = |writing| TakesContext {
        others: &[],
        writable: true,
        keeping: None,
        unsorted: false,
        picked: None,
        on_show: None,
        held: Some(dir.as_path()),
        writing,
        readings: &readings,
    };
    let rec = cx(Some(dir.as_path()));
    assert_eq!(take_tags(&e, &take, &rec), "Recording");
    let (cells, dim) = row_cells(&take, &dir, &rec);
    assert!(dim);
    assert!(cells[1..].iter().all(|c| c == "—"), "{cells:?}");
    let h = take_hover(&take, &rec, &dir, None, readings.reading(&dir));
    assert!(h.contains("being recorded") && !h.contains("+5.4"), "{h}");
    // Stopped: its row is the take's own again.
    let done = cx(None);
    assert_eq!(take_tags(&e, &take, &done), "On Show");
    let (cells, dim) = row_cells(&take, &dir, &done);
    assert!(!dim);
    assert_eq!((cells[2].as_str(), cells[3].as_str()), ("0:05", "+5.4"));
}

/// A watch's size in the list is a dash while the Bench records into
/// one of its takes, as that take's row is: the library holds only the
/// take's first write until it stops. Another watch's, or once it stops,
/// its size (Walter 2 on #2032, #2034).
#[test]
fn a_watchs_size_waits_while_a_take_records_into_it() {
    let take = Take {
        name: "2026-11-19 05.40".into(),
        bytes: 3_900_000,
        audio: true,
        incomplete: false,
        started_utc: None,
        named_start: None,
        duration_s: Some(5.0),
        positions: vec!["DU".into()],
        at_end: None,
        power_reserve: None,
        wind_h: None,
    };
    let e = WatchEntry {
        dir: "/nowhere/Pepsi".into(),
        watch: timegrapher_core::library::Watch::new("Pepsi"),
        takes: vec![take.clone()],
        photos: Vec::new(),
    };
    let dir = e.take_dir(&take);
    let what = "Show this watch's details and takes.";
    assert_eq!(
        crate::size_wait::size_or_wait(&e.dir, Some(dir.as_path()), what, || list_size(e.bytes())),
        (
            "—".to_string(),
            "Show this watch's details and takes. Its size is given once the take being \
             recorded stops."
                .to_string()
        )
    );
    let other = std::path::Path::new("/nowhere/Pepsi 2/takes/2026-11-19 05.40");
    let shown = ("3.9 MB".to_string(), what.to_string());
    assert_eq!(
        crate::size_wait::size_or_wait(&e.dir, Some(other), what, || list_size(e.bytes())),
        shown
    );
    assert_eq!(
        crate::size_wait::size_or_wait(&e.dir, None, what, || list_size(e.bytes())),
        shown
    );
}

/// Open greyed out says the real reason first: with no takes, that
/// there are none, not to stop the Bench; with takes but none picked, to
/// pick one; only a picked take waits on the Bench (#2007).
#[test]
fn open_greyed_out_gives_the_real_reason() {
    let bench = Some("Stop the take on the Bench first.");
    let mut e = WatchEntry {
        dir: "/nowhere/Unsorted".into(),
        watch: timegrapher_core::library::Watch::new("Unsorted"),
        takes: Vec::new(),
        photos: Vec::new(),
    };
    assert_eq!(open_why(&e, false, bench), Some(NO_TAKES));
    assert_eq!(open_why(&e, false, None), Some(NO_TAKES));
    e.takes.push(Take {
        name: "2026-11-19 06.00".into(),
        bytes: 0,
        audio: true,
        incomplete: false,
        started_utc: None,
        named_start: None,
        duration_s: None,
        positions: Vec::new(),
        at_end: None,
        power_reserve: None,
        wind_h: None,
    });
    assert_eq!(open_why(&e, false, bench), Some("Pick a take first"));
    assert_eq!(open_why(&e, true, bench), bench);
    assert_eq!(open_why(&e, true, None), None);
}

/// A Positions Test stopped before any position was measured has no
/// Mean row: its row's figures are the dim dash, not blank cells. One
/// measured, its Mean shows (Walter on #2032, #2007).
#[test]
fn a_positions_test_with_nothing_measured_shows_a_dash() {
    let take = Take {
        name: "2026-11-19 06.10".into(),
        bytes: 0,
        audio: true,
        incomplete: false,
        started_utc: None,
        named_start: None,
        duration_s: Some(20.0),
        positions: vec!["DU".into()],
        at_end: None,
        power_reserve: None,
        wind_h: None,
    };
    let none = Some((TakeTest::Positions, [None; 3]));
    assert!(dashed(&take, none));
    let cells = take_cells(&take, none, None);
    assert_eq!(&cells[3..6], ["—", "—", "—"], "{cells:?}");
    let one = Some((TakeTest::Positions, [Some(4.0), Some(280.0), Some(0.3)]));
    assert!(!dashed(&take, one));
    assert!(!take_cells(&take, one, None)[3..6].contains(&"—".to_string()));
}
