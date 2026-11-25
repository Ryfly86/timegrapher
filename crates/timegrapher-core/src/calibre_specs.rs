//! A calibre's reference facts as the pane and the website show them
//! (#1861, #401): the "Made" line, the Specs line and the functions line.
//!
//! Every figure is stored as its source states it and shown as stored.
//! Nothing here converts or rounds: lignes are never worked out from
//! millimetres (lignes are nominal), and a power reserve range stays a
//! range. A field the row lacks drops out with its separator, and a
//! function the row doesn't list is never named as absent. This is the
//! one formatter: the app's Calibres pane and `calibres export --json`
//! for opentimegrapher.org both call it.

use crate::calibres::{Calibre, Years};
use serde::{Deserialize, Serialize};

/// A single figure and the source that states it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stated<T> {
    pub value: T,
    pub source: String,
}

/// The movement's diameter, in millimetres, lignes or both, as the source
/// states it. Neither is ever worked out from the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diameter {
    /// As the source writes it: "25.60" stays "25.60".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mm: Option<String>,
    /// As the source writes it, without the ‴ sign: "11½", "13¼".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lignes: Option<String>,
    pub source: String,
}

/// The power reserve in hours: one figure (`value`, with `approx` where
/// the source says "about") or a range (`from` and `to`), each written as
/// the source writes it ("38.5").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerReserve {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub approx: bool,
    pub source: String,
}

/// The functions [`Calibre::functions`] may list, in the order they are
/// shown. Where the seconds hand sits stays in [`Calibre::seconds`].
pub const FUNCTIONS: [&str; 10] = [
    "hand winding",
    "automatic winding",
    "hacking",
    "date",
    "quickset date",
    "day",
    "GMT hand",
    "chronograph",
    "power reserve indicator",
    "moon phase",
];

/// Joins the figures of the Specs line.
pub const SPECS_SEPARATOR: &str = " · ";

/// One figure of the Specs line: what it says and the source in its hover.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpecPart {
    pub text: String,
    pub hover: String,
}

fn hover(label: &str, source: &str) -> String {
    format!("{label}: from {source}")
}

fn diameter_text(d: &Diameter) -> Option<String> {
    let lignes = d.lignes.as_ref().map(|l| format!("{l}‴"));
    match (d.mm.as_ref(), lignes) {
        (Some(mm), Some(l)) => Some(format!("{mm} mm ({l})")),
        (Some(mm), None) => Some(format!("{mm} mm")),
        (None, Some(l)) => Some(l),
        (None, None) => None,
    }
}

fn reserve_text(p: &PowerReserve) -> Option<String> {
    let hours = match (&p.value, &p.from, &p.to) {
        (Some(v), _, _) if p.approx => format!("about {v} h"),
        (Some(v), _, _) => format!("{v} h"),
        (None, Some(a), Some(b)) => format!("{a} to {b} h"),
        _ => return None,
    };
    Some(format!("{hours} power reserve"))
}

/// The Specs line's figures, in order: jewels, diameter, height, power
/// reserve. Empty when the row has none of them.
pub fn spec_parts(c: &Calibre) -> Vec<SpecPart> {
    let mut parts = Vec::new();
    if let Some(j) = &c.jewels {
        parts.push(SpecPart {
            text: format!("{} jewels", j.value),
            hover: hover("Jewels", &j.source),
        });
    }
    if let Some(d) = &c.diameter {
        if let Some(text) = diameter_text(d) {
            parts.push(SpecPart {
                text,
                hover: hover("Diameter", &d.source),
            });
        }
    }
    if let Some(h) = &c.height_mm {
        parts.push(SpecPart {
            text: format!("{} mm high", h.value),
            hover: hover("Height", &h.source),
        });
    }
    if let Some(p) = &c.power_reserve_h {
        if let Some(text) = reserve_text(p) {
            parts.push(SpecPart {
                text,
                hover: hover("Power reserve", &p.source),
            });
        }
    }
    parts
}

/// The Specs line ("25 jewels · 25.60 mm (11½‴) · 4.60 mm high · 38 h power
/// reserve"), or `None` when the row has none of its figures.
pub fn specs_line(c: &Calibre) -> Option<String> {
    let parts = spec_parts(c);
    (!parts.is_empty()).then(|| {
        parts
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join(SPECS_SEPARATOR)
    })
}

/// The functions line, in [`FUNCTIONS`] order with the first letter
/// capitalised ("Date, day, chronograph"), and its hover, or `None` when
/// none is listed.
pub fn functions_line(c: &Calibre) -> Option<SpecPart> {
    let f = c.functions.as_ref()?;
    let names: Vec<&str> = FUNCTIONS
        .iter()
        .copied()
        .filter(|n| f.value.iter().any(|l| l == n))
        .collect();
    let line = names.join(", ");
    let mut chars = line.chars();
    let first = chars.next()?;
    Some(SpecPart {
        text: first.to_uppercase().chain(chars).collect(),
        hover: hover("Functions", &f.source),
    })
}

