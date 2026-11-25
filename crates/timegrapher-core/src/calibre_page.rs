//! A calibre's reference page, as the Calibres pane and opentimegrapher.org
//! show it (#1861, #401), and the words for a calibre's name and sources
//! the pane uses.
//!
//! `timegrapher calibres export --json` writes [`export`] for the site's
//! build (Hugh on #1861, option b): every display string, heading, slug,
//! link and alt text comes from here, so the site's templates only lay out
//! what they are given and never word or work out a figure. The Made,
//! Specs and functions lines come from [`crate::calibre_specs`], the one
//! formatter. Only the shipped table is exported, never a user's own
//! calibres.

use crate::calibre_specs::{self, SpecPart};
use crate::calibres::{self, asset_url, Calibre, RELATIONS};
use crate::figure_text::plain_text;
use crate::steady_views::title_case;
use crate::train::period_text;
use serde::Serialize;
use std::collections::HashMap;

/// Where the calibre pages live on the site; each page is at
/// `/calibres/<maker>/<calibre>/` (Peter, #401).
pub const SITE_ROOT: &str = "/calibres/";

/// How many calibres a related line names before "+n more" (Hugh, #1861).
pub const RELATED_SHOWN: usize = 5;

/// The headings and fixed words of a calibre's page, beside the pane's.
#[derive(Debug, Clone, Serialize)]
pub struct Headings {
    pub index: &'static str,
    pub summary: &'static str,
    pub summary_hover: &'static str,
    pub pictures: &'static str,
    pub picture_caption: &'static str,
    pub documents: &'static str,
    pub open: &'static str,
    pub issues: &'static str,
    pub related: &'static str,
    pub wheels: &'static str,
    pub sources: &'static str,
    pub notes: &'static str,
}

/// The one set of headings.
pub const HEADINGS: Headings = Headings {
    index: "Calibres",
    summary: "Summary",
    summary_hover: "Written from the sources listed below.",
    pictures: "Pictures",
    picture_caption: "Maker's photo, mirrored",
    documents: "Documents",
    open: "Open",
    issues: "Reported issues",
    related: "Related movements",
    wheels: "Wheels",
    sources: "Sources",
    notes: "Notes",
};

/// The whole export: the headings, every maker for the index, and every
/// calibre's page, in table order.
#[derive(Debug, Clone, Serialize)]
pub struct Export {
    pub headings: Headings,
    pub makers: Vec<MakerEntry>,
    pub calibres: Vec<Page>,
}

/// A maker on the index page.
#[derive(Debug, Clone, Serialize)]
pub struct MakerEntry {
    pub maker: String,
    pub slug: String,
    pub path: String,
    /// One of the big makers the app lists first.
    pub common: bool,
    /// Its calibres in the table.
    pub calibres: usize,
}

/// A link to another calibre's page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Link {
    pub name: String,
    pub path: String,
}

/// One calibre's page.
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    /// The row's name in the table, as `calibres NAME` takes it.
    pub name: String,
    pub maker: String,
    /// The page's title: the name with its maker in front, once.
    pub title: String,
    pub maker_slug: String,
    pub slug: String,
    pub path: String,
    /// Where the page was under the names the row had before, for the
    /// site's redirects, so an old link still lands here.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub former_paths: Vec<String>,
    pub common: bool,
    /// "Also known as …", the row's other names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub also_known_as: Option<String>,
    /// Said in place of the lift angle when Timegrapher doesn't measure
    /// the escapement yet (#1872).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub escapement: Option<SpecPart>,
    /// Beat rate, lift angle and escape wheel in one line, saying which
    /// the table doesn't give ("28800 bph · lift angle 50°").
    pub figures: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub made: Option<SpecPart>,
    /// The Specs line and its figures with their hovers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specs: Option<Specs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<SpecPart>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wheels: Vec<WheelLine>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub summary: Vec<Cited>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pictures: Vec<Picture>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub documents: Vec<Document>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<Cited>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedLine>,
    /// "Sources · Maker's Page": the heading and the row's confidence.
    pub sources_heading: String,
    /// Numbered from 1; `Cited::source` and `RelatedLine::source` point here.
    pub sources: Vec<SourceLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// The Specs line whole, and its figures one by one for their hovers.
#[derive(Debug, Clone, Serialize)]
pub struct Specs {
    pub text: String,
    pub parts: Vec<SpecPart>,
}

