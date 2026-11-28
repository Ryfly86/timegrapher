//! The calibre picker, manager and store, headless.

use super::*;

/// A watch's own stored lift outside the one range is set aside, as a
/// calibre's is: the take starts on the default.
#[test]
fn a_watchs_lift_outside_the_range_is_set_aside() {
    let cal = Calibres::in_memory();
    for (lift, kept) in [(120.0, None), (5.0, None), (48.0, Some(48.0))] {
        let mut w = timegrapher_core::library::Watch::new("Test");
        w.lift = Some(lift);
        assert_eq!(cal.watch_lift(&w), kept, "{lift}");
    }
}
/// The search over the table finds a calibre by maker and name
/// together, the count says how many it shows, and a missing figure
/// is left to the cell's dash, not written into the filter's values as
/// a degree.
#[test]
fn the_table_search_finds_maker_and_calibre_together() {
    let mut cal = Calibres::in_memory();
    let all = cal.row_names().len();
    cal.search = "schild 1017".into();
    let found = cal.row_names();
    assert!(
        found.iter().any(|(_, n)| n == "A. Schild 1017"),
        "{found:?}"
    );
    assert!(found.len() < 5, "{found:?}");
    cal.search.clear();
    assert_eq!(cal.row_names().len(), all);
    assert_eq!(count_words(4537), "4,537 calibres");
    assert_eq!(count_words(1), "1 calibre");
    assert_eq!(shown_of(4537, 4537), "4,537 calibres");
    assert_eq!(shown_of(6, 4537), "6 of 4,537 calibres");
    let schild = cal.find("A. Schild 1017").unwrap();
    assert_eq!(cal.cell(schild, Col::LiftAngle), "51");
}
#[test]
fn saved_picks_follow_the_tables_renames() {
    let c = calibres::all()
        .iter()
        .find(|c| !c.former_names.is_empty())
        .expect("a renamed calibre in the table");
    let (old, new) = (c.former_names[0].clone(), c.calibre.clone());
    let mut cal = Calibres::in_memory();
    assert_eq!(cal.current_name(&old), new);
    assert_eq!(cal.current_name("Not in the table"), "Not in the table");
    // Saved under both names: one row, and on the short list either way.
    cal.restore(
        vec![old.clone(), new.clone()],
        vec![old.clone()],
        vec![old.clone()],
    );
    assert_eq!(cal.recent, vec![new.clone()]);
    assert_eq!(cal.short_added, vec![new.clone()]);
    assert_eq!(cal.short_removed, vec![new.clone()]);
    assert!(cal.find(&new).is_some_and(|e| e.name() == new));
}
fn store_fixture(label: &str) -> (PathBuf, PathBuf) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tg-calibre-protection-{label}-{}-{id}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("calibres.json");
    (dir, path)
}

fn legacy_calibre() -> settings::CustomCalibre {
    settings::CustomCalibre {
        name: "Legacy Bench".into(),
        wheels: vec![settings::CustomWheel {
            name: " Third Wheel ".into(),
            period_s: 450.0,
        }],
    }
}

#[test]
fn unreadable_catalogue_is_never_replaced_by_fallback_edits() {
    for bytes in [
        b"{ invalid catalogue, preserve me".as_slice(),
        br#"{"schema":"timegrapher.user-calibres/future","custom":[],"wheels":[]}"#,
    ] {
        let (dir, path) = store_fixture("blocked");
        std::fs::write(&path, bytes).unwrap();
        let mut c = Calibres::open(Some(path.clone()));
        assert!(c.user.custom.is_empty());
        let problem = c.problem.clone().expect("load failure is visible");
        assert!(problem.contains(&path.display().to_string()));
        assert!(c.find("Rolex 4130").is_some(), "built-ins remain usable");
        c.new_custom("Temporary New");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        c.adopt("Temporary Imported", &[("third wheel".into(), 123.0)]);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        c.migrate(vec![legacy_calibre()]);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(c.pending_legacy(), &[legacy_calibre()]);
        // Wheel changes and custom editor updates share this save boundary.
        c.user.set_wheel("Rolex 4130", "third wheel", 400.0, None);
        c.user.remove_custom("Temporary New");
        c.save();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(c.problem.as_ref(), Some(&problem));
        assert!(problem.contains("only in memory"));
        assert!(!path.with_extension("json.tmp").exists());
        // Repairing externally does not authorize this empty-fallback
        // instance to overwrite a catalogue it has never loaded.
        let mut repaired = UserCalibres::default();
        repaired.put_custom(custom("Repaired Original", [("third wheel", 321.0)]));
        repaired.save(&path).unwrap();
        let repaired_bytes = std::fs::read(&path).unwrap();
        c.new_custom("Still Temporary");
        assert_eq!(std::fs::read(&path).unwrap(), repaired_bytes);
        let reopened = Calibres::open(Some(path));
        assert!(reopened.problem.is_none());
        assert!(reopened.find("Repaired Original").is_some());
        assert!(reopened.find("Still Temporary").is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn unreadable_directory_catalogue_stays_blocked_after_path_repair() {
    let (dir, path) = store_fixture("directory");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("original"), b"keep").unwrap();
    let mut c = Calibres::open(Some(path.clone()));
    c.new_custom("Temporary");
    assert_eq!(std::fs::read(path.join("original")).unwrap(), b"keep");
    std::fs::remove_dir_all(&path).unwrap();
    c.new_custom("Still Temporary");
    assert!(
        !path.exists(),
        "a repaired path does not unblock this instance"
    );
    assert!(c.problem.as_ref().unwrap().contains("only in memory"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_settings_survive_failed_save_until_successful_retry() {
    let (dir, path) = store_fixture("retry");
    let mut c = Calibres::open(Some(path.clone()));
    // Deterministic rename failure, without permission assumptions.
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("original"), b"keep").unwrap();
    let old = vec![legacy_calibre()];
    c.migrate(old.clone());
    assert_eq!(c.pending_legacy(), old);
    assert!(c.find("Legacy Bench").is_some());
    assert!(c.problem.as_ref().unwrap().contains("couldn't be saved"));
    assert_eq!(std::fs::read(path.join("original")).unwrap(), b"keep");
    std::fs::remove_dir_all(&path).unwrap();
    // A normal later edit retries the save, including the migrated entry.
    c.adopt("Later Entry", &[("fourth wheel".into(), 60.0)]);
    assert!(c.pending_legacy().is_empty());
    assert!(c.problem.is_none());
    let reopened = Calibres::open(Some(path));
    assert!(reopened.find("Legacy Bench").is_some());
    assert!(reopened.find("Later Entry").is_some());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_valid_and_memory_only_catalogues_keep_their_contracts() {
    let (dir, path) = store_fixture("valid");
    let mut c = Calibres::open(Some(path.clone()));
    assert!(c.problem.is_none());
    c.new_custom("First Entry");
    let mut c = Calibres::open(Some(path.clone()));
    c.adopt("Second Entry", &[("third wheel".into(), 123.0)]);
    c.migrate(vec![legacy_calibre()]);
    assert!(c.pending_legacy().is_empty());
    let reopened = Calibres::open(Some(path));
    for name in ["First Entry", "Second Entry", "Legacy Bench"] {
        assert!(reopened.find(name).is_some(), "{name}");
    }
    let mut memory = Calibres::in_memory();
    memory.new_custom("Memory Entry");
    memory.migrate(vec![legacy_calibre()]);
    assert!(memory.find("Memory Entry").is_some());
    assert!(memory.find("Legacy Bench").is_some());
    assert_eq!(memory.pending_legacy(), &[legacy_calibre()]);
    assert!(memory.problem.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

/// A calibre the search hides isn't shown in full beside the rows the
/// search found, and the count says how many of all are shown.
#[test]
fn a_hidden_calibre_leaves_the_details_and_the_count_says_of_how_many() {
    let mut c = Calibres::in_memory();
    c.selected = Some("A. Schild 1017".into());
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let draw = |c: &mut Calibres| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 900.0),
            )),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run(input.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    c.contents(ui, "", "test");
                });
            }));
        }
        out.unwrap()
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let all = c.every().count();
    let seen = draw(&mut c);
    assert!(
        !seen.iter().any(|t| t.starts_with("Select a calibre")),
        "{seen:?}"
    );
    assert!(seen.contains(&count_words(all)), "{seen:?}");
    c.search = "2824".into();
    let found = c.row_names().len();
    assert!(found < all);
    let seen = draw(&mut c);
    assert!(
        seen.iter().any(|t| t.starts_with("Select a calibre")),
        "{seen:?}"
    );
    assert!(
        seen.contains(&format!("{found} of {}", count_words(all))),
        "{seen:?}"
    );
    // Still selected: clearing the search shows it again.
    assert_eq!(c.selected.as_deref(), Some("A. Schild 1017"));
}

