//! A calibre's page for the site, and the pane's name and source words.

use super::*;
use crate::calibres::{all, find};

fn row(extra: &str) -> Calibre {
    let json = format!(
        r#"{{"calibre": "Test 1", "maker": "Test", "sources": ["https://example.com/t1 (sheet) (28800 A/h)"],
            "confidence": "maker's page", "bph": 28800{extra}}}"#
    );
    serde_json::from_str(&json).expect("test row parses")
}

fn page_of(c: &Calibre) -> Page {
    page(c, &inverses(all()))
}

#[test]
fn slugs_are_lower_case_ascii_words_joined_by_dashes() {
    assert_eq!(slug("ETA 2824-2"), "eta-2824-2");
    assert_eq!(slug("A. Schild"), "a-schild");
    assert_eq!(slug("Shanghai (clone)"), "shanghai-clone");
    assert_eq!(slug("  Seiko 7S26 "), "seiko-7s26");
    assert_eq!(slug("Baguette 3 ½"), "baguette-3-1-2");
    assert_eq!(slug("½ Baguette 3 ¾"), "1-2-baguette-3-3-4");
    assert_eq!(slug("Lémania"), "lemania");
    assert_eq!(slug("SL4617Ⅲ"), "sl4617-iii");
    assert_eq!(path(find("ETA 2824-2").unwrap()), "/calibres/eta/2824-2/");
}

/// Every row has a page of its own, and no former name's address is
/// another row's page (#401).
#[test]
fn every_calibre_has_its_own_page_address() {
    let e = export();
    assert_eq!(e.calibres.len(), all().len());
    let mut seen = std::collections::HashMap::new();
    let mut shared = Vec::new();
    for p in &e.calibres {
        assert!(p.path.starts_with(SITE_ROOT) && p.path.ends_with('/'));
        assert!(!p.slug.is_empty() && !p.maker_slug.is_empty(), "{}", p.name);
        assert_eq!(p.path, format!("{SITE_ROOT}{}/{}/", p.maker_slug, p.slug));
        if let Some(other) = seen.insert(p.path.clone(), p.name.clone()) {
            shared.push(format!("{other} and {} share {}", p.name, p.path));
        }
    }
    assert!(shared.is_empty(), "{} shared: {shared:#?}", shared.len());
    for p in &e.calibres {
        for f in &p.former_paths {
            assert!(!seen.contains_key(f), "{}: {f} is another page", p.name);
        }
    }
    let makers: usize = e.makers.iter().map(|m| m.calibres).sum();
    assert_eq!(makers, all().len());
}

/// The page's lines are the formatter's, word for word, and every
/// sentence's number points at its own source.
#[test]
fn a_page_carries_the_formatters_lines_and_numbered_sources() {
    let src = "https://example.com/spec (maker's sheet) (25 jewels)";
    let c = row(&format!(
        r#", "lift_angle_deg": 50, "years": {{"from": 1971, "to": 1985, "source": "https://example.com/y (y) (1971 to 1985)"}},
           "jewels": {{"value": 25, "source": "{src}"}},
           "functions": {{"value": ["date"], "source": "{src}"}},
           "summary": [{{"text": "One.", "source": "https://example.com/a (a) (one)"}},
                       {{"text": "Two.", "source": "{src}"}}],
           "issues": [{{"text": "Worn pivots.", "source": "https://example.com/b (b) (worn)"}}],
           "family": ["T1", "1"], "former_names": ["Test 0"],
           "assets": [{{"path": "test/test-1/front.jpg", "kind": "photo (dial side)"}},
                      {{"path": "test/test-1/sheet.pdf", "kind": "spec sheet"}}]"#
    ));
    let p = page_of(&c);
    assert_eq!(p.title, "Test 1");
    assert_eq!(p.path, "/calibres/test/1/");
    assert_eq!(p.former_paths, ["/calibres/test/0/"]);
    assert_eq!(p.figures, "28800 bph · lift angle 50°");
    assert_eq!(p.made.as_ref().unwrap().text, "Made 1971 to 1985");
    assert_eq!(
        p.made.as_ref().unwrap().hover,
        "Made: from https://example.com/y (y) (1971 to 1985)"
    );
    assert_eq!(p.specs.as_ref().unwrap().text, "25 jewels");
    assert_eq!(p.functions.as_ref().unwrap().text, "Date");
    // "1" is the page's own name without its maker.
    assert_eq!(p.also_known_as.as_deref(), Some("Also known as T1"));
    assert_eq!(p.basis, "Best basis among these sources: maker's page.");
    // Each spec, functions and Made line carries its source's number.
    assert_eq!(p.made.as_ref().unwrap().source, Some(2));
    assert_eq!(p.specs.as_ref().unwrap().parts[0].source, Some(3));
    assert_eq!(p.functions.as_ref().unwrap().source, Some(3));
    // The row's own source first, then the fields', each once.
    let urls: Vec<_> = p
        .sources
        .iter()
        .map(|s| s.url.as_deref().unwrap())
        .collect();
    assert_eq!(
        urls,
        [
            "https://example.com/t1",
            "https://example.com/y",
            "https://example.com/spec",
            "https://example.com/a",
            "https://example.com/b"
        ]
    );
    let numbers: Vec<_> = p.sources.iter().map(|s| s.number).collect();
    assert_eq!(numbers, [1, 2, 3, 4, 5]);
    assert_eq!(
        p.summary.iter().map(|s| s.source).collect::<Vec<_>>(),
        [4, 3]
    );
    assert_eq!(p.issues[0].source, 5);
    assert_eq!(p.pictures.len(), 1);
    assert_eq!(p.pictures[0].alt, "Test 1: maker's photo (dial side)");
    assert_eq!(
        p.pictures[0].url,
        "https://raw.githubusercontent.com/peterhudson/timegrapher-calibres/main/test/test-1/front.jpg"
    );
    assert_eq!(p.documents[0].kind, "Spec sheet");
    assert_eq!(p.documents[0].file, "sheet.pdf");
}

