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
    // Nor do the views and maps beside it: no figure near the rate read
    // against either standard rate, but in the caveat that gives both.
    let mut drawn = json(&[
        "series".as_ref(),
        w,
        "--views".as_ref(),
        "--maps".as_ref(),
        j,
    ]);
    drawn["caveats"] = serde_json::Value::Null;
    let near = |v: f64| (v.abs() - 1455.1).abs() < 5.0 || (v.abs() - 2059.1).abs() < 5.0;
    let mut found = Vec::new();
    rates_near(&drawn, "", &near, &mut found);
    assert!(found.is_empty(), "{found:?}");

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

/// The paths in `v` of each number `near` says is near, leaving out times
/// (`t_s`, the views' and maps' seconds from the take's start).
fn rates_near(
    v: &serde_json::Value,
    path: &str,
    near: &dyn Fn(f64) -> bool,
    out: &mut Vec<String>,
) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m.iter().filter(|(k, _)| k.as_str() != "t_s") {
                rates_near(x, &format!("{path}.{k}"), near, out);
            }
        }
        serde_json::Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                rates_near(x, &format!("{path}[{i}]"), near, out);
            }
        }
        serde_json::Value::Number(n) if n.as_f64().is_some_and(near) => out.push(path.into()),
        _ => {}
    }
}

/// `seconds` of seeded white noise, ±200 counts at 48 kHz, as a 16-bit WAV.
fn noise_wav(path: &std::path::Path, seconds: u32) {
    let n = 48_000 * seconds;
    let mut b = Vec::with_capacity(44 + 2 * n as usize);
    let u32le = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
    b.extend_from_slice(b"RIFF");
    u32le(&mut b, 36 + 2 * n);
    b.extend_from_slice(b"WAVEfmt ");
    u32le(&mut b, 16);
    b.extend_from_slice(&[1, 0, 1, 0]);
    u32le(&mut b, 48_000);
    u32le(&mut b, 96_000);
    b.extend_from_slice(&[2, 0, 16, 0]);
    b.extend_from_slice(b"data");
    u32le(&mut b, 2 * n);
    let mut x: u64 = 0x2355;
    for _ in 0..n {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let v = ((x >> 33) % 401) as i16 - 200;
        b.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// Noise alone (#2355): `long` gives no figures, in `analyze`'s words, and
/// no amplitude windows; `series` reads nothing.
#[test]
fn long_and_series_on_noise_give_no_figures() {
    let dir = Dir(std::env::temp_dir().join(format!("tg-long-noise-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let wav = dir.0.join("noise.wav");
    let out = dir.0.join("noise_long");
    noise_wav(&wav, 45);
    let (w, to) = (wav.as_os_str(), out.as_os_str());
    let analyze = tg(&["analyze".as_ref(), w]);
    let said = line(&analyze, "No regular beat found");
    let text = tg(&["long".as_ref(), w, "--out".as_ref(), to]);
    assert_eq!(
        line(&text, "No regular beat found"),
        format!("{said}. There are no rate, amplitude or beat error readings.")
    );
    for lead in ["Rate ", "Beat error", "Amplitude", "Periodic"] {
        assert!(
            !text.lines().any(|l| l.starts_with(lead)),
            "{lead} in\n{text}"
        );
    }
    let amplitude = std::fs::read_to_string(out.join("amplitude.csv")).unwrap();
    assert_eq!(amplitude.lines().count(), 1, "{amplitude}");
    let long = json(&["long".as_ref(), w, "--out".as_ref(), to, "--json".as_ref()]);
    assert!(long["overall"].is_null() && long["amplitude_deg"].is_null());
    assert!((long["caveats"].as_array().unwrap().iter()).any(|c| c["code"] == "no_watch"));
    let series = json(&["series".as_ref(), w, "--json".as_ref()]);
    for s in series["series"].as_array().unwrap() {
        assert_eq!(s["readings"], 0, "{}", s["series"]);
    }
}
