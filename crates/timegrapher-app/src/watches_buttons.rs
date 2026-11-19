//! The Takes title row's Open and "…", acting on the take picked, and
//! why Open is greyed out. Split from `watches.rs` by concern.

use super::*;

/// Why Open has nothing to act on in a watch, or Unsorted, with no takes.
pub const NO_TAKES: &str = "There are no takes here yet";

/// Why the Takes row's Open is greyed out, the real reason first:
/// nothing to pick, then none picked, and only then the Bench `busy`
/// (#2007).
pub(super) fn open_why<'a>(e: &WatchEntry, picked: bool, busy: Option<&'a str>) -> Option<&'a str> {
    let none = if e.takes.is_empty() {
        NO_TAKES
    } else {
        "Pick a take first"
    };
    if picked {
        busy
    } else {
        Some(none)
    }
}

/// The Takes title row's Open and "…", acting on the picked take: Open
/// greyed out while `busy` says why (a take running on the Bench) or none
/// is picked.
pub fn takes_buttons(
    ui: &mut egui::Ui,
    e: &WatchEntry,
    cx: &TakesContext,
    busy: Option<&str>,
) -> Option<TakeAction> {
    let mut act = None;
    let picked = picked_take(e, cx);
    let r = ui.add_enabled(picked.is_some(), theme::secondary(ui, "…"));
    let r = r
        .on_hover_text("More for the picked take: Show in the folder, Keep Analysis Only…, move it, or delete it")
        .on_disabled_hover_text(open_why(e, false, None).unwrap_or_default());
    if let Some(t) = picked {
        egui::Popup::menu(&r)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                if let Some(a) = take_menu(ui, e, t, cx, busy) {
                    act = Some(a);
                }
            });
    }
    let why = open_why(e, picked.is_some(), busy);
    if ui
        .add_enabled(why.is_none(), theme::secondary(ui, "Open"))
        .on_hover_text(
            "Analyse the whole picked take on the Bench; a long take takes a while to read",
        )
        .on_disabled_hover_text(why.unwrap_or_default())
        .clicked()
    {
        if let Some(t) = picked {
            act = Some(TakeAction::Open(e.take_dir(t)));
        }
    }
    act
}