/// A relation reads from the row that states it with its source, and
/// from the other row as its inverse, which cites none there (#1861).
#[test]
fn related_calibres_read_both_ways() {
    let pm = page_of(find("Powermatic 80").unwrap());
    let based = pm
        .related
        .iter()
        .find(|r| r.label == "Based on")
        .expect("Based on line");
    assert_eq!(based.calibres[0].name, "ETA 2824-2");
    assert_eq!(based.calibres[0].path, "/calibres/eta/2824-2/");
    let n = based.source.expect("its source");
    assert!(pm.sources[n - 1].url.as_deref().unwrap().contains("tissot"));
    let eta = page_of(find("ETA 2824-2").unwrap());
    let base_of = eta
        .related
        .iter()
        .find(|r| r.label == "Base calibre for")
        .expect("Base calibre for line");
    assert!(base_of.source.is_none());
    let all_named: Vec<_> = base_of
        .calibres
        .iter()
        .chain(base_of.more.iter().flat_map(|m| &m.calibres))
        .map(|l| l.name.as_str())
        .collect();
    assert!(all_named.contains(&"ETA Powermatic 80"), "{all_named:?}");
    // "same base as" is stored on one row of each pair; the other row
    // reads it back, citing nothing there.
    let same = eta
        .related
        .iter()
        .find(|r| r.label == "Same base as")
        .expect("Same base as line on ETA 2824-2");
    assert!(same.source.is_none());
    let names: Vec<_> = same.calibres.iter().map(|l| l.name.as_str()).collect();
    assert!(
        names.contains(&"ETA 2836-2") && names.contains(&"ETA 2834-2"),
        "{names:?}"
    );
    let sister = page_of(find("ETA 2836-2").unwrap());
    let stated: Vec<_> = sister
        .related
        .iter()
        .filter(|r| r.label == "Same base as")
        .collect();
    assert!(!stated.is_empty() && stated.iter().all(|r| r.source.is_some()));
}

/// A pair stored from both sides is shown once on each, as stated.
#[test]
fn a_same_base_pair_stored_both_ways_is_shown_once() {
    let src = "https://example.com/x (x) (x)";
    let c = row(&format!(
        r#", "related": [{{"calibre": "ETA 2836-2", "relation": "same base as", "source": "{src}"}}]"#
    ));
    let mut table: Vec<Calibre> = all().to_vec();
    let mut back = find("ETA 2836-2").unwrap().clone();
    back.related.push(calibres::Related {
        calibre: "Test 1".into(),
        relation: "same base as".into(),
        source: src.into(),
    });
    table.push(back);
    let p = page(&c, &inverses(&table));
    let lines: Vec<_> = p
        .related
        .iter()
        .filter(|r| r.label == "Same base as")
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].source.is_some());
}

