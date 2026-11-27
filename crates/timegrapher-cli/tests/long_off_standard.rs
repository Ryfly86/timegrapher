//! `long` and `series` on Auto off every standard rate (#2404, the audit's
//! probe): 17,570 bph at +5 s/d gives no rate in s/d in any published
//! field, and the same sentence and caveat `analyze` gives; set at 17,570
//! it reads.
use std::{path::PathBuf, process::Command};

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tg(args: &[&std::ffi::OsStr]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_timegrapher"))
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8(out.stdout).unwrap()
}

fn json(args: &[&std::ffi::OsStr]) -> serde_json::Value {
    serde_json::from_str(&tg(args)).unwrap()
}

fn line<'a>(text: &'a str, lead: &str) -> &'a str {
    (text.lines())
        .find(|l| l.starts_with(lead))
        .unwrap_or_else(|| panic!("no {lead:?} line in\n{text}"))
}

fn off_standard(doc: &serde_json::Value) -> Option<&serde_json::Value> {
    (doc["caveats"].as_array().unwrap().iter()).find(|c| c["code"] == "off_standard_rate")
}

#[test]
fn long_and_series_give_no_rate_off_every_standard_rate() {
    let dir = Dir(std::env::temp_dir().join(format!("tg-long-off-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let wav = dir.0.join("x.wav");
    let out = dir.0.join("x_long");
    tg(&[
        "synth".as_ref(),
        wav.as_os_str(),
        "--duration=80".as_ref(),
        "--bph=17570".as_ref(),
        "--rate=5".as_ref(),
    ]);
    let w = wav.as_os_str();
    let to = out.as_os_str();
    let set = "--bph=17570".as_ref();
    let j = "--json".as_ref();

    // On Auto: no rate anywhere, the caveat beside it, as analyze's.
    let analyze = json(&["analyze".as_ref(), w, j]);
    let long = json(&["long".as_ref(), w, "--out".as_ref(), to, j]);
    assert!(
        long["overall"]["rate_s_per_day"].is_null(),
        "{}",
        long["overall"]
    );
    assert!(long["overall"]["beat_error_ms"].is_number());
    assert!(long["rate_p05"].is_null() && long["rate_p95"].is_null());
    assert!(long["amplitude_deg"].is_number());
    let caveat = off_standard(&long).expect("the caveat");
    assert_eq!(
        caveat["bph_measured"].as_f64().map(f64::round),
        off_standard(&analyze)
            .and_then(|c| c["bph_measured"].as_f64())
            .map(f64::round)
    );
    let slices = std::fs::read_to_string(out.join("slices.csv")).unwrap();
    let rows: Vec<&str> = slices.lines().skip(1).collect();
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(|r| r.split(',').nth(2) == Some("-")),
        "{slices}"
    );
    let report = std::fs::read_to_string(out.join("report.html")).unwrap();
    assert!(report.contains("No rate in s/d"), "the HTML says why");
    let text = tg(&["long".as_ref(), w, "--out".as_ref(), to]);
    assert_eq!(
        line(&text, "Rate  "),
        line(&tg(&["analyze".as_ref(), w]), "Rate  ")
    );
    let series = json(&["series".as_ref(), w, j]);
    assert!(off_standard(&series).is_some());
    assert_eq!(series["series"][0]["series"], "rate");
    assert_eq!(series["series"][0]["readings"], 0);
    assert!(series["series"][0]["median"].is_null());
    assert!(series["series"][1]["readings"].as_u64() > Some(0));

    // Set at 17,570: it reads, near the synthetic +5 s/d.
    let long = json(&["long".as_ref(), w, set, "--out".as_ref(), to, j]);
    assert!(off_standard(&long).is_none());
    let rate = long["overall"]["rate_s_per_day"].as_f64().expect("a rate");
    assert!((rate - 5.0).abs() < 0.5, "{rate}");
    let series = json(&["series".as_ref(), w, set, j]);
    assert!(series["series"][0]["readings"].as_u64() > Some(0));
    let median = series["series"][0]["median"].as_f64().expect("a median");
    assert!((median - 5.0).abs() < 0.5, "{median}");
}
