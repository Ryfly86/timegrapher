//! `analyze`'s text on Auto off every standard rate (#2223, Hugh on
//! #2316): the Beat rate line names no standard rate, the Rate line is the
//! caveat's sentence, at the clock's correction as the JSON reads it, and
//! there is no rate-spread line.
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

fn line<'a>(text: &'a str, lead: &str) -> &'a str {
    (text.lines())
        .find(|l| l.starts_with(lead))
        .unwrap_or_else(|| panic!("no {lead:?} line in\n{text}"))
}

#[test]
fn the_text_off_every_standard_rate() {
    let dir = Dir(std::env::temp_dir().join(format!("tg-off-standard-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let wav = dir.0.join("x.wav");
    let synth = [
        "synth".as_ref(),
        wav.as_os_str(),
        "--duration=40".as_ref(),
        "--bph=17570".as_ref(),
        "--rate=5".as_ref(),
    ];
    tg(&synth);
    let text = tg(&["analyze".as_ref(), wav.as_os_str()]);
    let beats = line(&text, "Beat rate");
    let n = beats
        .strip_prefix("Beat rate    Auto, ")
        .and_then(|s| s.strip_suffix(" beats found"))
        .unwrap_or_else(|| panic!("{beats}"));
    assert!(n.parse::<usize>().is_ok_and(|n| n > 150), "{beats}");
    assert_eq!(
        line(&text, "Rate  "),
        "Rate         The beat rate heard, 17571 bph, is more than 1000 s/d from every \
         standard rate (+1455.1 s/d against 17280 bph; -2059.1 s/d against 18000 bph), so no \
         rate in s/d is given."
    );
    assert!(!text.contains("Rate spread"), "{text}");
    // With the card 20 ppm fast, the sides are read at the correction,
    // as the JSON's caveat reads them.
    let text = tg(&[
        "analyze".as_ref(),
        wav.as_os_str(),
        "--card-ppm=20".as_ref(),
    ]);
    let json = tg(&[
        "analyze".as_ref(),
        wav.as_os_str(),
        "--card-ppm=20".as_ref(),
        "--json".as_ref(),
    ]);
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    let c = (doc["caveats"].as_array().unwrap().iter())
        .find(|c| c["code"] == "off_standard_rate")
        .expect("the caveat");
    let side = |s: &str| {
        let r = c[s]["rate_s_per_day"].as_f64().unwrap();
        format!(
            "{}{:.1} s/d against {} bph",
            if r >= 0.0 { "+" } else { "-" },
            r.abs(),
            c[s]["bph"]
        )
    };
    let rate = line(&text, "Rate  ");
    assert!(rate.contains(&side("below")), "{rate} for {c}");
    assert!(rate.contains(&side("above")), "{rate} for {c}");
}