/// A wheel and its time for one full turn.
#[derive(Debug, Clone, Serialize)]
pub struct WheelLine {
    pub name: String,
    /// "3600 s".
    pub period: String,
}

/// A sentence and the number of its source in `Page::sources`.
#[derive(Debug, Clone, Serialize)]
pub struct Cited {
    pub text: String,
    pub source: usize,
}

/// A maker's photo, mirrored in timegrapher-calibres.
#[derive(Debug, Clone, Serialize)]
pub struct Picture {
    pub kind: String,
    pub url: String,
    pub alt: String,
}

/// A maker's spec sheet, drawing or manual, mirrored in timegrapher-calibres.
#[derive(Debug, Clone, Serialize)]
pub struct Document {
    /// "Spec sheet".
    pub kind: String,
    pub file: String,
    pub url: String,
}

/// One relation: "Clone of" and the calibres, the first
/// [`RELATED_SHOWN`] named and "+n more" for the rest. A relation stored
/// on this row cites its source; one read off the other rows (the inverse,
/// "Cloned by") cites none here, as it is stated there.
#[derive(Debug, Clone, Serialize)]
pub struct RelatedLine {
    pub label: String,
    pub calibres: Vec<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub more: Option<More>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<usize>,
}

/// The calibres past the first few, and the words that stand for them.
#[derive(Debug, Clone, Serialize)]
pub struct More {
    pub text: String,
    pub calibres: Vec<Link>,
}

/// A numbered source: its name, its address when it has one, and what it
/// says, as the pane's Sources list shows them.
#[derive(Debug, Clone, Serialize)]
pub struct SourceLine {
    pub number: usize,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub says: String,
}

/// Beat rate, lift angle and escape wheel, saying which aren't given:
/// the words the pane's hints and the site's pages both use.
pub fn figure_words(bph: Option<u32>, lift: Option<f64>, escape_teeth: Option<u32>) -> Vec<String> {
    let mut parts = vec![
        bph.map_or("beat rate not given".into(), |b| format!("{b} bph")),
        lift.map_or("lift angle not given".into(), |l| {
            format!("lift angle {}°", plain_text(l))
        }),
    ];
    if let Some(t) = escape_teeth {
        parts.push(format!("{t}-tooth escape wheel"));
    }
    parts
}

/// `s` for an address: ASCII letters and digits in lower case, every
/// other run of characters one "-" ("ETA 2824-2" is "eta-2824-2").
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let dash = |out: &mut String| {
        if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    };
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if let Some(letter) = plain_letter(ch) {
            out.push_str(letter);
        } else if let Some(word) = spelt_word(ch) {
            dash(&mut out);
            out.push_str(word);
            dash(&mut out);
        } else {
            dash(&mut out);
        }
    }
    out.trim_end_matches('-').to_string()
}

/// A letter with an accent as the address spells it: "é" is "e".
fn plain_letter(ch: char) -> Option<&'static str> {
    Some(match ch {
        'à' | 'á' | 'â' | 'ä' | 'À' | 'Á' | 'Â' | 'Ä' => "a",
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' => "e",
        'ï' | 'î' | 'í' => "i",
        'ö' | 'ô' | 'ó' | 'Ö' => "o",
        'ü' | 'û' | 'ú' | 'Ü' => "u",
        'ç' => "c",
        'ß' => "ss",
        _ => return None,
    })
}

/// A character that stands as a word of its own in an address: a
/// fraction of a ligne ("3 ½" is "3-1-2") or a Roman numeral ("SL4617Ⅲ"
/// is "sl4617-iii").
fn spelt_word(ch: char) -> Option<&'static str> {
    Some(match ch {
        '¼' => "1-4",
        '½' => "1-2",
        '¾' => "3-4",
        '⅓' => "1-3",
        '⅔' => "2-3",
        '⅛' => "1-8",
        '⅜' => "3-8",
        '⅝' => "5-8",
        '⅞' => "7-8",
        'Ⅰ' => "i",
        'Ⅱ' => "ii",
        'Ⅲ' => "iii",
        'Ⅳ' => "iv",
        'Ⅴ' => "v",
        'Ⅵ' => "vi",
        _ => return None,
    })
}