#[test]
fn blocked_catalogue_warning_is_painted_without_a_selection() {
    let (dir, path) = store_fixture("warning");
    std::fs::write(&path, b"invalid JSON").unwrap();
    let mut c = Calibres::open(Some(path));
    assert!(c.selected.is_none());
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            c.manager_body(ui, "", "test", &mut Managed::default());
        });
    });
    let text = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("only in memory"), "{text}");
    assert!(text.contains("will not be overwritten"), "{text}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn settings_calibres_move_to_the_user_file_once() {
    let dir = std::env::temp_dir().join(format!("tg-calibres-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("calibres.json");
    let mut c = Calibres::open(Some(path.clone()));
    assert!(c.problem.is_none());
    let old = vec![settings::CustomCalibre {
        name: "Bench Watch".into(),
        wheels: vec![
            settings::CustomWheel {
                name: " Third Wheel ".into(),
                period_s: 450.0,
            },
            settings::CustomWheel {
                name: String::new(),
                period_s: 10.0,
            },
        ],
    }];
    c.migrate(old.clone());
    let again = Calibres::open(Some(path.clone()));
    assert_eq!(again.user.custom.len(), 1);
    assert_eq!(
        again.user.custom[0].wheels.len(),
        1,
        "the unnamed wheel is left out"
    );
    assert_eq!(again.user.custom[0].wheels[0].name, "third wheel");
    // Moving them again changes nothing.
    let mut again = again;
    again.migrate(old);
    assert_eq!(Calibres::open(Some(path)).user.custom.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Teeth and Leaves (#399): a period the teeth give fills a blank wheel
/// and is captioned From Teeth; one they disagree with keeps the typed
/// figure and says what the teeth give.
#[test]
fn counted_teeth_caption_the_wheels() {
    let mut c = Calibres::in_memory();
    c.user
        .set_count("ETA 2892-A2", "escape wheel", Some(21), None);
    c.user
        .set_count("ETA 2824-2", "fourth wheel", Some(100), None);
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let draw = |c: &mut Calibres, name: &str| {
        c.selected = Some(name.into());
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1440.0, 2000.0),
            )),
            ..Default::default()
        };
        let mut out = None;
        for _ in 0..2 {
            out = Some(ctx.run(input.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    c.contents(ui, "", "test");
                });
            }));
        }
        out.unwrap()
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let seen = draw(&mut c, "ETA 2892-A2");
    assert!(seen.iter().any(|t| t == "From Teeth"), "{seen:?}");
    assert!(
        seen.iter().any(|t| t.ends_with("Teeth and Leaves")),
        "{seen:?}"
    );
    let slots = Catalog::new(&c.user).slots(c.find("ETA 2892-A2").unwrap());
    let esc = slots.iter().find(|s| s.name == "escape wheel").unwrap();
    assert_eq!(esc.period_s, Some(5.25));
    let seen = draw(&mut c, "ETA 2824-2");
    assert!(seen.iter().any(|t| t == "Teeth give 55.556 s"), "{seen:?}");
}

