//! The Watches view's own state: which view fills the window, the watch
//! picked, the list's search and order, the details form, the take
//! questions, and the readings and thumbnails it shows (#1988).

use super::*;

/// The Watches view's state. One for the whole app, beside the benches.
pub(super) struct WatchesView {
    /// Which view fills the window. The app always opens on the Bench.
    pub(super) view: View,
    /// Find in Watches… asked for the search field's focus.
    pub(super) watch_search_focus: bool,
    /// The watch the New Watch form adds goes on the microphone, as the
    /// Watch line picks it: New Watch… there opened the form.
    pub(super) new_watch_on_mic: bool,
    /// The Watches view: the watch picked (its id), the list's search,
    /// order, and the details form.
    pub(super) watch_open: Option<String>,
    /// The pane shows Unsorted's takes instead of the list.
    pub(super) unsorted_open: bool,
    /// The list shown over the watch picked, or Unsorted, by ‹ All
    /// Watches: kept with what was picked then, so picking anything else
    /// shows that instead. Its row is marked in the list.
    pub(super) list_for: Option<Picked>,
    /// Unsorted Takes was picked with readings on show: the Quick Listen
    /// settings wait for the next Listen, so the readings keep theirs.
    pub(super) quick_defaults: bool,
    pub(super) watch_search: String,
    pub(super) watch_sort: watches::Sort,
    /// The added field the list shows under each watch's name, by its
    /// label; none, the calibre.
    pub(super) watch_list_shows: Option<String>,
    /// The Watches view was wide enough last frame for the list and the
    /// watch side by side.
    pub(super) watches_wide: bool,
    pub(super) watch_form: Option<watches::Form>,
    /// A question about a take (Delete, Keep Analysis Only, Add to
    /// a Watch), and the take being kept as its analysis in the background.
    pub(super) take_ask: Option<watches::Ask>,
    /// The takes' own readings the Watches list shows, worked out on a
    /// thread of their own.
    pub(super) take_readings: watches::TakeReadings,
    /// The watches' photos as thumbnails, read in the background.
    pub(super) thumbs: photos::Thumbs,
}

impl Default for WatchesView {
    fn default() -> Self {
        Self {
            view: View::Bench,
            watch_search_focus: false,
            new_watch_on_mic: false,
            watch_open: None,
            unsorted_open: false,
            list_for: None,
            quick_defaults: false,
            watch_search: String::new(),
            watch_sort: watches::Sort::default(),
            watch_list_shows: None,
            watches_wide: true,
            watch_form: None,
            take_ask: None,
            take_readings: watches::TakeReadings::default(),
            thumbs: Default::default(),
        }
    }
}

/// What the Watches view has picked: a watch (its id), Unsorted Takes,
/// or neither, as [`WatchesView::watch_open`] and
/// [`WatchesView::unsorted_open`] hold it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Picked {
    pub(super) watch: Option<String>,
    pub(super) unsorted: bool,
}

impl Picked {
    /// Unsorted Takes, with no watch.
    pub(super) const UNSORTED: Picked = Picked {
        watch: None,
        unsorted: true,
    };
}

impl WatchesView {
    /// What is picked now.
    pub(super) fn picked(&self) -> Picked {
        Picked {
            watch: self.watch_open.clone(),
            unsorted: self.unsorted_open,
        }
    }

    /// Whether `p` is what is picked now.
    pub(super) fn is_picked(&self, p: &Picked) -> bool {
        p.watch == self.watch_open && p.unsorted == self.unsorted_open
    }

    /// Pick `p` again.
    pub(super) fn pick_again(&mut self, p: Picked) {
        self.watch_open = p.watch;
        self.unsorted_open = p.unsorted;
    }
}