/// A calibre's name without its maker in front, as its page's slug takes
/// it ("2824-2" for ETA 2824-2); the whole name when nothing is left.
fn short_name<'a>(maker: &str, name: &'a str) -> &'a str {
    name.strip_prefix(maker)
        .and_then(|rest| rest.strip_prefix(' '))
        .filter(|rest| !slug(rest).is_empty())
        .unwrap_or(name)
}

/// The page address of `name` under `maker`.
fn page_path(maker: &str, name: &str) -> String {
    format!(
        "{SITE_ROOT}{}/{}/",
        slug(maker),
        slug(short_name(maker, name))
    )
}

/// The address of the page of `c`.
pub fn path(c: &Calibre) -> String {
    page_path(&c.maker, &c.calibre)
}

/// How the other rows' relations read from the calibre they point at:
/// "Cloned by" for "clone of". "same base as" is stored on both rows, so
/// it has no inverse.
fn inverse_label(relation: &str) -> Option<&'static str> {
    match relation {
        "clone of" => Some("Cloned by"),
        "based on" => Some("Base calibre for"),
        "successor of" => Some("Succeeded by"),
        _ => None,
    }
}

/// The sources of a page, numbered in the order they are first cited.
#[derive(Default)]
struct Numbered(Vec<String>);

impl Numbered {
    fn number(&mut self, s: &str) -> usize {
        match self.0.iter().position(|x| x == s) {
            Some(i) => i + 1,
            None => {
                self.0.push(s.to_string());
                self.0.len()
            }
        }
    }

    fn lines(self) -> Vec<SourceLine> {
        self.0
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let (name, url, says) = source_parts(s);
                SourceLine {
                    number: i + 1,
                    name,
                    url,
                    says,
                }
            })
            .collect()
    }
}

/// Every source the row's fields cite, beyond its own `sources`, in the
/// order its page shows them.
fn field_sources(c: &Calibre) -> Vec<&str> {
    let mut v: Vec<&str> = Vec::new();
    v.extend(c.years.as_ref().map(|y| y.source.as_str()));
    v.extend(c.jewels.as_ref().map(|j| j.source.as_str()));
    v.extend(c.diameter.as_ref().map(|d| d.source.as_str()));
    v.extend(c.height_mm.as_ref().map(|h| h.source.as_str()));
    v.extend(c.power_reserve_h.as_ref().map(|p| p.source.as_str()));
    v.extend(c.functions.as_ref().map(|f| f.source.as_str()));
    v.extend(c.escapement.as_ref().map(|e| e.source.as_str()));
    v
}

/// One related line: the first few calibres and the rest.
fn related_line(label: String, mut links: Vec<Link>, source: Option<usize>) -> RelatedLine {
    let rest = links.split_off(links.len().min(RELATED_SHOWN));
    let more = (!rest.is_empty()).then(|| More {
        text: format!("+{} more", rest.len()),
        calibres: rest,
    });
    RelatedLine {
        label,
        calibres: links,
        more,
        source,
    }
}

/// The relations other rows state about each calibre, by its name.
type Inverses<'a> = HashMap<&'a str, Vec<(&'static str, &'a Calibre)>>;

fn inverses(table: &[Calibre]) -> Inverses<'_> {
    let mut map: Inverses = HashMap::new();
    for c in table {
        for r in &c.related {
            if let Some(label) = inverse_label(&r.relation) {
                map.entry(r.calibre.as_str()).or_default().push((label, c));
            }
        }
    }
    map
}

fn link(c: &Calibre) -> Link {
    Link {
        name: with_maker_name(&c.maker, &c.calibre),
        path: path(c),
    }
}

/// The related lines of `c`: those stored on it, one line per relation
/// and source in [`RELATIONS`] order, then the inverses read off the
/// other rows.
fn related_lines(c: &Calibre, inv: &Inverses, cite: &mut Numbered) -> Vec<RelatedLine> {
    let mut lines = Vec::new();
    for rel in RELATIONS {
        for r in c.related.iter().filter(|r| r.relation == rel) {
            let Some(other) = calibres::all().iter().find(|o| o.calibre == r.calibre) else {
                continue;
            };
            let n = cite.number(&r.source);
            lines.push(related_line(capitalised(rel), vec![link(other)], Some(n)));
        }
    }
    let Some(theirs) = inv.get(c.calibre.as_str()) else {
        return lines;
    };
    for label in ["Cloned by", "Base calibre for", "Succeeded by"] {
        let links: Vec<Link> = theirs
            .iter()
            .filter(|(l, _)| *l == label)
            .map(|(_, o)| link(o))
            .collect();
        if !links.is_empty() {
            lines.push(related_line(label.into(), links, None));
        }
    }
    lines
}