/// Every period box at 800 wide shows its value with no digit under its
/// right edge: whole up to six figures, a longer one with decimals
/// dropped (#1586). The Add a Wheel box too.
#[test]
fn a_period_box_shows_its_whole_value() {
    let mut c = Calibres::in_memory();
    c.user
        .set_wheel("ETA 2824-2", "third wheel", 999_999.999, None);
    c.user
        .set_wheel("ETA 2824-2", "fourth wheel", 9_999_999.999, None);
    c.selected = Some("ETA 2824-2".into());
    c.adding = Some(Adding {
        calibre: "ETA 2824-2".into(),
        wheel: "moon wheel".into(),
        period: "21114.027".into(),
    });
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(800.0, 2000.0),
        )),
        ..Default::default()
    };
    // A group row's control grows to its width over a few frames, each
    // one laid in the width the last one took, as the app repaints it.
    let mut out = None;
    for _ in 0..12 {
        out = Some(ctx.run(input.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                c.contents(ui, "", "test");
            });
        }));
    }
    // A text box clips its text 1 px outside its inner edge; nothing else
    // here is clipped that narrow.
    let in_boxes: Vec<(String, f32)> = out
        .unwrap()
        .shapes
        .iter()
        .filter(|s| s.clip_rect.width() < 200.0)
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((
                t.galley.text().to_string(),
                s.clip_rect.max.x - 1.0 - (t.pos.x + t.galley.rect.right()),
            )),
            _ => None,
        })
        .collect();
    // The barrel's box and the Add a Wheel box.
    let barrels = in_boxes.iter().filter(|(t, _)| t == "21114.027").count();
    assert!(barrels >= 2, "{in_boxes:?}");
    for want in ["999999.999", "10000000"] {
        assert!(
            in_boxes.iter().any(|(t, _)| t == want),
            "no box shows {want}: {in_boxes:?}"
        );
    }
    for (t, gap) in &in_boxes {
        assert!(*gap >= 0.0, "{t} cut by {} px", -gap);
    }
}

/// A period cut to fit its box gives the full value in its hover; one
/// shown whole keeps the hover as it was (#1586).
#[test]
fn a_cut_period_gives_its_full_value_in_the_hover() {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let mut got = Vec::new();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            for v in [Some(9_999_999.999), Some(21114.027), None] {
                got.push(super::period_shown(ui, v, "The barrel's turn."));
            }
        });
    });
    let full = "The barrel's turn.\n\nIn full: 9999999.999 s";
    assert_eq!(got[0], ("10000000".into(), full.into()));
    assert_eq!(got[1], ("21114.027".into(), "The barrel's turn.".into()));
    assert_eq!(got[2], (String::new(), "The barrel's turn.".into()));
}

#[test]
fn wheels_typed_for_a_listed_calibre_name_its_cycles() {
    let mut c = Calibres::in_memory();
    let name = &calibres::common().next().unwrap().calibre;
    let before = c.wheels(name).unwrap().len();
    c.user.set_wheel(name, "third wheel", 450.0, None);
    let after = c.wheels(name).unwrap();
    assert!(after
        .iter()
        .any(|w| w.name == "third wheel" && w.period_s == 450.0));
    assert!(after.len() >= before);
    assert!(c.wheels("").is_none());
}

#[test]
fn the_short_list_starts_common_and_follows_the_ticks() {
    let mut c = Calibres::in_memory();
    let common = calibres::common().next().unwrap();
    let rare = calibres::all().iter().find(|x| !x.common).unwrap();
    assert!(c.in_short(common) && !c.in_short(rare));
    c.set_short(&common.calibre, false);
    c.set_short(&rare.calibre, true);
    assert!(!c.in_short(common) && c.in_short(rare));
    assert_eq!(c.short_removed, std::slice::from_ref(&common.calibre));
    assert_eq!(c.short_added, std::slice::from_ref(&rare.calibre));
    c.set_short(&common.calibre, true);
    c.set_short(&rare.calibre, true);
    assert!(c.short_removed.is_empty() && c.short_added.len() == 1);
    c.set_short("no such calibre", true);
    assert_eq!(c.short_added.len(), 1);
}

#[test]
fn recent_calibres_put_the_latest_first() {
    let mut c = Calibres::in_memory();
    for n in ["A", "B", "C", "A", "D", "E", "F", "G"] {
        c.remember(n);
    }
    assert_eq!(c.recent, ["G", "F", "E", "D", "A", "C"]);
    c.remember("");
    assert_eq!(c.recent.len(), RECENT);
}

#[test]
fn a_new_custom_calibre_takes_the_typed_name_when_free() {
    let mut c = Calibres::in_memory();
    assert_eq!(c.new_custom("Bench 1"), "Bench 1");
    assert_eq!(c.new_custom("Bench 1"), "My Calibre 2");
    let listed = &calibres::all()[0].calibre;
    assert_eq!(c.new_custom(listed), "My Calibre 3");
    // A custom calibre may be obscure or vintage: no wheel is assumed.
    assert!(c.wheels("Bench 1").unwrap().is_empty());
}

#[test]
fn a_custom_balance_period_follows_its_beat_rate() {
    let mut k = custom("Bench", []);
    let balance = |k: &CustomCalibre| {
        k.wheels
            .iter()
            .find(|w| w.name == "balance")
            .map(|w| w.period_s)
    };
    k.set_bph(Some(18000));
    assert_eq!(balance(&k), Some(0.4));
    k.set_bph(Some(28800));
    assert_eq!(balance(&k), Some(0.25));
    // A period typed by hand stays when the beat rate changes.
    k.wheels[0].period_s = 0.3;
    k.set_bph(Some(21600));
    assert_eq!(balance(&k), Some(0.3));
    // No beat rate, no derived balance.
    let mut k = custom("Bench", []);
    k.set_bph(Some(36000));
    k.set_bph(None);
    assert_eq!(balance(&k), None);
}

/// A funnel's menu keeps its height whatever its search leaves listed:
/// every value, a few, or none.
#[test]
fn a_funnel_menu_keeps_its_height_as_you_search() {
    let mut c = Calibres::in_memory();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 900.0));
    let height = |c: &mut Calibres, typed: &str| {
        c.funnel_search = typed.to_string();
        let mut h = 0.0;
        for _ in 0..2 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.set_width(230.0);
                        let r = ui.scope(|ui| c.funnel_menu(ui, Col::Calibre, "Calibre"));
                        h = r.response.rect.height();
                    });
                },
            );
        }
        h
    };
    let all = height(&mut c, "");
    let few = c
        .funnel_values(Col::Calibre)
        .iter()
        .filter(|v| v.contains("2824"))
        .count();
    assert!(few > 0 && few < 9, "{few}");
    assert_eq!(height(&mut c, "2824"), all);
    assert_eq!(height(&mut c, "no such calibre"), all);
    assert!(all > 300.0, "{all}");
}

