//! The kinds of take New Take starts, and the buttons that start them.

use super::{guided, guided_test::BenchBefore, TestKeep, TimegrapherApp};

/// The kinds of take New Take starts for the watch picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NewTake {
    Listen,
    Positions,
    Poise,
    Complication,
    PowerReserve,
}

impl NewTake {
    /// A test's name; Listen's is its button's.
    pub(super) fn name(self) -> &'static str {
        match self {
            NewTake::Listen => "Listen",
            NewTake::Positions => guided::Kind::Positions.name(),
            NewTake::Poise => guided::Kind::Poise.name(),
            NewTake::Complication => guided::Kind::Complication.name(),
            NewTake::PowerReserve => "Power Reserve Test",
        }
    }

    /// The guided test this is, if it is one.
    pub(super) fn guided(self) -> Option<guided::Kind> {
        match self {
            NewTake::Positions => Some(guided::Kind::Positions),
            NewTake::Poise => Some(guided::Kind::Poise),
            NewTake::Complication => Some(guided::Kind::Complication),
            _ => None,
        }
    }

    /// The take a guided test of `kind` is.
    pub(super) fn of(kind: guided::Kind) -> Self {
        match kind {
            guided::Kind::Positions => NewTake::Positions,
            guided::Kind::Poise => NewTake::Poise,
            guided::Kind::Complication => NewTake::Complication,
        }
    }
}

pub(super) const NEW_TAKES: [(NewTake, &str, &str, &str); 5] = [
    (
        NewTake::Listen,
        "Listen",
        "Listen to the watch in the position on the {card} card, for as long as you like; \
         Stop saves the take into the watch",
        "As long as you like",
    ),
    (
        NewTake::Positions,
        "Positions Test",
        "The six positions one after another, settled and measured in turn, saved into the \
         watch",
        "Six positions, guided",
    ),
    (
        NewTake::Poise,
        "Dynamic Poise Test",
        "The eight upright positions, 45° apart, settled and measured in turn: the rate's \
         once-round wave and the balance's heavy spot, saved into the watch",
        "Eight upright, 45° apart",
    ),
    (
        NewTake::PowerReserve,
        "Power Reserve Test",
        "Wind the watch fully and listen in one position until it stops, saved into the watch",
        "From full wind to a stop",
    ),
    (
        NewTake::Complication,
        "Complication Test",
        "One position with a complication off, on, then off again, settled and measured in \
         turn: how much it lowers the amplitude, saved into the watch",
        "Off, on, then off again",
    ),
];

/// New Take's buttons as the watch page draws them (name, hover,
/// caption), in [`NEW_TAKES`]' order.
#[cfg(test)]
pub(crate) fn new_take_names() -> Vec<(&'static str, &'static str, &'static str)> {
    NEW_TAKES.iter().map(|t| (t.1, t.2, t.3)).collect()
}

impl TimegrapherApp {
    /// New Test is setting a test up: the Current Position card as it is
    /// noted, so the Bench can be left as it was. Noted once: a second
    /// test set up over the first keeps the card from before either.
    pub(super) fn note_bench_before(&mut self) {
        if self.ready_test().is_none() {
            self.bench.test.bench_before = Some(BenchBefore {
                position: self.bench.take.position,
                ten: self.bench.test.ten_positions,
            });
        }
    }

    /// The test set up is let go: the card goes back to where it was
    /// before New Test, unless the test's take is `kept`.
    pub(super) fn bench_back(&mut self, kept: bool) {
        let Some(b) = self.bench.test.bench_before.take() else {
            return;
        };
        if !kept {
            // Set straight, not moved: the take, if any, is going.
            self.bench.take.position = b.position;
            self.bench.test.ten_positions = b.ten;
        }
    }

    /// The test whose take Keep This … Test? asks about, while it asks.
    fn test_to_keep(&self) -> Option<NewTake> {
        match self.bench.test.test_keep? {
            TestKeep::Positions => Some(
                self.bench
                    .test
                    .guided
                    .as_ref()
                    .map_or(NewTake::Positions, |t| NewTake::of(t.kind)),
            ),
            TestKeep::PowerReserve => Some(NewTake::PowerReserve),
        }
    }

    /// What the strip shows, for the status line: the take's name, its
    /// watch and its state.
    pub(super) fn status_words(&self) -> String {
        // A take not yet saved or opened, with no watch, is Quick Listen's,
        // as the Calibre card's title says (No Watch (Quick Listen)).
        let file = self.take_name();
        let named = self.named_watch().map_or_else(String::new, |w| w.0);
        let quick = self.bench.recording.quick_listen || file.is_none() && named.is_empty();
        let name =
            file.unwrap_or_else(|| if quick { "Quick Listen" } else { "New Take" }.to_string());
        let watch = if !named.is_empty() {
            named.clone()
        } else if quick {
            "No watch".to_string()
        } else {
            "No watch picked".to_string()
        };
        // A test set up or under way is named, with how it stands.
        let test = if let Some(kind) = self.ready_test() {
            Some((kind, self.state_words().to_string()))
        } else if self.reserve_running() {
            Some((NewTake::PowerReserve, self.state_words().to_string()))
        } else if let Some(kind) = self.test_to_keep() {
            // Ended, and Keep This … Test? asked: still that test's take
            // (#2003), never Quick Listen's, which is never asked about.
            Some((kind, self.state_words().to_string()))
        } else if let Some(t) = self
            .bench
            .test
            .guided
            .as_ref()
            .filter(|_| self.test_listening())
        {
            let of = t.order().len();
            let step = (t.step + 1).min(of);
            Some((
                NewTake::of(t.kind),
                format!("{}, {step} of {of}", t.step_code(t.step.min(of - 1))),
            ))
        } else {
            None
        };
        if let Some((kind, state)) = test {
            let name = kind.name();
            let watch = if named.is_empty() {
                "No watch picked".to_string()
            } else {
                named
            };
            return format!("{name} · {watch} · {state}");
        }
        format!("{name} · {watch} · {}", self.state_words())
    }
}