/// The pictures and documents of `c`, from its mirrored files.
fn files(c: &Calibre, title: &str) -> (Vec<Picture>, Vec<Document>) {
    let mut pictures = Vec::new();
    let mut documents = Vec::new();
    for a in &c.assets {
        if a.kind.starts_with("photo") {
            pictures.push(Picture {
                kind: capitalised(&a.kind),
                url: asset_url(a),
                alt: format!("{title}: maker's {}", a.kind),
            });
        } else {
            documents.push(Document {
                kind: capitalised(&a.kind),
                file: a.path.rsplit('/').next().unwrap_or(&a.path).to_string(),
                url: asset_url(a),
            });
        }
    }
    (pictures, documents)
}

/// The page of `c`; `inv` holds the relations other rows state about it.
fn page(c: &Calibre, inv: &Inverses) -> Page {
    let title = with_maker_name(&c.maker, &c.calibre);
    let mut cite = Numbered::default();
    for s in c.sources.iter().map(String::as_str).chain(field_sources(c)) {
        cite.number(s);
    }
    let mut cited = |v: &[calibres::Sourced]| -> Vec<Cited> {
        v.iter()
            .map(|s| Cited {
                text: s.text.clone(),
                source: cite.number(&s.source),
            })
            .collect()
    };
    let summary = cited(&c.summary);
    let issues = cited(&c.issues);
    let related = related_lines(c, inv, &mut cite);
    let (pictures, documents) = files(c, &title);
    let mut former_paths: Vec<String> = Vec::new();
    for f in &c.former_names {
        let p = page_path(&c.maker, f);
        if p != path(c) && !former_paths.contains(&p) {
            former_paths.push(p);
        }
    }
    let parts = calibre_specs::spec_parts(c);
    Page {
        name: c.calibre.clone(),
        maker: c.maker.clone(),
        maker_slug: slug(&c.maker),
        slug: slug(short_name(&c.maker, &c.calibre)),
        path: path(c),
        former_paths,
        common: c.common,
        also_known_as: (!c.family.is_empty())
            .then(|| format!("Also known as {}", c.family.join(", "))),
        escapement: c.escapement.as_ref().map(|x| SpecPart {
            text: x.sentence(),
            hover: format!("Source: {}", x.source),
        }),
        figures: figure_words(c.bph, c.lift_angle_deg, c.escape_teeth).join(" · "),
        made: calibre_specs::made_part(c),
        specs: calibre_specs::specs_line(c).map(|text| Specs { text, parts }),
        functions: calibre_specs::functions_line(c),
        wheels: c
            .wheels
            .iter()
            .map(|w| WheelLine {
                name: title_case(&w.name),
                period: format!("{} s", period_text(w.period_s)),
            })
            .collect(),
        summary,
        pictures,
        documents,
        issues,
        related,
        sources_heading: format!("{} · {}", HEADINGS.sources, title_case(&c.confidence)),
        sources: cite.lines(),
        notes: c.notes.clone(),
        title,
    }
}

/// Every maker in the table, by name, with how many calibres it has.
fn maker_entries(table: &[Calibre]) -> Vec<MakerEntry> {
    let mut makers: Vec<MakerEntry> = Vec::new();
    for c in table {
        match makers.iter_mut().find(|m| m.maker == c.maker) {
            Some(m) => m.calibres += 1,
            None => makers.push(MakerEntry {
                maker: c.maker.clone(),
                slug: slug(&c.maker),
                path: format!("{SITE_ROOT}{}/", slug(&c.maker)),
                common: calibres::makers()
                    .iter()
                    .any(|m| m.maker == c.maker && m.common),
                calibres: 1,
            }),
        }
    }
    makers.sort_by_key(|m| m.maker.to_lowercase());
    makers
}

/// The shipped table as the site shows it (#401).
pub fn export() -> Export {
    let table = calibres::all();
    let inv = inverses(table);
    Export {
        headings: HEADINGS,
        makers: maker_entries(table),
        calibres: table.iter().map(|c| page(c, &inv)).collect(),
    }
}