/// A funnel's search in the Calibre column also finds a calibre by one
/// of its other names, as the Calibre menu's does, and the Maker
/// column's lists only what it finds.
#[test]
fn a_funnel_search_finds_other_names() {
    let mut c = Calibres::in_memory();
    let (cal, other) = calibres::all()
        .iter()
        .find_map(|cal| {
            let name = Entry::Table(cal).name().to_lowercase();
            cal.family
                .iter()
                .find(|f| {
                    f.len() > 3
                        && f.chars().all(|ch| ch.is_alphanumeric() || ch == ' ')
                        && !name.contains(&f.to_lowercase())
                })
                .map(|f| (cal, f.clone()))
        })
        .expect("a calibre with another name its own doesn't hold");
    c.funnel_search = other.clone();
    let listed = c.funnel_listed(Col::Calibre);
    let name = c.cell(Entry::Table(cal), Col::Calibre);
    assert!(
        listed.contains(&name),
        "{other} doesn't list {name}: {listed:?}"
    );
    // Kept while the search stays; a new search lists afresh.
    assert_eq!(c.funnel_listed(Col::Calibre), listed);
    c.funnel_search = "no such calibre".into();
    assert!(c.funnel_listed(Col::Calibre).is_empty());
    c.funnel_search = "eta".into();
    let makers = c.funnel_listed(Col::Maker);
    assert!(makers.iter().any(|m| m == "ETA"), "{makers:?}");
    assert!(makers.len() < c.funnel_values(Col::Maker).len());
}

#[test]
fn a_column_filter_applies_only_on_ok() {
    let mut c = Calibres::in_memory();
    let offered = c.funnel_values(Col::BeatRate);
    assert!(offered.len() > 2);
    let one = offered[0].clone();
    // Untick every value but one, as in the menu, before OK.
    for v in &offered[1..] {
        c.draft_set(&offered, v, false);
    }
    assert!(
        c.funnels[Col::BeatRate as usize].is_none(),
        "nothing applied before OK"
    );
    let draft = c.funnel_draft.take();
    assert_eq!(draft.as_ref().map(|k| k.len()), Some(1));
    c.funnels[Col::BeatRate as usize] = draft;
    assert!(c.rows().iter().all(|e| c.cell(*e, Col::BeatRate) == one));
    // Ticking every value again is no filter at all.
    c.funnel_draft = c.funnels[Col::BeatRate as usize].clone();
    for v in &offered[1..] {
        c.draft_set(&offered, v, true);
    }
    assert!(c.funnel_draft.is_none());
}

#[test]
fn every_calibre_is_under_one_maker_common_makers_first() {
    let list = by_maker();
    let n: usize = list.iter().map(|m| m.2.len()).sum();
    assert_eq!(n, calibres::all().len());
    let first_rare = list.iter().position(|m| !m.1).unwrap_or(list.len());
    assert!(list[first_rare..].iter().all(|m| !m.1));
}

#[test]
fn titles_and_summaries_read_plainly() {
    assert_eq!(title_case("escape wheel"), "Escape Wheel");
    let e = Entry::Table(&calibres::all()[0]);
    assert!(summary(e).contains("bph") || summary(e).contains("beat rate not given"));
}

/// Manage Calibres' table: every calibre, the star column's funnel the
/// starred ones, and the custom ones under the maker Custom.
#[test]
fn the_manager_lists_every_calibre_and_the_short_list() {
    let mut cal = Calibres::in_memory();
    let names =
        |c: &Calibres| -> Vec<String> { c.rows().iter().map(|e| e.name().to_string()).collect() };
    assert_eq!(names(&cal).len(), calibres::all().len());
    cal.funnels[Col::Short as usize] = Some(["Starred".to_string()].into());
    let common = calibres::all().iter().filter(|c| cal.in_short(c)).count();
    assert_eq!(names(&cal).len(), common);
    assert!(common < calibres::all().len());
    cal.funnels = Default::default();
    let mine = cal.new_custom("Bench Special");
    assert_eq!(names(&cal).len(), calibres::all().len() + 1);
    let e = cal.find(&mine).unwrap();
    assert_eq!(cal.cell(e, Col::Maker), "Custom");
    cal.funnels[Col::Maker as usize] = Some(["Custom".to_string()].into());
    assert_eq!(names(&cal), vec![mine.clone()]);
    cal.funnels = Default::default();
    assert!(!cal.rows().is_empty());
}

/// Each column's funnel keeps the values ticked; the funnels combine,
/// each offers the values the others let through, and ticking every
/// value again is no filter.
#[test]
fn the_funnels_combine_and_offer_what_the_others_let_through() {
    let mut cal = Calibres::in_memory();
    let makers = cal.funnel_values(Col::Maker);
    assert!(makers.len() > 10 && makers.windows(2).all(|w| w[0] != w[1]));
    let maker = calibres::all()
        .iter()
        .find(|c| c.bph.is_some() && c.maker == "ETA")
        .map_or_else(|| calibres::all()[0].maker.clone(), |c| c.maker.clone());
    // One maker: the rows are that maker's, the Beat Rate funnel offers
    // only that maker's rates.
    for m in &makers {
        if *m != maker {
            cal.set_funnel(Col::Maker, m, false);
        }
    }
    assert_eq!(
        cal.funnels[Col::Maker as usize],
        Some([maker.clone()].into())
    );
    let theirs: Vec<&Calibre> = calibres::all()
        .iter()
        .filter(|c| c.maker == maker)
        .collect();
    assert_eq!(cal.rows().len(), theirs.len());
    let rates = cal.funnel_values(Col::BeatRate);
    for r in &rates {
        assert!(
            theirs
                .iter()
                .any(|c| cal.cell(Entry::Table(c), Col::BeatRate) == *r),
            "{r} is not a rate of {maker}"
        );
    }
    // The Maker funnel still offers every maker: its own filter doesn't
    // narrow what it lists.
    assert_eq!(cal.funnel_values(Col::Maker), makers);
    // A second funnel narrows further: both must match.
    let rate = rates[0].clone();
    for r in &rates[1..] {
        cal.set_funnel(Col::BeatRate, r, false);
    }
    let both = theirs
        .iter()
        .filter(|c| cal.cell(Entry::Table(c), Col::BeatRate) == rate)
        .count();
    assert_eq!(cal.rows().len(), both);
    assert!(cal
        .rows()
        .iter()
        .all(|e| cal.cell(*e, Col::Maker) == maker && cal.cell(*e, Col::BeatRate) == rate));
    // Ticking every rate offered again is no filter on Beat Rate.
    for r in &rates {
        cal.set_funnel(Col::BeatRate, r, true);
    }
    assert!(cal.funnels[Col::BeatRate as usize].is_none());
    assert_eq!(cal.rows().len(), theirs.len());
    // The star column's funnel combines with the others too.
    cal.funnels[Col::Short as usize] = Some(["Starred".to_string()].into());
    assert!(cal
        .rows()
        .iter()
        .all(|e| matches!(e, Entry::Table(c) if cal.in_short(c) && c.maker == maker)));
    // Unticking everything shows nothing; no funnel is every value.
    cal.funnels[Col::Short as usize] = None;
    cal.set_funnel(Col::Maker, &maker, false);
    assert!(cal.rows().is_empty());
    cal.funnels = Default::default();
    assert_eq!(cal.rows().len(), calibres::all().len());
}

