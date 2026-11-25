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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mm: Option<f64>,
    /// As the source writes it, without the ‴ sign: "11½", "13¼".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lignes: Option<String>,
    pub source: String,
}

/// The power reserve in hours: one figure (`value`, with `approx` where
/// the source says "about") or a range (`from` and `to`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerReserve {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<f64>,
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
    match (d.mm, lignes) {
        (Some(mm), Some(l)) => Some(format!("{mm} mm ({l})")),
        (Some(mm), None) => Some(format!("{mm} mm")),
        (None, Some(l)) => Some(l),
        (None, None) => None,
    }
}

fn reserve_text(p: &PowerReserve) -> Option<String> {
    let hours = match (p.value, p.from, p.to) {
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

/// The Specs line ("25 jewels · 25.6 mm (11½‴) · 4.6 mm high · 38 h power
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
/// capitalised ("Date, day, chronograph"), or `None` when none is listed.
pub fn functions_line(c: &Calibre) -> Option<String> {
    let listed = &c.functions.as_ref()?.value;
    let names: Vec<&str> = FUNCTIONS
        .iter()
        .copied()
        .filter(|f| listed.iter().any(|l| l == f))
        .collect();
    let line = names.join(", ");
    let mut chars = line.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

/// The "Made" line: "Made 1971–1985", "Made from 1971", "Made until
/// 1985", with "c. " before each year where the source gives them as
/// approximate.
pub fn made_line(y: &Years) -> Option<String> {
    let year = |n: u16| {
        if y.approx {
            format!("c. {n}")
        } else {
            n.to_string()
        }
    };
    match (y.from, y.to) {
        (Some(a), Some(b)) => Some(format!("Made {}–{}", year(a), year(b))),
        (Some(a), None) => Some(format!("Made from {}", year(a))),
        (None, Some(b)) => Some(format!("Made until {}", year(b))),
        (None, None) => None,
    }
}

/// Why the row's spec fields break the rules, or `None` when they keep
/// them: every field has a source starting with its address, a diameter
/// states mm or lignes, a power reserve is one figure or a range, heights
/// carry at most two decimals, and functions come from [`FUNCTIONS`].
pub fn spec_fault(c: &Calibre) -> Option<String> {
    let mut sources = Vec::new();
    if let Some(j) = &c.jewels {
        if j.value == 0 {
            return Some("jewels is 0".into());
        }
        sources.push(&j.source);
    }
    if let Some(d) = &c.diameter {
        if d.mm.is_none() && d.lignes.as_deref().is_none_or(str::is_empty) {
            return Some("diameter states neither mm nor lignes".into());
        }
        if d.mm.is_some_and(|mm| mm <= 0.0) {
            return Some("diameter is not positive".into());
        }
        sources.push(&d.source);
    }
    if let Some(h) = &c.height_mm {
        let hundredths = h.value * 100.0;
        if h.value <= 0.0 || (hundredths - hundredths.round()).abs() > 1e-6 {
            return Some(format!(
                "height {} is not a positive figure to 0.01 mm",
                h.value
            ));
        }
        sources.push(&h.source);
    }
    if let Some(p) = &c.power_reserve_h {
        let ok = match (p.value, p.from, p.to) {
            (Some(v), None, None) => v > 0.0,
            (None, Some(a), Some(b)) => a > 0.0 && a < b && !p.approx,
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

    const SRC: &str = "https://example.com/2824 (maker's sheet) (\"25 jewels\")";

    #[test]
    fn specs_line_shows_figures_as_stated() {
        let c = row(&format!(
            r#", "jewels": {{"value": 25, "source": "{s}"}},
               "diameter": {{"mm": 25.6, "lignes": "11½", "source": "{s}"}},
               "height_mm": {{"value": 4.6, "source": "{s}"}},
               "power_reserve_h": {{"value": 38, "source": "{s}"}}"#,
            s = SRC.replace('"', "\\\"")
        ));
        assert_eq!(
            specs_line(&c).as_deref(),
            Some("25 jewels · 25.6 mm (11½‴) · 4.6 mm high · 38 h power reserve")
        );
        assert_eq!(spec_parts(&c)[0].hover, format!("Jewels: from {SRC}"));
        assert_eq!(spec_fault(&c), None);
    }

    #[test]
    fn missing_figures_drop_out_and_nothing_is_converted() {
        let lignes = row(r#", "diameter": {"lignes": "11½", "source": "https://x"}"#);
        assert_eq!(specs_line(&lignes).as_deref(), Some("11½‴"));
        let mm = row(r#", "diameter": {"mm": 25.6, "source": "https://x"}"#);
        assert_eq!(specs_line(&mm).as_deref(), Some("25.6 mm"));
        assert_eq!(specs_line(&row("")), None);
        assert_eq!(functions_line(&row("")), None);
    }

    #[test]
    fn power_reserve_is_a_figure_about_a_figure_or_a_range() {
        let pr = |j: &str| specs_line(&row(&format!(r#", "power_reserve_h": {j}"#)));
        assert_eq!(
            pr(r#"{"value": 42, "approx": true, "source": "https://x"}"#).as_deref(),
            Some("about 42 h power reserve")
        );
        assert_eq!(
            pr(r#"{"from": 38, "to": 40, "source": "https://x"}"#).as_deref(),
            Some("38 to 40 h power reserve")
        );
        let both = row(r#", "power_reserve_h": {"value": 38, "to": 40, "source": "https://x"}"#);
        assert!(spec_fault(&both).is_some());
    }

    #[test]
    fn functions_follow_the_fixed_order_and_list() {
        let c = row(
            r#", "functions": {"value": ["chronograph", "day", "date"], "source": "https://x"}"#,
        );
        assert_eq!(
            functions_line(&c).as_deref(),
            Some("Date, day, chronograph")
        );
        let bad = row(r#", "functions": {"value": ["no hacking"], "source": "https://x"}"#);
        assert!(spec_fault(&bad).is_some());
    }

    #[test]
    fn made_line_forms() {
        let y = |j: &str| made_line(&row(&format!(r#", "years": {j}"#)).years.unwrap());
        let s = r#""source": "https://x""#;
        assert_eq!(
            y(&format!(r#"{{"from": 1971, "to": 1985, {s}}}"#)).as_deref(),
            Some("Made 1971–1985")
        );
        assert_eq!(
            y(&format!(r#"{{"from": 1982, {s}}}"#)).as_deref(),
            Some("Made from 1982")
        );
        assert_eq!(
            y(&format!(r#"{{"from": 1970, "approx": true, {s}}}"#)).as_deref(),
            Some("Made from c. 1970")
        );
    }

    #[test]
    fn bad_rows_are_caught() {
        for extra in [
            r#", "jewels": {"value": 25, "source": "maker's sheet"}"#,
            r#", "diameter": {"source": "https://x"}"#,
            r#", "height_mm": {"value": 4.605, "source": "https://x"}"#,
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