/// A source as its name, its address when it has one, and what it says.
/// "https://watchguy.co.uk/… (A. Schild 1017, 51°)" is watchguy.co.uk
/// saying "51°" (the calibre's own name, first, is dropped: the panel
/// is headed by it); a source with a description before what it says,
/// "https://docs.google.com/… (lift angle sheet linked from
/// watchrepairtalk.com/topic/…) (…)", is named "Lift angle sheet
/// (watchrepairtalk.com)".
pub fn source_parts(s: &str) -> (String, Option<String>, String) {
    let (url, rest) = match s.split_once(' ') {
        Some((u, r)) if u.starts_with("http") => (Some(u.to_string()), r.trim()),
        None if s.starts_with("http") => (Some(s.to_string()), ""),
        _ => (None, s.trim()),
    };
    let host = |u: &str| {
        u.trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .split('/')
            .next()
            .unwrap_or(u)
            .to_string()
    };
    let Some(url) = url else {
        return (rest.to_string(), None, String::new());
    };
    let groups = paren_groups(rest);
    let (name, says) = match groups.as_deref() {
        Some([one]) => (host(&url), one.clone()),
        Some([first, .., last]) => {
            let name = match first.split_once(" linked from ") {
                Some((what, from)) => format!("{} ({})", capitalised(what), host(from)),
                None => capitalised(first),
            };
            (name, last.clone())
        }
        _ => (host(&url), rest.to_string()),
    };
    (name, Some(url), drop_calibre_name(&says))
}

/// `s` as nothing but bracketed groups, "(a) (b (c))", each group's
/// inside; `None` for anything else.
fn paren_groups(s: &str) -> Option<Vec<String>> {
    let mut groups = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for ch in s.chars() {
        match ch {
            '(' => {
                if depth > 0 {
                    cur.push(ch);
                }
                depth += 1;
            }
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                if depth == 0 {
                    groups.push(std::mem::take(&mut cur));
                } else {
                    cur.push(ch);
                }
            }
            c if depth == 0 => {
                if !c.is_whitespace() {
                    return None;
                }
            }
            c => cur.push(c),
        }
    }
    (depth == 0 && !groups.is_empty()).then_some(groups)
}

/// What a source says, without the calibre's name in front: "A. Schild
/// 1017, 51°, 18000 vph" is "51°, 18000 vph". A first part holding a
/// figure (a degree sign or vph) is kept.
fn drop_calibre_name(says: &str) -> String {
    let mut depth = 0;
    for (i, ch) in says.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                let first = &says[..i];
                if first.contains('°') || first.contains("vph") || first.contains("bph") {
                    return says.to_string();
                }
                return says[i + 1..].trim().to_string();
            }
            _ => {}
        }
    }
    says.to_string()
}

/// "lift angle sheet" as "Lift angle sheet".
fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// A calibre's name with its maker in front, unless the name already
/// starts with the maker under one of its names ("Rolex 3235", not "Rolex
/// Rolex 3235"; "Seagull ST36", not "Tianjin Seagull Seagull ST36").
pub fn with_maker_name(maker: &str, name: &str) -> String {
    // A clone maker's marker is already in a name such as "Patek 240 Clone",
    // so only the maker itself goes in front.
    if let Some(outer) = maker.strip_suffix("(clone)").map(str::trim_end) {
        if name
            .split(|ch: char| !ch.is_alphanumeric())
            .any(|word| word.eq_ignore_ascii_case("clone"))
        {
            return match outer {
                "" => name.to_string(),
                outer => format!("{outer} {name}"),
            };
        }
    }
    let starts_with = |word: &str| {
        !word.is_empty()
            && name
                .get(..word.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(word))
            && name[word.len()..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric())
    };
    if maker_words(maker).iter().any(|w| starts_with(w)) {
        // The maker goes, but not what it says about the calibre.
        match maker.strip_suffix(" (clone)") {
            Some(_) => format!("{name} (clone)"),
            None => name.to_string(),
        }
    } else {
        format!("{maker} {name}")
    }
}

/// The names a maker goes by: its own, its other names, and the parts of
/// a name like "Peacock (Dandong)".
fn maker_words(maker: &str) -> Vec<&str> {
    let mut words = calibres::maker_names(maker);
    if let Some((outer, inner)) = maker.split_once(" (") {
        words.push(outer);
        words.push(inner.trim_end_matches(')'));
    }
    words
}

#[cfg(test)]
#[path = "calibre_page_tests.rs"]
mod tests;