/// Manage Calibres… from the Calibre menu with a rate known starts at
/// the calibres at that rate; Settings opened afresh starts unfiltered,
/// and a funnel the user changed is left alone.
#[test]
fn manage_calibres_starts_at_the_rate_known() {
    let mut cal = Calibres::in_memory();
    cal.open_manager("");
    cal.manager_rate = Some((28800, "the rate found from the beats"));
    cal.apply_rate_preset();
    let rows = cal.rows();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|e| e.bph() == Some(28800)));
    assert!(cal.rows().len() < calibres::all().len());
    // The gear: Settings opens the table unfiltered.
    cal.drop_rate_preset();
    assert_eq!(cal.rows().len(), calibres::all().len());
    // No rate known: nothing is set.
    cal.open_manager("");
    cal.apply_rate_preset();
    assert_eq!(cal.rows().len(), calibres::all().len());
    // A funnel changed after the preset stays as the user left it.
    cal.manager_rate = Some((28800, "the rate found from the beats"));
    cal.apply_rate_preset();
    cal.set_funnel(Col::BeatRate, "21600", true);
    cal.drop_rate_preset();
    assert!(cal.funnels[Col::BeatRate as usize].is_some());
}

/// The table's kept rows follow every change they depend on.
#[test]
fn the_kept_rows_follow_the_sort_the_filters_and_the_calibres() {
    let mut cal = Calibres::in_memory();
    let names = |c: &Calibres| -> Vec<(bool, String)> {
        c.rows()
            .iter()
            .map(|e| (e.is_custom(), e.name().to_string()))
            .collect()
    };
    assert_eq!(cal.row_names(), names(&cal));
    cal.sort = (Col::BeatRate, true);
    assert_eq!(cal.row_names(), names(&cal));
    cal.funnels[Col::Maker as usize] = Some(["ETA".to_string()].into());
    assert_eq!(cal.row_names(), names(&cal));
    cal.funnels[Col::Short as usize] = Some(["Starred".to_string()].into());
    assert_eq!(cal.row_names(), names(&cal));
    let first = cal.row_names()[0].1.clone();
    cal.set_short(&first, false);
    assert_eq!(cal.row_names(), names(&cal));
    cal.funnels = Default::default();
    let name = cal.new_custom("Kept Rows Test");
    assert_eq!(cal.row_names(), names(&cal));
    assert_eq!(cal.row_names()[0], (true, name));
}

/// The Calibre column leaves out the maker the Maker column already
/// shows, so rows of one maker differ in what is left: their numbers.
#[test]
fn the_calibre_column_drops_the_makers_name() {
    let cal = Calibres::in_memory();
    let rows = cal.rows();
    let schild = rows
        .iter()
        .find(|e| e.name() == "A. Schild 970")
        .expect("A. Schild 970 listed");
    assert_eq!(short_name(*schild), "970");
    for e in &rows {
        let short = short_name(*e);
        assert!(!short.trim().is_empty(), "{}", e.name());
        assert!(e.name().ends_with(short), "{} -> {short}", e.name());
    }
}

/// The custom calibres stay at the top whatever column the table is
/// sorted on, either way round; the rest follow the sort.
#[test]
fn custom_calibres_stay_at_the_top_whatever_the_sort() {
    let mut cal = Calibres::in_memory();
    let a = cal.new_custom("Zeta Bench");
    let b = cal.new_custom("Alpha Bench");
    for (col, ..) in COLS {
        for reverse in [false, true] {
            cal.sort = (col, reverse);
            let rows = cal.rows();
            let top: Vec<&str> = rows[..2].iter().map(|e| e.name()).collect();
            assert!(
                top.contains(&a.as_str()) && top.contains(&b.as_str()),
                "{col:?} {reverse}: {top:?}"
            );
            assert!(rows[2..].iter().all(|e| !e.is_custom()));
            // The listed ones in the column's order.
            for w in rows[2..].windows(2) {
                assert_ne!(
                    cal.order(&w[0], &w[1]),
                    std::cmp::Ordering::Greater,
                    "{col:?} {reverse}: {} before {}",
                    w[0].name(),
                    w[1].name()
                );
            }
        }
    }
    cal.sort = (Col::Calibre, false);
    let names: Vec<&str> = cal.rows()[..2].iter().map(|e| e.name()).collect();
    assert_eq!(names, [b.as_str(), a.as_str()]);
    cal.sort = (Col::Calibre, true);
    let names: Vec<&str> = cal.rows()[..2].iter().map(|e| e.name()).collect();
    assert_eq!(names, [a.as_str(), b.as_str()]);
    // A calibre with no beat rate comes after every rate, either way
    // round.
    for reverse in [false, true] {
        cal.sort = (Col::BeatRate, reverse);
        let rows = cal.rows();
        let first_blank = rows[2..].iter().position(|e| e.bph().is_none());
        if let Some(i) = first_blank {
            assert!(rows[2 + i..].iter().all(|e| e.bph().is_none()));
        }
        assert!(rows[2].bph().is_some());
    }
}