/// The "Made" line: "Made 1971 to 1985", "Made from 1982", "Made until
/// 1985", and with approximate years "Made from about 1970" or "Made
/// about 1971 to 1985" (one "about", at the front).
pub fn made_line(y: &Years) -> Option<String> {
    let about = if y.approx { "about " } else { "" };
    match (y.from, y.to) {
        (Some(a), Some(b)) => Some(format!("Made {about}{a} to {b}")),
        (Some(a), None) => Some(format!("Made from {about}{a}")),
        (None, Some(b)) => Some(format!("Made until {about}{b}")),
        (None, None) => None,
    }
}

/// `s` as a positive number written with at most `places` decimals, or
/// `None` when it isn't one ("4.60" passes at 2 places, "4.605" doesn't).
fn stated_number(s: &str, places: usize) -> Option<f64> {
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    let digits = |p: &str| p.chars().all(|ch| ch.is_ascii_digit());
    let ok = !whole.is_empty()
        && digits(whole)
        && digits(frac)
        && frac.len() <= places
        && (s.contains('.') != frac.is_empty());
    let v: f64 = s.parse().ok()?;
    (ok && v > 0.0).then_some(v)
}

/// Decimals a stated millimetre figure may carry (Norm, #2266).
const MM_PLACES: usize = 2;
/// Decimals a stated power reserve, in hours, may carry.
const HOUR_PLACES: usize = 1;

