//! `long`'s text and report give its rates as signed figures, the slice
//! range included (#734).
use std::process::Command;

#[test]
fn the_slice_range_reads_as_signed_rates() {
    let dir = std::env::temp_dir().join(format!("tg-long-text-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| run_in(&dir, args);
    // The synthetic watch runs +5 s/d, so every slice reads fast.
    let wav = dir.join("watch.wav");
    let wav = wav.to_str().unwrap();
    run(&["synth", wav, "--duration", "40"]);
    let out = dir.join("long");
    let text = run(&["long", wav, "--out", out.to_str().unwrap()]);
    let line = text
        .lines()
        .find(|l| l.starts_with("Rate "))
        .unwrap_or_else(|| panic!("no rate line in {text}"));
    assert!(line.contains("s/d over the run; "), "{line}");
    assert!(line.contains(" slices from +"), "{line}");
    assert!(line.contains(" to +"), "{line}");
    let html = std::fs::read_to_string(out.join("report.html")).unwrap();
    let row = html
        .lines()
        .find(|l| l.starts_with("<tr><th>Rate</th>"))
        .expect("the report's rate row");
    assert!(row.contains(" slices from +"), "{row}");
    assert!(row.contains(" to +"), "{row}");
    // Each search says the shortest cycle it tried, in the text and the
    // report, and the dashed line is a rate over searches (#1897). On 40 s
    // the rate is searched and the amplitude is not, which is said, never
    // "none named" (#1880).
    use timegrapher_core::longterm::TOO_SHORT_TO_SEARCH;
    for (out, shortest) in [
        (&text, "  Shortest cycle searched: "),
        (&html, "Shortest cycle searched: "),
    ] {
        assert_eq!(out.matches(shortest).count(), 1, "{out}");
        assert_eq!(out.matches(TOO_SHORT_TO_SEARCH).count(), 1, "{out}");
    }
    assert_eq!(text.matches("none named").count(), 1, "{text}");
    assert_eq!(html.matches("No periodic change").count(), 1);
    assert!(
        html.contains(&timegrapher_core::steady_views::threshold_meaning(
            timegrapher_core::longterm::LongConfig::default().false_alarm
        ))
    );
    assert!(
        !html.contains("too short to search."),
        "one fact, one place"
    );
    assert!(!html.contains("chance of being noise"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rate line in `long` gives no share of the variation, its size names
/// its quantity at the difference place, the reasons are said once, and the
/// wheel's match rule is said as its wider part (Norm on #2100; #2342
/// item 11). Amplitude lines keep their share.
#[test]
fn rate_lines_give_no_share_and_name_their_swing() {
    use timegrapher_core::session::{timing_swing_text, RATE_SHARE_NOTE, RATE_SHARE_WHY};
    let dir = std::env::temp_dir().join(format!("tg-long-share-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
            .env("TIMEGRAPHER_CONFIG_DIR", &dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    // A fault every 20 s moves both the timing and the amplitude.
    let wav = dir.join("watch.wav");
    let wav = wav.to_str().unwrap();
    run(&[
        "synth",
        wav,
        "--duration",
        "600",
        "--snr",
        "24",
        "--fault-period",
        "20",
        "--fault-length",
        "4",
    ]);
    let out = dir.join("long");
    let text = run(&[
        "long",
        wav,
        "--wheel",
        "a wheel=20",
        "--out",
        out.to_str().unwrap(),
    ]);
    let (rate, amplitude) = text
        .split_once("Periodic changes in amplitude:")
        .expect("both searches");
    let rate = rate
        .split_once("Periodic changes in rate")
        .expect("the rate search")
        .1;
    let rate_rows = rows(rate);
    let amplitude_rows = rows(amplitude);
    assert!(
        !rate_rows.is_empty() && !amplitude_rows.is_empty(),
        "{text}"
    );
    for row in &rate_rows {
        assert!(
            row.contains("timing swing ") && row.contains(" ms peak to peak"),
            "{row}"
        );
        assert!(row.contains("explains    –"), "{row}");
        assert!(!row.contains('%') || row.contains("% off"), "{row}");
    }
    assert!(
        rate_rows[0].contains("a match is within the search's resolution, ±"),
        "{}",
        rate_rows[0]
    );
    assert!(
        amplitude_rows[0].contains("explains 100%"),
        "{}",
        amplitude_rows[0]
    );
    assert!(
        amplitude_rows[0].contains(" deg peak to peak"),
        "{}",
        amplitude_rows[0]
    );
    assert_eq!(text.matches(RATE_SHARE_NOTE).count(), 1, "{text}");
    assert_eq!(text.matches(&timing_swing_text()).count(), 1, "{text}");
    assert!(!text.contains(" or 1%"), "{text}");
    // The report: only the amplitude cycle gives a share.
    let html = std::fs::read_to_string(out.join("report.html")).unwrap();
    let shares = html.matches("% of the detrended variation").count();
    assert_eq!(shares, amplitude_rows.len(), "{html}");
    assert!(html.contains(" ms peak to peak over each turn"));
    assert_eq!(html.matches(RATE_SHARE_WHY).count(), 1);
    assert_eq!(html.matches(&timing_swing_text()).count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runs the CLI with its config in `dir`, and gives its stdout.
fn run_in(dir: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .env("TIMEGRAPHER_CONFIG_DIR", dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The cycle lines of one part of `long`'s text.
fn rows(part: &str) -> Vec<String> {
    part.lines()
        .filter(|l| l.contains(" s ±"))
        .map(str::to_string)
        .collect()
}

/// With no rate line in ms, the ms note is not said, in the text or the
/// report (Trevor 78 on #2472).
#[test]
fn no_ms_rate_line_no_ms_note() {
    use timegrapher_core::session::{timing_swing_text, RATE_SHARE_NOTE};
    let dir = std::env::temp_dir().join(format!("tg-long-slow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let run = |args: &[&str]| run_in(&dir, args);
    // A fault every 60 s gives a rate line in s/d and none in ms.
    let wav = dir.join("slow.wav");
    let wav = wav.to_str().unwrap();
    run(&[
        "synth",
        wav,
        "--duration",
        "600",
        "--snr",
        "24",
        "--fault-period",
        "60",
    ]);
    let out = dir.join("slow_long");
    let text = run(&[
        "long",
        wav,
        "--wheel",
        "fourth wheel=60",
        "--out",
        out.to_str().unwrap(),
    ]);
    let rate = text
        .split_once("Periodic changes in amplitude:")
        .expect("both searches")
        .0;
    let rate_rows = rows(rate);
    assert!(
        !rate_rows.is_empty() && rate_rows.iter().all(|r| r.contains(" s/d peak to peak")),
        "{text}"
    );
    assert_eq!(text.matches(RATE_SHARE_NOTE).count(), 1, "{text}");
    assert!(!text.contains(&timing_swing_text()), "{text}");
    let html = std::fs::read_to_string(out.join("report.html")).unwrap();
    assert!(!html.contains(&timing_swing_text()));
    let _ = std::fs::remove_dir_all(&dir);
}