/// Enter in a funnel is OK: the values ticked become the column's filter,
/// as a click on OK makes them; without it nothing changes.
#[test]
fn enter_in_a_funnel_keeps_its_ticks() {
    let mut c = Calibres::in_memory();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 900.0));
    let run = |c: &mut Calibres, events: Vec<egui::Event>| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(230.0);
                    c.funnel_menu(ui, Col::Maker, "Maker");
                });
            },
        );
    };
    run(&mut c, vec![]);
    let one: std::collections::BTreeSet<String> =
        c.funnel_values(Col::Maker).into_iter().take(1).collect();
    c.funnel_draft = Some(one.clone());
    run(&mut c, vec![]);
    assert!(
        c.funnels[Col::Maker as usize].is_none(),
        "nothing kept before OK"
    );
    let enter = |pressed| egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    run(&mut c, vec![enter(true), enter(false)]);
    let kept = c.funnels[Col::Maker as usize].clone().expect("Enter is OK");
    assert_eq!(kept, one);
}

/// A search typed in a funnel narrows what OK keeps to the values it
/// lists, as a spreadsheet's filter keeps its search results (Walter's
/// walk of #1192: "rol" then Enter left the table unfiltered).
#[test]
fn a_funnels_search_narrows_what_ok_keeps() {
    let mut c = Calibres::in_memory();
    let ctx = egui::Context::default();
    theme::install(&ctx);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 900.0));
    let run = |c: &mut Calibres, events: Vec<egui::Event>| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.set_width(230.0);
                    c.funnel_menu(ui, Col::Maker, "Maker");
                });
            },
        );
    };
    run(&mut c, vec![]);
    run(&mut c, vec![egui::Event::Text("rol".into())]);
    let listed: std::collections::BTreeSet<String> =
        c.funnel_listed(Col::Maker).into_iter().collect();
    assert!(!listed.is_empty() && listed.len() < c.funnel_values(Col::Maker).len());
    let enter = |pressed| egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    run(&mut c, vec![enter(true), enter(false)]);
    assert_eq!(c.funnels[Col::Maker as usize], Some(listed));
}

/// The Lift Angle column is wide enough for the not-supported tag, the
/// room taken from the Calibre column alone (#1872).
#[test]
fn the_lift_column_fits_the_not_supported_tag() {
    let ctx = egui::Context::default();
    let mut min = 0.0;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| min = lift_min_w(ui));
    });
    assert!(min > NUMBER_PAD);
    for w in [440.0, 700.0, 1200.0] {
        let plain = col_widths(w, 0.0);
        let widened = col_widths(w, min);
        assert!(widened[3] >= min, "{w}");
        assert_eq!(widened[0], plain[0]);
        assert_eq!(widened[2], plain[2]);
        assert_eq!(widened[4], plain[4]);
        let sum = |c: [f32; 5]| c.iter().sum::<f32>();
        assert!((sum(widened) - sum(plain)).abs() < 1e-3, "{w}");
    }
}

/// Typing into a custom calibre's Name or Maker, one key a frame, as
/// a person types: each key's frame, then Enter.
struct Typing {
    ctx: egui::Context,
}

impl Typing {
    fn new() -> Self {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        Typing { ctx }
    }

    fn frame(&self, c: &mut Calibres, name: &str, events: Vec<egui::Event>) -> Change {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 900.0));
        let mut change = Change::default();
        let _ = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    change = c.custom_editor(ui, name, "manager");
                });
            },
        );
        change
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> Vec<egui::Event> {
        [true, false]
            .map(|pressed| egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            })
            .into()
    }

    /// Focus the field `what`, select all of it, then type `text` a
    /// character a frame: returns every name the calibre had meanwhile.
    fn type_into(&self, c: &mut Calibres, name: &str, what: &str, text: &str) -> Vec<String> {
        self.frame(c, name, vec![]);
        self.ctx
            .memory_mut(|m| m.request_focus(custom_field_id(what, "manager", name)));
        self.frame(c, name, vec![]);
        self.frame(c, name, Self::key(egui::Key::A, egui::Modifiers::COMMAND));
        let mut seen = Vec::new();
        for ch in text.chars() {
            let change = self.frame(c, name, vec![egui::Event::Text(ch.to_string())]);
            assert!(change.renamed.is_none(), "renamed while typing {ch:?}");
            seen.extend(c.user.custom.iter().map(|k| k.name.clone()));
        }
        seen
    }

    fn enter(&self, c: &mut Calibres, name: &str) -> Change {
        self.frame(c, name, Self::key(egui::Key::Enter, egui::Modifiers::NONE))
    }
}

/// A custom calibre's name is kept whole, spaces and all, on Enter, and
/// nothing is renamed or saved while it is typed: a space typed was
/// kept untrimmed while the editor looked for the trimmed name, and the
/// Name box went (#2481).
#[test]
fn a_custom_calibres_name_is_kept_on_enter_not_on_each_key() {
    let mut c = Calibres::in_memory();
    let name = c.new_custom("");
    let t = Typing::new();
    let seen = t.type_into(&mut c, &name, "custom name", "Bench 8500 co-axial");
    assert!(seen.iter().all(|n| *n == name), "{seen:?}");
    let change = t.enter(&mut c, &name);
    assert_eq!(change.renamed.as_deref(), Some("Bench 8500 co-axial"));
    assert_eq!(c.user.custom[0].name, "Bench 8500 co-axial");
}

/// A name the list already has goes back to the calibre's own on Enter,
/// saying why under the box; Escape puts back what was typed (#2481).
#[test]
fn a_taken_name_or_escape_leaves_the_custom_name_as_it_was() {
    let mut c = Calibres::in_memory();
    let name = c.new_custom("");
    let t = Typing::new();
    t.type_into(&mut c, &name, "custom name", "Omega 8500");
    assert!(t.enter(&mut c, &name).renamed.is_none());
    assert_eq!(c.user.custom[0].name, name);
    let taken = custom_field_id("custom name taken", "manager", &name);
    let said = t.ctx.data(|d| d.get_temp::<String>(taken));
    assert_eq!(said.as_deref(), Some("Omega 8500"));
    assert!(taken_words("Omega 8500").contains("in the list already"));
    t.type_into(&mut c, &name, "custom name", "Bench 9");
    t.frame(
        &mut c,
        &name,
        Typing::key(egui::Key::Escape, egui::Modifiers::NONE),
    );
    assert_eq!(c.user.custom[0].name, name);
    t.frame(&mut c, &name, vec![]);
    assert!(
        t.ctx.data(|d| d.get_temp::<String>(taken)).is_none(),
        "typing again, then Escape, takes back the note"
    );
    // A name kept clears the note.
    t.type_into(&mut c, &name, "custom name", "Bench 9");
    assert!(t.enter(&mut c, &name).renamed.is_some());
    assert!(t.ctx.data(|d| d.get_temp::<String>(taken)).is_none());
}