/// Why the row's spec fields break the rules, or `None` when they keep
/// them: every field has a source starting with its address, a diameter
/// states mm or lignes (never an empty lignes), diameter and height mm
/// carry at most two decimals and hours at most one, a power reserve is
/// one figure or a range, and functions come from [`FUNCTIONS`].
pub fn spec_fault(c: &Calibre) -> Option<String> {
    let mut sources = Vec::new();
    let number = |what: &str, v: &str| {
        stated_number(v, MM_PLACES)
            .is_none()
            .then(|| format!("{what} {v:?} is not a positive figure to 0.01 mm"))
    };
    if let Some(j) = &c.jewels {
        if j.value == 0 {
            return Some("jewels is 0".into());
        }
        sources.push(&j.source);
    }
    if let Some(d) = &c.diameter {
        if d.mm.is_none() && d.lignes.is_none() {
            return Some("diameter states neither mm nor lignes".into());
        }
        if d.lignes.as_deref().is_some_and(|l| l.trim().is_empty()) {
            return Some("diameter lignes is empty".into());
        }
        if let Some(fault) = d.mm.as_deref().and_then(|mm| number("diameter", mm)) {
            return Some(fault);
        }
        sources.push(&d.source);
    }
    if let Some(h) = &c.height_mm {
        if let Some(fault) = number("height", &h.value) {
            return Some(fault);
        }
        sources.push(&h.source);
    }
    if let Some(p) = &c.power_reserve_h {
        let n = |v: &Option<String>| v.as_deref().and_then(|v| stated_number(v, HOUR_PLACES));
        let ok = match (&p.value, &p.from, &p.to) {
            (Some(_), None, None) => n(&p.value).is_some(),
            (None, Some(_), Some(_)) => {
                !p.approx && matches!((n(&p.from), n(&p.to)), (Some(a), Some(b)) if a < b)
            }
            _ => false,
        };
        if !ok {
            return Some("power reserve is neither one figure nor a range".into());
        }
        sources.push(&p.source);
    }
    if let Some(f) = &c.functions {
        if f.value.is_empty() {
            return Some("functions lists none".into());
        }
        if let Some(bad) = f.value.iter().find(|v| !FUNCTIONS.contains(&v.as_str())) {
            return Some(format!("function {bad:?} is not in the fixed list"));
        }
        sources.push(&f.source);
    }
    sources
        .into_iter()
        .find(|s| !s.starts_with("http"))
        .map(|s| format!("source {s:?} does not start with its address"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibres::all;

    fn row(extra: &str) -> Calibre {
        let json = format!(
            r#"{{"calibre": "Test 1", "maker": "Test", "sources": [],
                "confidence": "maker's page"{extra}}}"#
        );
        serde_json::from_str(&json).expect("test row parses")
    }

    const SRC: &str = "https://example.com/2824 (maker's sheet) (25 jewels)";

    #[test]
    fn specs_line_shows_figures_as_the_source_writes_them() {
        let c = row(&format!(
            r#", "jewels": {{"value": 25, "source": "{SRC}"}},
               "diameter": {{"mm": "25.60", "lignes": "11½", "source": "{SRC}"}},
               "height_mm": {{"value": "4.60", "source": "{SRC}"}},
               "power_reserve_h": {{"value": "38", "source": "{SRC}"}}"#
        ));
        assert_eq!(
            specs_line(&c).as_deref(),
            Some("25 jewels · 25.60 mm (11½‴) · 4.60 mm high · 38 h power reserve")
        );
        let hovers: Vec<_> = spec_parts(&c).into_iter().map(|p| p.hover).collect();
        assert_eq!(
            hovers,
            ["Jewels", "Diameter", "Height", "Power reserve"].map(|l| format!("{l}: from {SRC}"))
        );
        assert_eq!(spec_fault(&c), None);
    }

    #[test]
    fn missing_figures_drop_out_and_nothing_is_converted() {
        let lignes = row(r#", "diameter": {"lignes": "11½", "source": "https://x"}"#);
        assert_eq!(specs_line(&lignes).as_deref(), Some("11½‴"));
        let mm = row(r#", "diameter": {"mm": "25.6", "source": "https://x"}"#);
        assert_eq!(specs_line(&mm).as_deref(), Some("25.6 mm"));
        assert_eq!(specs_line(&row("")), None);
        assert_eq!(functions_line(&row("")), None);
    }

    #[test]
    fn power_reserve_is_a_figure_about_a_figure_or_a_range() {
        let pr = |j: &str| specs_line(&row(&format!(r#", "power_reserve_h": {j}"#)));
        assert_eq!(
            pr(r#"{"value": "42", "approx": true, "source": "https://x"}"#).as_deref(),
            Some("about 42 h power reserve")
        );
        assert_eq!(
            pr(r#"{"value": "38.5", "source": "https://x"}"#).as_deref(),
            Some("38.5 h power reserve")
        );
        assert_eq!(
            pr(r#"{"from": "38", "to": "40", "source": "https://x"}"#).as_deref(),
            Some("38 to 40 h power reserve")
        );
        // Compared as numbers, not strings: "9" is less than "10".
        let range = row(r#", "power_reserve_h": {"from": "9", "to": "10", "source": "https://x"}"#);
        assert_eq!(spec_fault(&range), None);
    }

    #[test]
    fn functions_follow_the_fixed_order_and_list() {
        let c = row(
            r#", "functions": {"value": ["chronograph", "GMT hand", "date"], "source": "https://x"}"#,
        );
        let line = functions_line(&c).expect("functions line");
        assert_eq!(line.text, "Date, GMT hand, chronograph");
        assert_eq!(line.hover, "Functions: from https://x");
    }

    #[test]
    fn made_line_forms() {
        let y = |j: &str| {
            let c = row(&format!(r#", "years": {{{j}, "source": "https://x"}}"#));
            made_line(&c.years.expect("years")).expect("made line")
        };
        assert_eq!(y(r#""from": 1971, "to": 1985"#), "Made 1971 to 1985");
        assert_eq!(y(r#""from": 1982"#), "Made from 1982");
        assert_eq!(y(r#""to": 1985"#), "Made until 1985");
        assert_eq!(y(r#""from": 1970, "approx": true"#), "Made from about 1970");
        assert_eq!(
            y(r#""from": 1971, "to": 1985, "approx": true"#),
            "Made about 1971 to 1985"
        );
    }

    #[test]
    fn bad_rows_are_caught() {
        for extra in [
            r#", "jewels": {"value": 25, "source": "maker's sheet"}"#,
            r#", "diameter": {"source": "https://x"}"#,
            r#", "diameter": {"mm": "25,6", "source": "https://x"}"#,
            r#", "height_mm": {"value": "4.605", "source": "https://x"}"#,
            r#", "height_mm": {"value": "0", "source": "https://x"}"#,
            r#", "power_reserve_h": {"value": "38", "to": "40", "source": "https://x"}"#,
            r#", "power_reserve_h": {"from": "40", "to": "38", "source": "https://x"}"#,
            r#", "power_reserve_h": {"value": "38.25", "source": "https://x"}"#,
            r#", "power_reserve_h": {"value": "~38", "source": "https://x"}"#,
            r#", "height_mm": {"value": "4.6 mm", "source": "https://x"}"#,
            r#", "jewels": {"value": 0, "source": "https://x"}"#,
            r#", "power_reserve_h": {"from": "38", "to": "38", "source": "https://x"}"#,
            r#", "height_mm": {"value": "4.", "source": "https://x"}"#,
            r#", "height_mm": {"value": ".5", "source": "https://x"}"#,
            r#", "functions": {"value": [], "source": "https://x"}"#,
            r#", "diameter": {"mm": "25.60", "lignes": "", "source": "https://x"}"#,
            r#", "power_reserve_h": {"from": "38", "to": "40", "approx": true, "source": "https://x"}"#,
            r#", "functions": {"value": ["no hacking"], "source": "https://x"}"#,
        ] {
            assert!(spec_fault(&row(extra)).is_some(), "{extra}");
        }
    }

    /// Every row in the table keeps the spec fields' rules (#1861).
    #[test]
    fn every_row_keeps_the_spec_rules() {
        for c in all() {
            assert_eq!(spec_fault(c), None, "{}", c.calibre);
        }
    }
}