/// A name with nothing for an address after its maker keeps its whole
/// name, and a former name with the page's own address, or one listed
/// twice, gives no redirect.
#[test]
fn addresses_never_come_out_empty_or_point_at_themselves() {
    assert_eq!(page_path("Test", "Test +"), "/calibres/test/test/");
    let c = row(r#", "former_names": ["Test 1.", "Test 0", "Test 0."]"#);
    assert_eq!(page_of(&c).former_paths, ["/calibres/test/0/"]);
}

/// The index lists makers by name, ignoring case, with the big makers
/// marked.
#[test]
fn makers_are_listed_by_name_with_the_big_ones_marked() {
    let m = export().makers;
    let names: Vec<String> = m.iter().map(|x| x.maker.to_lowercase()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    let big = calibres::makers()
        .iter()
        .find(|x| x.common && m.iter().any(|e| e.maker == x.maker))
        .expect("a big maker with calibres");
    assert!(m.iter().any(|x| x.maker == big.maker && x.common));
    let small = calibres::makers()
        .iter()
        .find(|x| !x.common && m.iter().any(|e| e.maker == x.maker))
        .expect("a smaller maker with calibres");
    assert!(m.iter().any(|x| x.maker == small.maker && !x.common));
}

#[test]
fn the_pane_and_the_page_share_their_words() {
    assert_eq!(
        also_known_as(&row(r#", "family": ["2824", "SW200"]"#)).as_deref(),
        Some("Also known as 2824, SW200")
    );
    assert_eq!(also_known_as(&row("")), None);
    assert_eq!(also_known_as(&row(r#", "family": ["1"]"#)), None);
    assert_eq!(sources_heading("maker's page"), "Sources · Maker's Page");
    let x = calibres::Escapement {
        kind: "co-axial".into(),
        source: "https://example.com/c (c) (co-axial)".into(),
    };
    let said = escapement_part(&x);
    assert_eq!(said.text, x.sentence());
    assert_eq!(said.hover, "Source: https://example.com/c (c) (co-axial)");
    let c = row(&format!(
        r#", "escapement": {{"kind": "co-axial", "source": "{}"}}"#,
        x.source
    ));
    assert_eq!(page_of(&c).escapement.unwrap().hover, said.hover);
}

#[test]
fn a_long_related_list_names_five_and_counts_the_rest() {
    let links: Vec<Link> = (0..7)
        .map(|i| Link {
            name: format!("C{i}"),
            path: format!("/calibres/x/c{i}/"),
        })
        .collect();
    let line = related_line("Cloned by".into(), links, None);
    assert_eq!(line.calibres.len(), RELATED_SHOWN);
    let more = line.more.expect("more");
    assert_eq!(more.text, "+2 more");
    assert_eq!(more.calibres.len(), 2);
    let short = related_line(
        "Cloned by".into(),
        vec![link(find("ETA 2824-2").unwrap())],
        None,
    );
    assert!(short.more.is_none());
}

#[test]
fn figures_say_which_the_table_does_not_give() {
    assert_eq!(
        figure_words(Some(21600), Some(51.5), Some(15)),
        ["21600 bph", "lift angle 51.5°", "15-tooth escape wheel"]
    );
    assert_eq!(
        figure_words(None, None, None),
        ["beat rate not given", "lift angle not given"]
    );
}

/// The export's JSON keeps the shape the site reads: headings, makers,
/// calibres, and nothing empty written out.
#[test]
fn the_export_serialises_with_its_headings() {
    let v = serde_json::to_value(export()).expect("export serialises");
    assert_eq!(v["headings"]["related"], "Related movements");
    assert_eq!(v["headings"]["issues"], "Reported issues");
    let eta = v["calibres"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "ETA 2824-2")
        .expect("ETA 2824-2");
    assert_eq!(eta["path"], "/calibres/eta/2824-2/");
    let bare = serde_json::to_value(page_of(&row(""))).unwrap();
    for key in ["specs", "summary", "pictures", "related", "made", "notes"] {
        assert!(bare.get(key).is_none(), "{key}");
    }
}

/// A source reads as its name, its link and what it says, without
/// the calibre's own name the panel is headed by.
#[test]
fn a_source_is_its_name_and_what_it_says() {
    assert_eq!(
        source_parts("https://watchguy.co.uk/cgi-bin/lift_angles (A. Schild [AS] 1017, 51°)"),
        (
            "watchguy.co.uk".to_string(),
            Some("https://watchguy.co.uk/cgi-bin/lift_angles".to_string()),
            "51°".to_string()
        )
    );
    let (name, _, says) = source_parts(
        "https://docs.google.com/spreadsheets/d/x (lift angle sheet linked from \
         watchrepairtalk.com/topic/27749-lift-angle) (IWC (International Watch Company) \
         45, 51°, 21600 vph)",
    );
    assert_eq!(name, "Lift angle sheet (watchrepairtalk.com)");
    assert_eq!(says, "51°, 21600 vph");
    let (name, url, says) = source_parts("https://www.sellita.ch/en/sw200");
    assert_eq!((name.as_str(), says.as_str()), ("sellita.ch", ""));
    assert!(url.is_some());
    let (name, url, _) = source_parts("project's bench recordings: a 5.00 s rate");
    assert_eq!(name, "project's bench recordings: a 5.00 s rate");
    assert!(url.is_none());
}

#[test]
fn the_maker_is_named_once() {
    assert_eq!(with_maker_name("Rolex", "Rolex 3235"), "Rolex 3235");
    assert_eq!(with_maker_name("Rolex", "rolex 3235"), "rolex 3235");
    assert_eq!(with_maker_name("ETA", "2824-2"), "ETA 2824-2");
    // A name that only begins with the same letters is a different word.
    assert_eq!(
        with_maker_name("Seiko", "SeikoMatic 6218"),
        "Seiko SeikoMatic 6218"
    );
    // Under another of the maker's names.
    assert_eq!(
        with_maker_name("Tianjin Seagull", "Seagull ST36"),
        "Seagull ST36"
    );
    // A clone stays marked as one.
    assert_eq!(
        with_maker_name("Shanghai (clone)", "Shanghai 4401"),
        "Shanghai 4401 (clone)"
    );
    for name in ["Patek 240 Clone", "Patek 240 (clone)", "Patek 240 CLONE"] {
        assert_eq!(with_maker_name("(clone)", name), name);
    }
    assert_eq!(
        with_maker_name("Shanghai (clone)", "Patek 240 Clone"),
        "Shanghai Patek 240 Clone"
    );
    // Do not lose the clone marker when it is absent or only a substring.
    assert_eq!(with_maker_name("(clone)", "240"), "(clone) 240");
    assert_eq!(
        with_maker_name("(clone)", "Clonecraft 240"),
        "(clone) Clonecraft 240"
    );
}

#[test]
fn no_row_in_the_table_names_its_maker_twice() {
    let mut twice = Vec::new();
    for c in calibres::all() {
        let row = with_maker_name(&c.maker, &c.calibre);
        let words: Vec<String> = row
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect();
        let repeated = maker_words(&c.maker).iter().any(|name| {
            name.split(|ch: char| !ch.is_alphanumeric())
                .filter(|w| w.len() > 2)
                .any(|w| words.iter().filter(|x| **x == w.to_lowercase()).count() > 1)
        });
        if repeated {
            twice.push(row);
        }
    }
    assert!(
        twice.is_empty(),
        "{} rows: {:?}",
        twice.len(),
        &twice[..twice.len().min(10)]
    );
}

/// One address is one numbered source, listing each thing quoted from it
/// (Hugh on #2281: Miyota 8215 had five numbers for one page).
#[test]
fn one_address_is_one_source_with_its_quotes() {
    let c = row(
        r#", "jewels": {"value": 21, "source": "https://example.com/8215 (maker's page) (21 jewels)"},
           "height_mm": {"value": "5.67", "source": "https://example.com/8215 (maker's page) (5.67 mm)"}"#,
    );
    let p = page_of(&c);
    assert_eq!(p.sources.len(), 2);
    assert_eq!(p.sources[1].quotes, ["21 jewels", "5.67 mm"]);
    let parts = &p.specs.as_ref().unwrap().parts;
    assert_eq!(parts[0].source, Some(2));
    assert_eq!(parts[1].source, Some(2));
}

/// Wheels run barrel to balance, then the rest as the table lists them.
#[test]
fn wheels_are_listed_in_train_order() {
    let c = row(
        r#", "wheels": [{"name": "escape wheel", "period_s": 5}, {"name": "cannon pinion", "period_s": 3600},
            {"name": "barrel", "period_s": 25200}, {"name": "balance", "period_s": 0.25},
            {"name": "centre wheel", "period_s": 3600}]"#,
    );
    let names: Vec<_> = page_of(&c).wheels.into_iter().map(|w| w.name).collect();
    assert_eq!(
        names,
        [
            "Barrel",
            "Centre Wheel",
            "Escape Wheel",
            "Balance",
            "Cannon Pinion"
        ]
    );
}

/// The page's address leaves out the maker under any of its names.
#[test]
fn the_address_names_the_maker_once() {
    let c = row(r#""#);
    assert_eq!(
        page_path("Miyota (Citizen)", "Miyota 8215"),
        "/calibres/miyota-citizen/8215/"
    );
    assert_eq!(page_path("Rolex", "Rolex 3135"), "/calibres/rolex/3135/");
    assert_eq!(page_path("ETA", "eta 2824-2"), "/calibres/eta/2824-2/");
    assert_eq!(
        page_path("Seiko", "SeikoMatic 6218"),
        "/calibres/seiko/seikomatic-6218/"
    );
    assert_eq!(path(&c), "/calibres/test/1/");
    // The longer name goes first: not "piguet-15202".
    assert_eq!(
        page_path("Audemars Piguet", "Audemars Piguet 15202"),
        "/calibres/audemars-piguet/15202/"
    );
}