/// A maker with a space in it types in order: the space isn't trimmed
/// away before the next word (#2481).
#[test]
fn a_custom_makers_space_is_kept_while_typing() {
    let mut c = Calibres::in_memory();
    let name = c.new_custom("");
    let t = Typing::new();
    t.type_into(&mut c, &name, "custom maker", "La Joux ");
    assert_eq!(c.user.custom[0].maker, None, "nothing kept while typing");
    t.enter(&mut c, &name);
    assert_eq!(c.user.custom[0].maker.as_deref(), Some("La Joux"));
}

/// A name is kept trimmed: a space typed last is dropped on Return,
/// and one typed between words stays (#2481, the space that lost the
/// Name box on main).
#[test]
fn a_custom_name_is_kept_trimmed() {
    let mut c = Calibres::in_memory();
    let name = c.new_custom("");
    let t = Typing::new();
    t.type_into(&mut c, &name, "custom name", "Bench 9 ");
    assert_eq!(t.enter(&mut c, &name).renamed.as_deref(), Some("Bench 9"));
    assert_eq!(c.user.custom[0].name, "Bench 9");
}

/// What is typed for one custom calibre, and the note on a name
/// refused, stay with that calibre when another is selected before
/// Return (Trevor 82 on #2482).
#[test]
fn typing_for_one_custom_calibre_never_renames_another() {
    let mut c = Calibres::in_memory();
    let a = c.new_custom("Bench A");
    let b = c.new_custom("Bench B");
    let t = Typing::new();
    t.type_into(&mut c, &a, "custom name", "Bench C");
    // B selected before Return: A's box goes, and Return is B's.
    t.frame(&mut c, &b, vec![]);
    assert!(t.enter(&mut c, &b).renamed.is_none());
    let names: Vec<_> = c.user.custom.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names, ["Bench A", "Bench B"]);
    // A refused name's note is A's alone.
    t.type_into(&mut c, &a, "custom name", "Omega 8500");
    assert!(t.enter(&mut c, &a).renamed.is_none());
    let note = |n: &str| {
        t.ctx
            .data(|d| d.get_temp::<String>(custom_field_id("custom name taken", "manager", n)))
    };
    assert!(note(&a).is_some());
    t.frame(&mut c, &b, vec![]);
    assert!(note(&b).is_none());
}

/// The note on a name refused goes when the Name box is typed in again,
/// and on its own when Escape is pressed (Trevor 82 on #2482).
#[test]
fn the_refused_name_note_goes_on_retyping_or_on_escape_alone() {
    let mut c = Calibres::in_memory();
    let name = c.new_custom("");
    let t = Typing::new();
    let taken = custom_field_id("custom name taken", "manager", &name);
    let noted = |t: &Typing| t.ctx.data(|d| d.get_temp::<String>(taken)).is_some();
    let refuse = |c: &mut Calibres| {
        t.type_into(c, &name, "custom name", "Omega 8500");
        assert!(t.enter(c, &name).renamed.is_none());
        t.frame(c, &name, vec![]);
        assert!(noted(&t), "refused");
    };
    refuse(&mut c);
    t.type_into(&mut c, &name, "custom name", "B");
    assert!(!noted(&t), "typing again takes the note back");
    t.frame(
        &mut c,
        &name,
        Typing::key(egui::Key::Escape, egui::Modifiers::NONE),
    );
    refuse(&mut c);
    t.frame(
        &mut c,
        &name,
        Typing::key(egui::Key::Escape, egui::Modifiers::NONE),
    );
    t.frame(&mut c, &name, vec![]);
    assert!(!noted(&t), "Escape alone takes the note back");
}

/// The texts Manage Calibres draws with `name` selected, at 1440 wide.
fn drawn_with(c: &mut Calibres, name: &str) -> Vec<String> {
    let ctx = egui::Context::default();
    theme::install(&ctx);
    c.selected = Some(name.into());
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            Vec2::new(1440.0, 2000.0),
        )),
        ..Default::default()
    };
    let mut out = None;
    for _ in 0..2 {
        out = Some(ctx.run(input.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                c.contents(ui, "", "test");
            });
        }));
    }
    out.unwrap()
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

/// A wheel half typed in Add a Wheel… stays with the calibre it was
/// started on: another calibre selected shows Add a Wheel… of its own and
/// none of it, and the first shows it again when selected (#2380).
#[test]
fn a_half_typed_wheel_stays_with_its_calibre() {
    let mut c = Calibres::in_memory();
    c.adding = Some(Adding {
        calibre: "Rolex 3235".into(),
        wheel: "Intermediate".into(),
        period: "90".into(),
    });
    let before = Catalog::new(&c.user).slots(c.find("ETA 2824-2").unwrap());
    let other = drawn_with(&mut c, "ETA 2824-2");
    assert!(other.iter().any(|t| t == "Add a Wheel…"), "{other:?}");
    assert!(!other.iter().any(|t| t == "Intermediate"), "{other:?}");
    assert_eq!(
        Catalog::new(&c.user).slots(c.find("ETA 2824-2").unwrap()),
        before
    );
    let own = drawn_with(&mut c, "Rolex 3235");
    assert!(own.iter().any(|t| t == "Intermediate"), "{own:?}");
    assert!(!own.iter().any(|t| t == "Add a Wheel…"), "{own:?}");
}

/// A custom calibre's balance its beat rate gives says so, with no Clear
/// and no Custom caption, and shows the rate's period even where none is
/// kept; one typed that the rate doesn't give is the user's (#2381).
#[test]
fn a_balance_from_the_beat_rate_says_so() {
    let mut k = custom("Bench", []);
    k.set_bph(Some(28800));
    let row_of = |k: &CustomCalibre, wheel: &str| {
        let user = UserCalibres::default();
        let e = Entry::Custom(k);
        let slots = Catalog::new(&user).slots(e);
        let s = slots.iter().find(|s| s.name == wheel).unwrap();
        let (s, tip, clear) = slot_shown(e, s, None);
        (s.period_s, tip, clear)
    };
    let row = |k: &CustomCalibre| row_of(k, "balance");
    assert_eq!(
        row(&k),
        (
            Some(0.25),
            "From the beat rate, 28800 bph".to_string(),
            None
        )
    );
    k.set_bph(Some(21600));
    let (p, tip, clear) = row(&k);
    assert_eq!((p, clear), (Some(train::balance_period_s(21600)), None));
    assert_eq!(tip, "From the beat rate, 21600 bph");
    // None kept (a file from before): still the rate's, never Not Known.
    k.wheels.clear();
    assert_eq!(row(&k).0, Some(train::balance_period_s(21600)));
    // Typed by hand: the user's, with a Clear that goes back to the rate's.
    k.wheels.push(CustomWheel {
        name: "balance".into(),
        period_s: 0.3,
    });
    assert_eq!(
        row(&k),
        (
            Some(0.3),
            "Custom, typed in".to_string(),
            Some("Go back to the beat rate's period")
        )
    );
    // Without a beat rate, Clear forgets the period typed in.
    k.bph = None;
    assert_eq!(row(&k).2, Some("Forget the period typed in"));
    // Another wheel of a calibre with a beat rate keeps its own period,
    // hover and Clear.
    let mut k = custom("Bench", [("fourth wheel", 60.0)]);
    k.set_bph(Some(28800));
    assert_eq!(
        row_of(&k, "fourth wheel"),
        (
            Some(60.0),
            "Custom, typed in".to_string(),
            Some("Forget the period typed in")
        )
    );
    // A beat rate of 0 (a file edited by hand) is none: nothing worked out.
    k.bph = Some(0);
    k.wheels.retain(|w| w.name != "balance");
    assert_eq!(
        row(&k),
        (
            None,
            "Nobody has given this wheel's period yet: type it if you know it".to_string(),
            Some("Forget the period typed in")
        )
    );
    // Drawn: the balance's row has no Clear and no Custom caption; typed,
    // it has both.
    let mut c = Calibres::in_memory();
    let mut k = custom("Bench", []);
    k.set_bph(Some(28800));
    c.user.put_custom(k);
    // The table's Maker column says "Custom" for the calibre itself.
    let customs = |seen: &[String]| seen.iter().filter(|t| *t == "Custom").count();
    let seen = drawn_with(&mut c, "Bench");
    assert!(seen.iter().any(|t| t == "0.25"), "{seen:?}");
    assert!(!seen.iter().any(|t| t == "Clear"), "{seen:?}");
    let from_rate = customs(&seen);
    c.user.set_wheel("Bench", "balance", 0.3, None);
    let seen = drawn_with(&mut c, "Bench");
    assert!(seen.iter().any(|t| t == "Clear"), "{seen:?}");
    assert_eq!(customs(&seen), from_rate + 1, "{seen:?}");
}

/// The balance periods a custom calibre keeps.
fn kept_balance(c: &Calibres) -> Vec<f64> {
    c.user.custom[0]
        .wheels
        .iter()
        .filter(|w| w.name == "balance")
        .map(|w| w.period_s)
        .collect()
}

/// A beat rate typed into the drawn Beat Rate box goes through core's
/// `CustomCalibre::set_bph`: the balance follows the rate, and a balance
/// typed that the rate doesn't give stays (#2515).
#[test]
fn the_beat_rate_box_moves_the_balance_with_it() {
    let mut c = Calibres::in_memory();
    let name = c.new_custom("Bench 28");
    let t = Typing::new();
    t.type_into(&mut c, &name, "custom bph", "28800");
    t.enter(&mut c, &name);
    assert_eq!(c.user.custom[0].bph, Some(28800));
    assert_eq!(kept_balance(&c), [0.25]);
    t.type_into(&mut c, &name, "custom bph", "21600");
    t.enter(&mut c, &name);
    assert_eq!(c.user.custom[0].bph, Some(21600));
    let p = kept_balance(&c);
    assert_eq!(p.len(), 1);
    assert!((p[0] - 1.0 / 3.0).abs() < 1e-9, "{p:?}");
    // A balance typed stays when the rate changes.
    c.user.custom[0].wheels[0].period_s = 0.3;
    t.type_into(&mut c, &name, "custom bph", "18000");
    t.enter(&mut c, &name);
    assert_eq!(c.user.custom[0].bph, Some(18000));
    assert_eq!(kept_balance(&c), [0.3]);
}

/// Return on an emptied Beat Rate or Lift Angle box clears it just as 0
/// does, and the balance from the rate goes with the rate (#2516).
#[test]
fn an_emptied_beat_rate_or_lift_box_clears_it_as_0_does() {
    for cleared in ["", "0", " "] {
        let mut c = Calibres::in_memory();
        let name = c.new_custom("Bench 28");
        let t = Typing::new();
        for (what, text) in [("custom bph", "28800"), ("custom lift", "52")] {
            t.type_into(&mut c, &name, what, text);
            t.enter(&mut c, &name);
        }
        assert_eq!(c.user.custom[0].bph, Some(28800));
        assert_eq!(c.user.custom[0].lift_angle_deg, Some(52.0));
        assert_eq!(kept_balance(&c), [0.25]);
        for what in ["custom bph", "custom lift"] {
            // All selected, then "0" or a space typed over it, or Backspace.
            t.type_into(&mut c, &name, what, cleared);
            if cleared.is_empty() {
                let back = Typing::key(egui::Key::Backspace, egui::Modifiers::NONE);
                t.frame(&mut c, &name, back);
            }
            t.enter(&mut c, &name);
        }
        assert_eq!(c.user.custom[0].bph, None, "{cleared:?}");
        assert_eq!(c.user.custom[0].lift_angle_deg, None, "{cleared:?}");
        assert!(kept_balance(&c).is_empty(), "{cleared:?}");
    }
}
