//! The readouts and the status bar: the figures shown, the clocks behind them, the status line and notices, and what shows when nothing is open.

use super::*;
use crate::sparkline;
use timegrapher_core::figure_text::{figure, figure_with_pm as with_pm, true_minus};
use timegrapher_core::library::{base_digits as base, fixed_text, pm_text, signed_text};
use timegrapher_core::measure::{self, Scope};
use timegrapher_core::steady_views;

/// What the reading cards draw from.
struct CardCtx<'a> {
    r: Option<&'a LiveReading>,
    caveats: &'a [Caveat],
    lift_now: f64,
    beats: usize,
    pm: Option<f64>,
    be_pm: Option<f64>,
    whole: &'a Option<WholeTake>,
    whole_breaks: String,
    heard: bool,
    /// What every reading card says in its figure's place while the
    /// reading waits or hears no watch, and its hover (#2007).
    unheard: Option<(String, String)>,
    compact: bool,
    /// The cards' figures each second over the last minute, for their
    /// sparklines.
    spark: Vec<TrendPoint>,
    /// "at hh:mm:ss, set by the Paper Strip", for the figures' hovers,
    /// when the strip is moved back: the strip's own label says it.
    moment: Option<String>,
    /// The reading cards go without their "?" ([`cards_short`]).
    short: bool,
}

impl CardCtx<'_> {
    /// What a card says in its figure's place ([`card_wait`]).
    fn wait(&self, figure: bool, said: bool) -> Option<(String, String)> {
        card_wait(self.unheard.as_ref(), self.r, figure, said)
    }
}

/// What a reading card says in its figure's place: the cards' shared
/// wait, `unheard`, or, with a reading `r` of a stretch still short of
/// beats but not yet this card's `figure`, that it waits for more of
/// them (#2007). Not where a limit on the card already says why it is
/// empty (`said`), as faint sides do on the Amplitude card, and not on a
/// stretch long enough, where more beats wouldn't bring the figure.
pub(super) fn card_wait(
    unheard: Option<&(String, String)>,
    r: Option<&LiveReading>,
    figure: bool,
    said: bool,
) -> Option<(String, String)> {
    unheard.cloned().or_else(|| {
        let short = r.is_some_and(|r| {
            r.caveats
                .iter()
                .any(|c| matches!(c, Caveat::ShortSpan { .. }))
        });
        (short && !figure && !said).then(|| wait_words(measure::Wait::MoreBeats))
    })
}

/// What the Beat Error card says in its figure's place, for reading `r`:
/// the cards' shared wait, or the card's own ([`card_wait`]); and where
/// `r` has figures but no beat error from either edge, its sides not yet
/// told apart, core's "Waiting for Tick and Tock", never a bare dash
/// (Norm and Hugh 5 on #2133).
pub(super) fn beat_error_card_wait(
    unheard: Option<&(String, String)>,
    r: Option<&LiveReading>,
) -> Option<(String, String)> {
    let be = r.and_then(beat_error_figure);
    let sides = r
        .and_then(|r| {
            measure::beat_error_wait(r.rate_s_per_day, r.beat_error_ms, r.beat_error_unlock_ms)
        })
        .map(wait_words);
    card_wait(unheard, r, be.is_some(), false).or(sides.filter(|_| be.is_none()))
}

/// Whether a reading's stretch holds no beat at all: no beat rate found,
/// or none of its beats in the stretch (an opened recording of silence).
pub(super) fn no_beat(r: &LiveReading) -> bool {
    r.bph.is_none() || r.beats_used == 0
}

/// A wait's short words and its hover sentence, as core gives them.
fn wait_words(w: measure::Wait) -> (String, String) {
    (w.title(), w.text())
}

/// The Listen card where the reading cards keep their full width beside
/// it: wide enough for its Wind box's second form, "Wind: full 2.0 h
/// ago" or "Wind: 2.0 h at start", or its third, "Wind 2.0 h" (Hugh on
/// #1915). The third measures 219 pt with "10 s" beside it at three figures of hours, and
/// about 226 at four; with the card's padding, 260 holds it to 999.9 h.
pub(super) const LISTEN_WIDE_W: f32 = 350.0;
pub(super) const LISTEN_HOURS_W: f32 = 260.0;

/// The cards' widths at `total`: Current Position, Listen, Calibre and
/// each reading card.
fn card_widths(total: f32, gap: f32) -> [f32; 4] {
    // The first layout that fits, from every card full down: Current
    // Position, Listen and Calibre keep their full width before the
    // reading cards keep theirs (a reading card needs little room for
    // its figure alone), then Listen, Position and Calibre go compact in
    // turn, the reading cards keeping room for their captions whole.
    let layouts = [
        // Listen wider while the reading cards keep their full width, for
        // its Wind box in words (Hugh on #1915): "Wind: full 2.0 h ago",
        // then "Wind 2.0 h".
        (POSITION_W, LISTEN_WIDE_W, CALIBRE_W, READING_FULL_W),
        (POSITION_W, LISTEN_HOURS_W, CALIBRE_W, READING_FULL_W),
        (POSITION_W, LISTEN_W, CALIBRE_W, READING_FULL_W),
        (POSITION_W, LISTEN_W, CALIBRE_W, READING_ROOMY_W),
        (POSITION_W, LISTEN_COMPACT_W, CALIBRE_W, READING_ROOMY_W),
        (
            POSITION_COMPACT_W,
            LISTEN_COMPACT_W,
            CALIBRE_W,
            READING_ROOMY_W,
        ),
        (
            POSITION_COMPACT_W,
            LISTEN_COMPACT_W,
            CALIBRE_ROOMY_W,
            CAPTION_FIT_W,
        ),
        (
            POSITION_COMPACT_W,
            LISTEN_NARROWEST_W,
            CALIBRE_ROOMY_W,
            CAPTION_FIT_W,
        ),
    ];
    let rest = |(p, l, c, _): (f32, f32, f32, f32)| (total - p - l - c - 5.0 * gap) / 3.0;
    let (position_w, listen_w, calibre_w, _) = layouts
        .into_iter()
        .find(|&layout| rest(layout) >= layout.3)
        .unwrap_or((
            POSITION_COMPACT_W,
            LISTEN_NARROWEST_W,
            CALIBRE_COMPACT_W,
            CAPTION_FIT_W,
        ));
    let reading_w = rest((position_w, listen_w, calibre_w, 0.0)).max(60.0);
    [position_w, listen_w, calibre_w, reading_w]
}

/// Each card's rect in `block` at its width in `widths`, laid left to
/// right in `CARD_ORDER`: the open cards' and the strip's alike, so the
/// strip's figures sit where the cards' do.
pub(super) fn card_rects(block: egui::Rect, widths: [f32; 6], gap: f32) -> [egui::Rect; 6] {
    let mut x = block.left();
    let mut rects = [block; 6];
    for i in CARD_ORDER {
        rects[i] = egui::Rect::from_min_size(
            egui::pos2(x, block.top()),
            Vec2::new(widths[i], block.height()),
        );
        x += widths[i] + gap;
    }
    rects
}

/// The six cards' widths at `total`, in the order of `card_rects`:
/// Position, Listen, Rate, Amplitude, Beat Error and Calibre.
pub(super) fn six_widths(total: f32, gap: f32) -> [f32; 6] {
    let [position_w, listen_w, calibre_w, reading_w] = card_widths(total, gap);
    [
        position_w, listen_w, reading_w, reading_w, reading_w, calibre_w,
    ]
}

/// The cards' backgrounds at their widths, left to right in `CARD_ORDER`,
/// and each card's three rows, in the order of `widths`.
fn lay_cards(
    ui: &mut egui::Ui,
    (total, h): (f32, f32),
    widths: [f32; 6],
    gap: f32,
    pal: &theme::Palette,
) -> Vec<CardRows> {
    let (block, _) = ui.allocate_exact_size(Vec2::new(total, h), egui::Sense::hover());
    // Each card's three rows, hung off the same three lines; left to
    // right they read Position, Calibre, Listen, Rate, Amplitude, Beat
    // Error.
    card_rects(block, widths, gap)
        .into_iter()
        .map(|rect| {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(theme::CARD_RADIUS), pal.card);
            CardRows::of(rect)
        })
        .collect()
}

/// The reading the cards show: none when the beats scatter like noise,
/// which is no watch.
pub(super) fn heard(r: Option<&LiveReading>) -> bool {
    r.is_none_or(|r| r.watch_heard || r.bph.is_none())
}

/// The Rate card's figure, signed, to its ±'s decimals.
pub(super) fn rate_figure(r: &LiveReading) -> Option<String> {
    let v = r.rate_s_per_day?;
    Some(figure(v, r.rate_error_s_per_day, base::RATE, true))
}

/// What the Rate figure's ± rests on, or what it waits for, for the
/// figure's hover (#1470): its exact size before it is rounded up, or how
/// many of the 10 s rates it needs the take holds so far.
pub(super) fn rate_pm_words(r: &LiveReading) -> Option<String> {
    use timegrapher_core::confidence::{NoPm, MIN_NOISE_VALUES as NEED, MIN_VALUES};
    use timegrapher_core::measure::{RATE_SE_MIN_S, RATE_WINDOW_S};
    r.rate_s_per_day?;
    Some(match r.rate_error_s_per_day {
        Some(se) => format!(
            "± {} s/d is one standard error ({se:.3}, rounded up). Its scatter comes \
             from {}.",
            pm_text(se, base::RATE)?,
            match r.rate_own_values {
                n if n >= NEED => format!("this run's own {n} rates of 10 s each"),
                n => format!(
                    "{NEED} rates of 10 s each: {n} of this run's own and the {} nearest it \
                     in this take",
                    NEED - n
                ),
            }
        ),
        // Readings that repeat exactly first: more won't give a ± (#1908).
        None if r.rate_no_pm == Some(NoPm::Repeats) => NoPm::REPEATS_TEXT.to_string(),
        None if r.rate_noise_values < NEED => format!(
            "± after {} of beats: it needs {NEED} rates of 10 s each, and has {}.",
            fields::duration(RATE_SE_MIN_S),
            r.rate_noise_values
        ),
        None => format!(
            "± after {} of unbroken beats.",
            fields::duration(MIN_VALUES as f64 * RATE_WINDOW_S)
        ),
    })
}

/// What the Amplitude figure's ± rests on, or why there is none, for the
/// figure's hover (#1470), as [`rate_pm_words`] says the rate's.
pub(super) fn amplitude_pm_words(r: &LiveReading, average_s: f64) -> Option<String> {
    use timegrapher_core::confidence::{NoPm, MIN_NOISE_VALUES as NEED, MIN_VALUES};
    r.amplitude_deg?;
    let stretch = floored_stretch(average_s).unwrap_or_else(|| "this average".into());
    Some(match r.amplitude_error_deg {
        Some(se) => format!(
            "± {}° is one standard error ({se:.2}, rounded up). It comes from at \
             least {NEED} readings of 2 s each: those in {stretch}, or if it has \
             fewer, the {NEED} nearest in this take.",
            pm_text(se, base::AMPLITUDE)?
        ),
        None if r.amplitude_no_pm == Some(NoPm::Repeats) => NoPm::REPEATS_TEXT.to_string(),
        None if r.amplitude_noise_values < NEED => format!(
            "± after {} of beats: it needs {NEED} readings of 2 s each, and has {}.",
            fields::duration(NEED as f64 * timegrapher_core::distribution::STRETCH_S),
            r.amplitude_noise_values
        ),
        None => format!(
            "No ± now: {} of the 2 s readings in {stretch} found the amplitude, \
             and a ± needs {MIN_VALUES}.",
            r.amplitude_values
        ),
    })
}

/// The Amplitude card's figure, with its degree sign.
pub(super) fn amplitude_figure(r: &LiveReading) -> Option<String> {
    let v = r.amplitude_deg?;
    let f = figure(v, r.amplitude_error_deg, base::AMPLITUDE, false);
    Some(format!("{f}°"))
}

/// The Amplitude card's hover on Tick's and Tock's own amplitudes, at the
/// amplitude's place, as the figure beside them and the CLI (#1470).
pub(super) fn sides_text(tick: f64, tock: f64, se: Option<f64>) -> String {
    let deg = |x| figure(x, se, base::AMPLITUDE, false);
    format!(
        "Tick {}°, Tock {}°: each side's own amplitude.",
        deg(tick),
        deg(tock)
    )
}

/// The Beat Error card's figure: its size, timed from the unlock where
/// it can be, to the unlock's ±'s decimals.
pub(super) fn beat_error_figure(r: &LiveReading) -> Option<String> {
    let (v, pm) = beat_error_value(r)?;
    Some(figure(v, pm, base::BEAT_ERROR, false))
}

/// The Beat Error card's figure, its size, and its ±: the unlock's, and
/// only when the figure is from the unlock. The one choice the figure and
/// its BE Angle both read.
fn beat_error_value(r: &LiveReading) -> Option<(f64, Option<f64>)> {
    let v = core_session::beat_error_size(r.beat_error_unlock_ms, r.beat_error_ms)?;
    let pm = r
        .beat_error_unlock_error_ms
        .filter(|_| r.beat_error_unlock_ms.is_some());
    Some((v, pm))
}

/// The Beat Error card's words on its BE Angle: the angle core works from
/// the card's figure and ± and the amplitude's, at `lift_deg`.
pub(super) fn be_angle_words(r: &LiveReading, lift_deg: f64) -> Option<String> {
    let (be, be_pm) = beat_error_value(r)?;
    let amp = r.amplitude_deg?;
    let angle =
        timegrapher_core::be_angle::Angle::of((amp, r.amplitude_error_deg), (be, be_pm), r.bph?)?;
    Some(timegrapher_core::be_angle::card_words(
        &angle, amp, lift_deg,
    ))
}

/// A row of a card, laid out left to right and kept inside it.
fn card_row(ui: &mut egui::Ui, rect: egui::Rect, salt: &str) -> egui::Ui {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("card row", salt))
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(rect.expand(2.0).intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing.x = 8.0;
    child.spacing_mut().interact_size.y = theme::CONTROL_H;
    child
}

/// The title row: the caption, its limits in a chip, and the "?" in
/// the corner. With `bare`, where a reading card has no room for its
/// whole caption beside a "?" ([`cards_short`]), there is no "?": the
/// caption carries its help, on a hover and kept open by a click, so the
/// caption stays whole (Hugh on #1954). The caption is cut short before
/// the chip is.
pub(crate) fn card_title(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    caption: &str,
    help: &[&str],
    limits: &[(String, String)],
    (note, bare): (Option<&(String, String)>, bool),
) {
    let pal = theme::pal(ui);
    let mut child = card_row(ui, rect, caption);
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // Tight, so a reading card's caption stays whole at 800.
        ui.spacing_mut().item_spacing.x = title_gap();
        if !bare {
            theme::help(ui, caption, help);
        }
        let caption_w = caption_width(ui, caption);
        let small_w = |ui: &egui::Ui, t: &str| {
            ui.fonts_mut(|f| {
                f.layout_no_wrap(t.into(), egui::FontId::proportional(12.0), pal.text)
                    .size()
                    .x
            })
        };
        // A short fact in plain small text before the "?", where the
        // caption stays whole beside it.
        if let Some((text, hover)) = note {
            if small_w(ui, text) + 4.0 + caption_w <= ui.available_width() {
                ui.label(RichText::new(text).size(12.0).color(pal.text_secondary))
                    .on_hover_text(hover);
            }
        }
        // Limits always show in a chip, shortened to fit beside the whole
        // caption, never dropped (Hugh, #1808); on a bare card, at the
        // trend row's end instead ([`trend_chip`]), the caption whole.
        if !limits.is_empty() && !bare {
            let room = ui.available_width();
            let words =
                limits_chip_words(limits, |w| theme::chip_w(ui, w) + 4.0 + caption_w <= room);
            let sentences: Vec<String> = limits.iter().map(|l| l.1.clone()).collect();
            theme::chip(ui, &words, caption, &sentences, ui.available_width());
        }
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            // Too narrow for the whole caption even with no "?" (narrower
            // than the window can be made), its short form as the axes
            // have it rather than a cut "Ampli…" that names nothing
            // (#1061).
            let short = steady_views::short_name(caption)
                .filter(|_| bare && caption_w > ui.available_width());
            let label = egui::Label::new(
                RichText::new(short.unwrap_or(caption))
                    .font(caption_font())
                    .color(pal.text_secondary),
            )
            .truncate();
            if bare {
                let r = ui.add(label.sense(egui::Sense::click()));
                theme::help_on(ui, r, (caption, help, &[]), caption);
            } else {
                ui.add(label);
            }
        });
    });
}

/// On a bare reading card (the narrowest step), its limits chip at the
/// right end of its trend row `spark`, under the figure, where the title
/// row has no room for it beside the whole caption: the same chip, hover
/// and click, in the same place on all three cards (Hugh on #2026). The
/// room left for the trend, the whole row when there is no chip.
pub(crate) fn trend_chip(
    ui: &mut egui::Ui,
    spark: egui::Rect,
    caption: &str,
    limits: &[(String, String)],
    bare: bool,
) -> egui::Rect {
    if !bare || limits.is_empty() {
        return spark;
    }
    let words = limits_chip_words(limits, |w| theme::chip_w(ui, w) <= spark.width() / 2.0);
    let w = if words.is_empty() {
        theme::CHIP_ICON + 14.0
    } else {
        theme::chip_w(ui, &words)
    };
    let at = egui::Rect::from_min_max(egui::pos2(spark.right() - w, spark.top()), spark.max);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(at)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let sentences: Vec<String> = limits.iter().map(|l| l.1.clone()).collect();
    theme::chip(&mut child, &words, caption, &sentences, w);
    egui::Rect::from_min_max(
        spark.min,
        egui::pos2(spark.right() - w - title_gap(), spark.bottom()),
    )
}

/// What a card's limits chip says, the first form that `fits` (Hugh,
/// #1808): the first limit's words and how many more, then how many
/// limits, then nothing, a bare warning chip. Every card's the same way.
pub(crate) fn limits_chip_words(
    limits: &[(String, String)],
    fits: impl Fn(&str) -> bool,
) -> String {
    let n = limits.len();
    let first = limits.first().map(|l| chip_words(&l.0)).unwrap_or_default();
    let named = match n {
        0 | 1 => first,
        _ => format!("{first} +{}", n - 1),
    };
    let count = match n {
        1 => "1 limit".to_string(),
        _ => format!("{n} limits"),
    };
    [named, count]
        .into_iter()
        .find(|w| fits(w))
        .unwrap_or_default()
}

/// The heading of the status bar hover on the caveats that limit more
/// than one card of the reading shown (Walter, #2015).
pub(crate) const SHOWN_CAVEATS_TITLE: &str = "About the readings shown:";

/// A card title's caption font.
fn caption_font() -> egui::FontId {
    theme::semibold(13.0)
}

/// The gap between the items of a card's title row.
fn title_gap() -> f32 {
    4.0
}

/// How wide a card title's whole caption is.
fn caption_width(ui: &egui::Ui, caption: &str) -> f32 {
    ui.fonts_mut(|f| {
        f.layout_no_wrap(caption.into(), caption_font(), Color32::WHITE)
            .size()
            .x
    })
}

/// Whether the readings cards go without their "?" ([`card_title`]'s
/// `bare`): where any of them, in its title row, has no room for its
/// whole caption beside its "?", all of them do, so the row stays alike
/// (Hugh, #1061, #1954). Worked out from the rows alone, each pass.
pub(crate) fn cards_short(ui: &egui::Ui, titles: &[(egui::Rect, &str)]) -> bool {
    titles.iter().any(|&(rect, caption)| {
        // Beside the "?" and the gap to it, as `card_title` lays them.
        let room = rect.width() - theme::help_size() - title_gap();
        steady_views::short_name(caption).is_some() && caption_width(ui, caption) > room
    })
}

/// A figure's hover, with the moment it is read at first when the strip
/// is moved back.
fn with_moment(moment: Option<&str>, hover: &str) -> String {
    match moment {
        Some(m) if hover.is_empty() => m.to_string(),
        Some(m) => format!("{m}\n\n{hover}"),
        None => hover.to_string(),
    }
}

/// Where the amplitude and the unlock beat error are read when Average
/// Over is shorter than they need: "the last 10 s", from core's floor
/// (#1951). None where they are read over the average.
fn floored_stretch(average_s: f64) -> Option<String> {
    measure::amplitude_span_s(average_s).map(|s| format!("the last {} s", fields::plain(s)))
}

/// The whole take's figure for a card's hover, after the big figure's
/// own stretch, `big`: "the stretch of Average Over", or for a figure
/// read over at least 10 s, "the last 10 s" (#1951).
fn whole_text(
    whole_breaks: &str,
    figure: String,
    spread: Option<String>,
    (span, big): (f64, &str),
) -> String {
    let spread = spread.map_or_else(String::new, |s| format!(", {s}"));
    format!(
        "\n\nWhole take {figure}{spread} over {}. The big figure is {big} \
         ending at the strip's newest edge; this is the take in this position, settling left out, read \
         as the command line reads it.{whole_breaks}",
        length_text(span)
    )
}

/// The big figure's stretch, as [`whole_text`] names it: the amplitude's
/// and the unlock's, `floored`, are [`floored_stretch`] (#1951).
fn big_stretch(average_s: f64, floored: bool) -> String {
    floored
        .then(|| floored_stretch(average_s))
        .flatten()
        .unwrap_or_else(|| "the stretch of Average Over".into())
}

impl TimegrapherApp {
    /// Under 10 s of sound since Listen started or the latest break, no
    /// figures yet: a word saying what they wait for, and its hover
    /// (#2014, Hugh). A settle's own state wins. Where the sound is over it
    /// is no longer listening (#2053): with beats, "Under 10 s Heard", the
    /// fact (#2066); with none, the cards' "No Beats Heard".
    pub(super) fn listening(&self, r: Option<&LiveReading>) -> Option<(String, String)> {
        // With no beat at all, once over, the cards' "No Beats Heard".
        let none = r.is_some_and(no_beat) && self.sound_over();
        r.and_then(|r| r.listening)
            .filter(|_| !none && heard(r) && self.settling_left().is_none())
            .map(|since| match self.sound_over() {
                // The sound over, under 10 s heard is a fact, not a wait:
                // no more will come (Hugh on #2066).
                true => wait_words(measure::Wait::ShortHeard { since }),
                false => (
                    measure::LISTENING_TITLE.to_string(),
                    measure::listening_text(since),
                ),
            })
    }

    /// Whether the cards show the end of a sound that is over: stopped, or
    /// an opened recording, at its newest moment. Earlier in it, the strip
    /// moved back, its first 10 s are still "Listening" (#2053).
    fn sound_over(&self) -> bool {
        !self.running() && self.strip_end.is_none()
    }

    /// Why the cards have no reading, or a reading with no beat at all:
    /// the watch settling after a move, a test waiting for its Start, a
    /// recording being read, nothing listened to and nothing open (as
    /// after New Session while stopped), or no beat in the stretch: a
    /// wait while listening, a fact once stopped or opened (#2007; Hugh
    /// on #2036). The same words on
    /// every reading card.
    pub(super) fn waiting(&self, r: Option<&LiveReading>) -> Option<(String, String)> {
        use measure::Wait;
        // No beat in the stretch: a wait while listening, a fact once
        // stopped or opened (Hugh on #2036).
        let quiet = if self.running() {
            Wait::NoBeats
        } else {
            Wait::NoBeatsHeard
        };
        if let Some(r) = r {
            return no_beat(r).then(|| wait_words(quiet));
        }
        // At the moment the cards show: the strip's newest edge when it
        // is moved back, as `card_reading` reads it.
        let at = match self.strip_end {
            Some(_) => self.strip_end_s(),
            None => self.latest_s(),
        };
        let Some(left_s) = self.settle_left_at(at) else {
            let w = if self.ready_test().is_some() {
                Wait::TestStart
            } else if self.batch.is_some() || self.stop_read_waits().is_some() {
                Wait::Analysing
            } else if self.live.is_none() {
                Wait::NotListening
            } else {
                quiet
            };
            return Some(wait_words(w));
        };
        Some(self.settle_words(left_s))
    }

    /// That the Rate figure `rate` is on the sound card's clock until the
    /// recording's own is read (#1721): only with a figure it is about,
    /// as a wait has none (Walter on #2036, where a cleared take's clock
    /// line followed New Session).
    pub(super) fn clock_not_yet(&self, rate: Option<f64>) -> Option<String> {
        rate.and_then(|_| self.live_usb_not_yet())
    }

    /// The settle's wait, with the Settle note after its sentence.
    fn settle_words(&self, left_s: f64) -> (String, String) {
        let (title, text) = wait_words(measure::Wait::Settling { left_s });
        (
            title,
            format!("{text}\n\n{}", settle_hint(self.session.settle_len())),
        )
    }

    /// Seconds of settling still to come at `at`: after a move, as the
    /// session keeps them, or a guided test's opening settle in its first
    /// position, which no move starts (Walter on #2036).
    pub(super) fn settle_left_at(&self, at: f64) -> Option<f64> {
        self.session
            .settling_left(at)
            .or_else(|| self.test_settle_left(at))
    }

    /// A guided test's phase as its pane shows it: a settle counted in the
    /// cards' own seconds, from the one `settling_left`, so the pane and
    /// the cards never say the same settle 1 s apart (Walter on #2051).
    pub(super) fn shown_phase(&self, p: Option<guided::Phase>) -> Option<guided::Phase> {
        p.map(|p| match (p, self.settling_left()) {
            (guided::Phase::Settling(_), Some(left)) => guided::Phase::Settling(left),
            (p, _) => p,
        })
    }

    /// A guided test's own settle still to come at `at`, while it listens.
    pub(super) fn test_settle_left(&self, at: f64) -> Option<f64> {
        self.guided
            .as_ref()
            .filter(|_| self.test_listening())
            .and_then(|t| t.settling_left(at))
    }

    /// The reading the reading cards show, whether a watch is heard in
    /// it, and what every card says in its figure's place when it has
    /// none: no regular beat, Finding the Beat, Listening, or a wait such
    /// as a settle (#2007).
    fn cards_state(
        &self,
        r: Option<LiveReading>,
    ) -> (Option<LiveReading>, bool, Option<(String, String)>) {
        // A reading whose beats scatter like noise is no watch: no figures
        // then, only a word saying so where the unit goes.
        // A regular beat just heard after noise or silence is still being
        // found: a wait, not that verdict (#2008).
        // A stretch with no beat at all is no beat heard, not an
        // irregular one (Walter on #2036).
        let none = r.as_ref().is_some_and(no_beat);
        let heard = heard(r.as_ref()) || none;
        let finding = r.as_ref().is_some_and(|r| r.finding_beat);
        let (unheard_line, unheard_hover) = if finding {
            (
                timegrapher_core::measure::FINDING_TITLE.to_string(),
                timegrapher_core::measure::finding_text(),
            )
        } else {
            (
                timegrapher_core::measure::NO_WATCH_TITLE.to_string(),
                format!(
                    "{}.\n\n{}",
                    r.as_ref().map(no_regular_beat).unwrap_or_default(),
                    *help::NO_REGULAR_BEAT
                ),
            )
        };
        let unheard = (!heard)
            .then(|| (unheard_line.clone(), unheard_hover.clone()))
            .or_else(|| self.listening(r.as_ref()))
            .or_else(|| self.waiting(r.as_ref()));
        (r, heard, unheard)
    }

    /// The reading the cards show their figures from: none while the
    /// take just stopped is read whole, as they wait for it (#2151). The
    /// strip and the panes keep [`Self::card_reading`] meanwhile.
    pub(super) fn shown_on_cards(&self) -> Option<LiveReading> {
        self.card_reading()
            .filter(|_| self.stop_read_waits().is_none())
    }

    /// What every reading card says in its figure's place at the moment
    /// they show, or None while they show figures ([`Self::cards_state`]).
    /// Panes that describe the same beats withhold with them.
    pub(super) fn cards_unheard(&self) -> Option<(String, String)> {
        self.cards_state(self.card_reading()).2
    }

    /// The reading cards across the top: Current Position, Listen, then
    /// Rate, Amplitude, Beat Error and Beat Rate. Every card is the same
    /// three rows (title, figure, controls) and one height at every width;
    /// where the full controls don't fit, the compact form of every card.
    /// Returns what the cards asked for, and where their controls go.
    pub(super) fn readouts(&self, ui: &mut egui::Ui) -> Readouts {
        let pal = theme::pal(ui);
        let r = self.shown_on_cards();
        let gap = theme::GAP;
        let total = ui.available_width();
        let widths = six_widths(total, gap);
        let position_compact = widths[0] < POSITION_W;
        let listen_compact = widths[1] < LISTEN_W;
        let compact = widths[2] < READING_FULL_W;
        let h = self.readings_card_h(ui.ctx());
        let cards = lay_cards(ui, (total, h), widths, gap, pal);
        let (r, heard, unheard) = self.cards_state(r);
        let r = r.as_ref().filter(|_| heard);
        let caveats = self.shown_caveats(r);
        let caveats = &caveats[..];
        // Each reading's ±, as an analysis of the recording gives them
        // (core `measure`).
        let pm = r.and_then(|r| r.rate_error_s_per_day);
        let be_pm = r.and_then(|r| r.beat_error_unlock_error_ms);
        let lift_now = self.lift_deg;
        let beats = r.map_or(0, |r| r.beats_used);
        // Over the whole take, once it is stopped: `session`'s reading of
        // it beside the last window's figure.
        let whole = self.whole_take_shown(ui.ctx());
        let whole_breaks = whole.as_ref().map_or_else(String::new, whole_breaks_text);
        let mut slots = vec![(Slot::Position, cards[0].inner)];
        // Listen: its caption here, its buttons and Average Over in the
        // rows under it.
        // A compact Listen card has no room beside its buttons for the
        // take's length: it goes in the title row, before the "?".
        let length = listen_compact.then(|| {
            let length = length_text(self.live.as_ref().map_or(0.0, |l| l.duration_s()));
            (
                length.clone(),
                format!(
                    "{length} · {}: the take's length, the sound heard so far while \
                     listening, or the whole take opened",
                    self.state_words()
                ),
            )
        });
        card_title(
            ui,
            cards[1].title,
            "Listen",
            help::LISTEN,
            &[],
            (length.as_ref(), false),
        );
        slots.push((Slot::Listen, cards[1].figure.union(cards[1].controls)));
        let c = CardCtx {
            r,
            caveats,
            lift_now,
            beats,
            pm,
            be_pm,
            whole: &whole,
            whole_breaks,
            heard,
            unheard,
            compact,
            spark: r.map_or_else(Vec::new, |r| self.spark_trend(ui.ctx(), r)),
            moment: self.moment_hover(),
            short: cards_short(
                ui,
                &[
                    (cards[3].title, steady::title(SeriesKind::Amplitude)),
                    (cards[4].title, steady::title(SeriesKind::BeatError)),
                ],
            ),
        };
        match self.stop_read_waits() {
            // Just stopped: the open's own wait across the reading cards
            // until the take is read as its saved take opens (Hugh on
            // #2151), with no Cancel.
            Some(s) => {
                let inner = cards[2].inner.union(cards[4].inner);
                let outer = inner.expand(f32::from(theme::CARD_PADDING));
                ui.painter().rect_filled(
                    outer,
                    egui::CornerRadius::same(theme::CARD_RADIUS),
                    pal.card,
                );
                let done = f64::from_bits(s.progress.load(std::sync::atomic::Ordering::Relaxed));
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    analysing_lines(ui, pal, &s.label, done, Some(s.duration_s), s.started);
                });
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
            None => {
                self.rate_card(ui, &cards[2], &c);
                self.amplitude_card(ui, &cards[3], &c);
                self.beat_error_card(ui, &cards[4], &c);
            }
        }
        // Calibre: what the readings are worked out for. The calibre on
        // the figure row, the beat rate and the lift angle under it; a
        // calibre picked that beats at another rate than the one heard says
        // so in a chip. Its title is the Watch line, drawn with the controls.
        let calibre_limits = self.calibre_chips();
        slots.push((Slot::WatchLine, cards[5].title));
        slots.push((Slot::Calibre, cards[5].figure));
        slots.push((Slot::RateAndLift, cards[5].controls));
        Readouts {
            slots,
            position_compact,
            listen_compact,
            calibre_limits,
        }
    }

    /// The Rate card's title and figure: the reading, its ±, what it waits on, and the beats and whole take behind it.
    fn rate_card(&self, ui: &mut egui::Ui, rows: &CardRows, c: &CardCtx<'_>) {
        let r = c.r;
        let caveats = c.caveats;
        let lift_now = c.lift_now;
        let beats = c.beats;
        let pm = c.pm;
        let whole = c.whole;
        let heard = c.heard;
        let compact = c.compact;
        let rate = r.and_then(|r| r.rate_s_per_day);
        let rate_facts = reading_limits("Rate", caveats, lift_now, beats, false);
        card_title(
            ui,
            rows.title,
            "Rate",
            &help::RATE,
            &rate_facts,
            (None, c.short),
        );
        let unit = match pm.and_then(|e| pm_text(e, base::RATE)) {
            Some(t) => format!("± {t} s/d"),
            None => "s/d".to_string(),
        };
        // The beats behind the figure, in its hover.
        let mut behind = self.rate_beats_behind(r);
        if let Some(t) = r.filter(|_| heard).and_then(rate_pm_words) {
            behind += &format!("\n\n{t}");
        }
        if let Some((w, v)) = whole.as_ref().and_then(|w| Some((w, w.m.rate_s_per_day?))) {
            let se = w.m.confidence.and_then(|c| c.rate.se);
            let spread = w.m.rate_p05.zip(w.m.rate_p95).map(|(a, b)| {
                let at = |x| signed_text(x, base::RATE_RANGE);
                true_minus(&format!("10 s rates {} to {} s/d", at(a), at(b)))
            });
            behind += &whole_text(
                &c.whole_breaks,
                true_minus(&format!("{} s/d", with_pm(v, se, base::RATE, true))),
                spread,
                (w.span, &big_stretch(self.average_s, false)),
            );
        }
        // In the unit's place: what the reading waits on, as every card
        // says it, or why it is the position before.
        let status = c.wait(rate.is_some(), false).or_else(|| {
            self.stopped_in_settle().map(|at| {
                (
                    "Before the Move".to_string(),
                    format!(
                        "Stopped while the watch settled in its new position, so \
                         these are the last readings of the position before it, \
                         before the move at {}.",
                        length_text(at)
                    ),
                )
            })
        });
        // While a clock is kept, the rate on it in the figure's hover:
        // first when the figure itself is corrected, as it says what
        // the figure is.
        if let Some(t) = rate.and_then(|rate| self.true_clock_words(rate)) {
            behind = if self.take_rate_ppm().is_some() {
                format!("{t}\n\n{}", behind.trim_start())
            } else {
                behind + &format!("\n\n{t}")
            };
        }
        // Live, before the recording's own clock is read: first, so the
        // figure isn't taken for the one the take will read (#1721).
        if let Some(n) = self.clock_not_yet(rate) {
            behind = format!("{n}\n\n{}", behind.trim_start());
        }
        let (fig, spark, place) = sparkline::lay(rows.inner, rows.figure, rows.controls);
        let spark = trend_chip(ui, spark, "Rate", &rate_facts, c.short);
        let colour = theme::pal(ui).trace_rate;
        sparkline::draw(ui, spark, place, &c.spark, &sparkline::RATE, colour);
        figure_row(
            ui,
            fig,
            r.and_then(rate_figure).as_deref(),
            &unit,
            status.as_ref(),
            &with_moment(c.moment.as_deref(), behind.trim_start()),
            (compact, Some(0)),
        );
    }

    /// The reading the cards show: the newest, or where the strip is
    /// moved off the newest beats, the one ending at its newest edge, as
    /// the sound panes show that moment.
    pub(super) fn card_reading(&self) -> Option<LiveReading> {
        match self.strip_end {
            Some(_) => self.reading_at_time(self.strip_end_s()),
            None => self.reading(),
        }
    }

    /// The figures the reading cards showed each whole second over the
    /// last [`sparkline::SPAN_S`] seconds up to `r`'s, the card's figure
    /// itself last: each the reading at that second over Average Over, as
    /// the figure is read. The seconds already read are kept in egui's
    /// memory, so listening reads one more each second; a change to what
    /// the readings rest on, or to Average Over, reads them all again.
    pub(super) fn spark_trend(&self, ctx: &egui::Context, r: &LiveReading) -> Vec<TrendPoint> {
        type Kept = (Revision, (u64, bool), Vec<(i64, TrendPoint)>);
        // Just stopped: the take as its saved take opens (#2151).
        let stopped = self.stopped_take();
        let Some(live) = stopped.map(|s| &s.live).or(self.live.as_ref()) else {
            return Vec::new();
        };
        let id = egui::Id::new("reading card sparklines");
        let key = (self.rev, (self.average_s.to_bits(), stopped.is_some()));
        let mut kept: Kept = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
        if (kept.0, kept.1) != key {
            kept = (key.0, key.1, Vec::new());
        }
        let end = r.time_s;
        let last = end.floor() as i64;
        let first = last - sparkline::SPAN_S + 1;
        kept.2.retain(|(s, _)| (first..=last).contains(s));
        for s in first.max(1)..=last {
            if !kept.2.iter().any(|(k, _)| *k == s) {
                let t = s as f64;
                kept.2
                    .push((s, TrendPoint::of(t, &live.reading_at(t, self.average_s))));
            }
        }
        kept.2.sort_by_key(|(s, _)| *s);
        let mut out: Vec<TrendPoint> = kept
            .2
            .iter()
            .filter(|(s, _)| self.session.replay_reading_allowed(*s as f64) && (*s as f64) < end)
            .map(|(_, p)| *p)
            .collect();
        ctx.data_mut(|d| d.insert_temp(id, kept));
        // The figure the card shows, as the newest point.
        out.push(TrendPoint {
            t: end,
            rate_s_per_day: r.rate_s_per_day,
            amplitude_deg: r.amplitude_deg,
            amplitude_even_deg: r.amplitude_even_deg,
            amplitude_odd_deg: r.amplitude_odd_deg,
            beat_error_unlock_ms: r.beat_error_unlock_ms,
            beat_error_ms: r.beat_error_ms,
            rate_error_s_per_day: r.rate_error_s_per_day,
            amplitude_error_deg: r.amplitude_error_deg,
            beat_error_unlock_error_ms: r.beat_error_unlock_error_ms,
            heard: r.watch_heard,
        });
        out
    }

    /// The beats behind the Rate figure for its hover: none said where the
    /// stretch holds no beat at all, as its wait says so (#2044).
    pub(super) fn rate_beats_behind(&self, r: Option<&LiveReading>) -> String {
        r.filter(|r| !no_beat(r))
            .map_or_else(String::new, |r| self.beats_behind(r))
    }

    /// The beats behind the Rate figure, and their jitter, for its hover.
    fn beats_behind(&self, r: &LiveReading) -> String {
        let short = r.span_s + 1.0 < self.average_s;
        let mut t = if short && r.whole_take {
            // The take's whole reading at its end: its beats are the whole
            // stretch's fitted pieces (Hugh 5 and Norm on #2075).
            format!(
                "{} beats in {} of {}: the beats the readings are fitted to, \
                 fewer seconds than Average Over asks for. They are {} after \
                 settling, fitted apart at each break, masked stretch or gap \
                 of more than {} with no beats, with any piece under {} left out.",
                r.beats_used,
                fields::duration(r.span_s.round()),
                fields::duration(self.average_s),
                self.whole_named(),
                fields::duration(timegrapher_core::clean::GAP_S),
                fields::duration(timegrapher_core::timing::MIN_PIECE_S)
            )
        } else if short {
            format!(
                "{} beats in {} of {}: the beats the readings are fitted to, \
                 fewer seconds than Average Over asks for. They are every beat \
                 since the sound began, or since the latest break, masked \
                 stretch or gap of more than {} with no beats.",
                r.beats_used,
                fields::duration(r.span_s.round()),
                fields::duration(self.average_s),
                fields::duration(timegrapher_core::clean::GAP_S)
            )
        } else {
            format!(
                "{} beats in {}: the beats the readings are fitted to, over \
                 Average Over.",
                r.beats_used,
                fields::duration(self.average_s)
            )
        };
        if let Some(j) = r.jitter_us {
            t += &format!(
                "\n\nJitter {j:.0} µs: how far single beats land from the \
                 rate's straight line, the spread of the dots across the strip."
            );
        }
        t
    }

    /// The Amplitude card's title and figure. Its lift angle is on the Calibre card, which says when it is the default, so that isn't said again in a chip.
    fn amplitude_card(&self, ui: &mut egui::Ui, rows: &CardRows, c: &CardCtx<'_>) {
        let r = c.r;
        let caveats = c.caveats;
        let lift_now = c.lift_now;
        let beats = c.beats;
        let whole = c.whole;
        let compact = c.compact;
        let facts = reading_limits(
            steady::title(SeriesKind::Amplitude),
            caveats,
            lift_now,
            beats,
            r.and_then(|r| r.amplitude_deg).is_none(),
        );
        card_title(
            ui,
            rows.title,
            steady::title(SeriesKind::Amplitude),
            help::AMPLITUDE,
            &facts,
            (None, c.short),
        );
        // Whole degrees, whatever the ±, its ± rounded up to a whole
        // degree (#1470).
        let amp_pm = r.and_then(|r| r.amplitude_error_deg);
        let unit = amp_pm
            .and_then(|e| pm_text(e, base::AMPLITUDE))
            .map_or_else(String::new, |t| format!("± {t}°"));
        let mut behind = String::new();
        if let Some(&LiveReading {
            amplitude_even_deg: Some(a),
            amplitude_odd_deg: Some(b),
            ..
        }) = r
        {
            behind = sides_text(a, b, amp_pm);
        }
        // Read over at least 10 s where Average Over is shorter (#1951).
        if let Some(note) = r.and_then(|_| measure::amplitude_span_note(self.average_s)) {
            behind += &format!(" {note}");
        }
        if let Some((w, v)) = whole.as_ref().and_then(|w| Some((w, w.m.amplitude_deg?))) {
            let se = w.m.confidence.and_then(|c| c.amplitude.se);
            let spread = w.m.amplitude_p05.zip(w.m.amplitude_p95).map(|(a, b)| {
                let deg = |x| fixed_text(x, base::AMPLITUDE);
                format!("2 s windows {}° to {}°", deg(a), deg(b))
            });
            let text = format!("{}°", with_pm(v, se, base::AMPLITUDE, false));
            let big = big_stretch(self.average_s, true);
            behind += &whole_text(&c.whole_breaks, text, spread, (w.span, &big));
        }
        behind += &format!(
            " Worked out with a lift angle of {}°, on the Calibre card.",
            fields::plain(lift_now)
        );
        if let Some(t) = r
            .filter(|_| c.heard)
            .and_then(|r| amplitude_pm_words(r, self.average_s))
        {
            behind += &format!("\n\n{t}");
        }
        let amplitude = r.and_then(amplitude_figure);
        // Faint sides say why it is empty, in the card's own chip.
        let faint = caveats.iter().any(|c| empties(c) == Some(Scope::Amplitude));
        let (fig, spark, place) = sparkline::lay(rows.inner, rows.figure, rows.controls);
        let caption = steady::title(SeriesKind::Amplitude);
        let spark = trend_chip(ui, spark, caption, &facts, c.short);
        let colour = theme::pal(ui).trace_amplitude;
        sparkline::draw(ui, spark, place, &c.spark, &sparkline::AMPLITUDE, colour);
        figure_row(
            ui,
            fig,
            amplitude.as_deref(),
            &unit,
            c.wait(amplitude.is_some(), faint).as_ref(),
            &with_moment(c.moment.as_deref(), behind.trim_start()),
            (compact, Some(1)),
        );
    }

    /// The Beat Error card's title and figure, the size only: which beat is the tick can flip between runs.
    fn beat_error_card(&self, ui: &mut egui::Ui, rows: &CardRows, c: &CardCtx<'_>) {
        let r = c.r;
        let caveats = c.caveats;
        let lift_now = c.lift_now;
        let beats = c.beats;
        let be_pm = c.be_pm;
        let whole = c.whole;
        let compact = c.compact;
        let unlock = r.and_then(|r| r.beat_error_unlock_ms);
        let drop = r.and_then(|r| r.beat_error_ms);
        let caption = steady::title(SeriesKind::BeatError);
        let be_facts = reading_limits(caption, caveats, lift_now, beats, false);
        card_title(
            ui,
            rows.title,
            caption,
            help::BEAT_ERROR,
            &be_facts,
            (None, c.short),
        );
        let be_pm = be_pm.filter(|_| unlock.is_some());
        let unit = match be_pm.and_then(|e| pm_text(e, base::BEAT_ERROR)) {
            Some(t) => format!("± {t} ms"),
            None => "ms".to_string(),
        };
        // Which edge it is timed from, in the figure's hover.
        let mut from = match (unlock, drop) {
            (Some(_), Some(d)) => format!(
                "From the unlock, the usual reading, as tg shows it: quote this one. \
                 From the drop, a second opinion: {} ms.",
                fixed_text(d.abs(), base::BEAT_ERROR)
            ),
            (None, Some(_)) => "From the drop: the unlock couldn't be timed.".into(),
            _ => "From the unlock, the usual reading, as tg shows it.".into(),
        };
        if let Some(note) = unlock.and_then(|_| measure::unlock_span_note(self.average_s)) {
            from += &format!(" {note}");
        }
        if let Some((w, v)) = whole.as_ref().and_then(|w| Some((w, w.m.beat_error()?))) {
            let se = w.m.confidence.and_then(|c| c.beat_error.se);
            let text = format!("{} ms", with_pm(v, se, base::BEAT_ERROR, false));
            // The unlock's stretch where it gives the figure (#1951).
            let big = big_stretch(self.average_s, unlock.is_some());
            from += &whole_text(&c.whole_breaks, text, None, (w.span, &big));
            if w.m.caveats.contains(&Caveat::BeatErrorAsSize) {
                from += &format!(
                    " The {WHOLE_AS_SIZES}, so the sizes either side are combined \
                     rather than the signed values, as the command line does."
                );
            }
        }
        // Its BE Angle first, where the amplitude and beat rate are known
        // (Hugh's spec, #1576).
        if let Some(words) = r
            .filter(|_| c.heard)
            .and_then(|r| be_angle_words(r, lift_now))
        {
            from = format!("{words}\n\n{from}");
        }
        let be = r.and_then(beat_error_figure);
        let wait = beat_error_card_wait(c.unheard.as_ref(), r);
        let (fig, spark, place) = sparkline::lay(rows.inner, rows.figure, rows.controls);
        let spark = trend_chip(ui, spark, caption, &be_facts, c.short);
        let colour = theme::pal(ui).trace_beat_error;
        sparkline::draw(ui, spark, place, &c.spark, &sparkline::BEAT_ERROR, colour);
        figure_row(
            ui,
            fig,
            be.as_deref(),
            &unit,
            wait.as_ref(),
            &with_moment(c.moment.as_deref(), &from),
            (compact, Some(2)),
        );
    }

    /// A calibre picked that beats at another rate than the one heard, as
    /// a chip on the Calibre card.
    pub(super) fn calibre_chips(&self) -> Vec<(String, String)> {
        self.bph_heard()
            .zip(self.cal.bph(&self.calibre))
            .filter(|&(b, cb)| b != cb && self.take_shows_rate_and_lift())
            .map(|(b, cb)| {
                (
                    format!("{} is {cb} bph", self.calibre),
                    format!(
                        "The {} beats at {cb} bph; this watch beats at {b}.",
                        self.calibre
                    ),
                )
            })
            .into_iter()
            .collect()
    }

    /// The clock the readings of the take on show are put on, for a log
    /// `duration_s` long ([`reading_clock`]).
    pub(super) fn reading_clock(
        &self,
        duration_s: f64,
    ) -> Option<timegrapher_core::clock::ClockFit> {
        reading_clock(
            self.take_clock.as_ref(),
            self.take_breaks.is_some(),
            self.clock.as_ref().map(|c| c.ppm),
            duration_s,
        )
    }

    /// The breaks the readings of `log` are cut at: the take's own, with
    /// every other cut its beats show (a jump, a move, a settling's end, a
    /// mask's ends) as a break of no length, as core's `take::breaks`
    /// adds the jumps.
    pub(super) fn reading_breaks(&self, log: &BeatLog) -> Vec<timegrapher_core::breaks::Break> {
        steady::reading_breaks(self.take_breaks.clone().unwrap_or_default(), log)
    }

    /// The take's own clock error the readings are corrected by, ppm fast:
    /// none where its clock log measures the clock too loosely to correct
    /// by (core's `ClockFit::corrects`).
    pub(super) fn take_rate_ppm(&self) -> Option<f64> {
        self.take_rate_clock().map(|f| f.ppm)
    }

    /// The take's own clock the readings are corrected on, where it
    /// corrects: its mapping, which may drift over the take.
    pub(super) fn take_rate_clock(&self) -> Option<&timegrapher_core::clock::ClockFit> {
        Self::rate_clock(&self.take_clock)
    }

    /// A clock where it corrects (core's `ClockFit::corrects`), the one
    /// place that rule is applied; it takes the field, not `self`, so a
    /// setter can call it while it holds the live engine.
    pub(super) fn rate_clock(
        clock: &Option<timegrapher_core::clock::ClockFit>,
    ) -> Option<&timegrapher_core::clock::ClockFit> {
        clock.as_ref().filter(|f| f.corrects())
    }

    /// The rate `rate` on the clock kept for the input, for the Rate
    /// figure's hover, while one is kept. Readings on a clock of their
    /// own (a take's, or while listening the clock it would be reopened
    /// on, #1721) have the big figure on it, so the hover says so. True
    /// Clock is only for the moment before that: listening, a clock kept,
    /// before the first poll puts the readings on it (a take opened keeps
    /// no input clock, Walter).
    pub(super) fn true_clock_words(&self, rate: f64) -> Option<String> {
        if let Some(f) = &self.take_clock {
            // What a log measured but doesn't apply, and why, in core's words.
            let note = f.withheld.map(|w| {
                let note = w.note();
                let mut c = note.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            });
            // A log that measures the clock too loosely corrects nothing.
            if !f.corrects() {
                return Some(format!(
                    "{}. The rate stays on the sound card's clock, as tg reads the recording.",
                    note.unwrap_or_default()
                ));
            }
            let ppm = timegrapher_core::clock::ppm_words(f.ppm, Some(f.ppm_sd));
            let from = self
                .take_clock_source
                .as_deref()
                .map_or(String::new(), |s| format!(", {s}"));
            // The recording's USB lines set the rate over a log kept for its
            // breaks: what the log measured follows.
            let note = note.map_or(String::new(), |n| format!(" {n}."));
            return Some(format!(
                "The rate corrected for the sound card's own clock, {ppm}{from}, \
                 as the Positions table reads it.{note} tg reads the recording on the card's \
                 clock."
            ));
        }
        let c = self.clock.as_ref()?;
        let v = true_minus(&format!("{:+.1}", c.correct_rate(rate)));
        Some(format!(
            "True Clock {v} seconds per day, with the clock kept {}: the rate corrected \
             for this input's own clock.",
            day_of(&c.measured_utc)
        ))
    }

    /// The input listening as Settings' Quartz Correction shows it being
    /// timed: how much sound it has heard, and once timed, the error,
    /// offered to keep. None while not listening.
    pub(super) fn clock_timing(&self) -> Option<settings_window::Timing> {
        let t = self.clock_track.as_ref()?;
        if !(self.running() && self.session.mic()) {
            return None;
        }
        let words = self.clock_words(self.clock_measured(), self.clock.as_ref());
        Some(settings_window::Timing {
            shown: t.shown.clone(),
            heard_s: t.tracker.audio_s(),
            needed_s: CLOCK_MEASURE_S,
            timed: self
                .clock_measured()
                .map(|_| (format!("Runs {}", words.value), words.hint)),
            offered: self.clock_offer().is_some(),
        })
    }

    /// The input's clock in words, for Settings' Quartz Correction: as
    /// timed and offered to keep, as kept, or not timed yet.
    pub(super) fn clock_words(
        &self,
        measured: Option<&timegrapher_core::clock::ClockFit>,
        kept: Option<&clockstore::DeviceClock>,
    ) -> ClockWords {
        use timegrapher_core::clock::{gap_sd, ppm_words, rate_error_s_per_day};
        if let Some(f) = measured {
            let way = if f.ppm >= 0.0 { "fast" } else { "slow" };
            let mut hint = format!(
                "Timed against the computer's clock over {} of listening, this input's \
                 clock runs {} ppm {way}: {:+.1} seconds per day on every rate. \
                 Keep it to show the corrected rate as True Clock from now on; the command \
                 line and the MCP server use it too. It is only right if the computer's \
                 clock is kept by internet time.",
                length_text(f.span_s),
                timed_clock_text(f.ppm, f.ppm_sd),
                f.rate_error_s_per_day
            );
            if let Some(c) = kept {
                // How far apart the two are, at the clock's place; the gap's
                // ± adds each measure's in squares (#1271, #1470).
                let off = (f.ppm - c.ppm).abs();
                let sd = gap_sd(Some(f.ppm_sd), c.ppm_sd);
                // A gap under the rate's place reads as one, not as "0.0",
                // which reads as none.
                let secs = figure(
                    rate_error_s_per_day(off).abs(),
                    sd.map(|s| rate_error_s_per_day(s).abs()),
                    base::RATE,
                    false,
                );
                let secs = if secs.chars().all(|c| c == '0' || c == '.') {
                    format!("less than {}", 10f64.powi(-(base::RATE as i32)))
                } else {
                    secs
                };
                hint += &format!(
                    "\n\nThe clock kept for \"{}\" runs {}, kept {}. This timing is {} ppm \
                     from it, {} seconds per day on True Clock. An input reached by a \
                     shared name, such as the system default, may be a different sound card \
                     from the one timed then. Keep replaces the kept clock with this one.",
                    c.device,
                    ppm_words(c.ppm, c.ppm_sd),
                    day_of(&c.measured_utc),
                    figure(off, sd, base::CLOCK, false),
                    secs
                );
            }
            return ClockWords {
                value: ppm_words(f.ppm, Some(f.ppm_sd)),
                hint,
            };
        }
        if let Some(c) = kept {
            return ClockWords {
                value: ppm_words(c.ppm, c.ppm_sd),
                hint: format!(
                    "This input's own clock runs {} ({:+.1} s/d on every rate), measured {} \
                     from {}. The Rate figure's hover gives the rate corrected for it, as True \
                     Clock. The clock kept is under Settings > Microphone > Quartz Correction.",
                    ppm_words(c.ppm, c.ppm_sd),
                    c.rate_error_s_per_day(),
                    c.measured_utc,
                    match c.source.as_str() {
                        "log" => "a recording's clock log",
                        "measure" => "listening against the system clock",
                        _ => "a value typed in",
                    }
                ),
            };
        }
        ClockWords {
            value: "Not Timed".into(),
            hint: format!(
                "This input's clock hasn't been timed, so the rate includes the sound card's \
                 own clock error. After {} of listening it is timed against the computer's \
                 clock and offered here to keep.",
                fields::duration(CLOCK_MEASURE_S)
            ),
        }
    }

    /// The microphone's clock as timed so far, once it has run
    /// [`CLOCK_MEASURE_S`] of audio and is known to half a ppm.
    pub(super) fn clock_measured(&self) -> Option<&timegrapher_core::clock::ClockFit> {
        self.clock_track
            .as_ref()?
            .fit
            .as_ref()
            .filter(|f| f.ppm_sd.is_finite() && f.ppm_sd < 0.5)
    }

    /// The clock timed while listening, offered to keep: when none is kept
    /// for the input, or the kept one is more than [`CLOCK_DIFFERS_PPM`]
    /// from it.
    pub(super) fn clock_offer(&self) -> Option<&timegrapher_core::clock::ClockFit> {
        self.clock_measured().filter(|f| {
            self.clock
                .as_ref()
                .is_none_or(|c| (f.ppm - c.ppm).abs() > CLOCK_DIFFERS_PPM)
        })
    }

    /// Keep the microphone's clock as timed: corrected rates from now on,
    /// for the command line and the MCP server too.
    pub(super) fn keep_clock(&mut self) {
        let (Some(fit), Some(t)) = (self.clock_measured(), self.clock_track.as_ref()) else {
            return;
        };
        let d = clockstore::DeviceClock::from_fit(
            &t.device,
            fit,
            timegrapher_core::recorder::utc(std::time::SystemTime::now()),
            "measure",
        );
        let saved = clockstore::ClockStore::load().and_then(|mut s| {
            s.set(d.clone());
            s.save()
        });
        match saved {
            Ok(p) => {
                self.info_msg(format!(
                    "Kept this input's clock, {}, in {}",
                    timegrapher_core::clock::ppm_words(d.ppm, d.ppm_sd),
                    p.display()
                ));
                self.put_input_clock(Some(d));
            }
            Err(e) => self.error(format!("The input's clock couldn't be kept: {e}")),
        }
    }

    /// The caveats on the reading shown, `r` where a watch is heard: its
    /// own, and the take's breaks not all its own list's, as session says
    /// too.
    pub(super) fn shown_caveats(&self, r: Option<&LiveReading>) -> Vec<Caveat> {
        let mut caveats = r.map_or_else(Vec::new, |r| r.caveats.clone());
        if let (Some(why), Some(_)) = (self.take_unlisted, r) {
            timegrapher_core::measure::add(&mut caveats, Caveat::BreaksUnlisted { why });
        }
        caveats
    }

    /// The caveats on the reading shown that bear on more than one card,
    /// or on the take, once, in the status bar after the take's state: a
    /// chip in the cards' step-down, each one's sentence on its hover
    /// (Hugh, #1975).
    fn shown_caveats_chip(&self, ui: &mut egui::Ui) {
        let r = self.shown_on_cards();
        let r = r.as_ref().filter(|r| heard(Some(r)));
        let amplitude_empty = r.and_then(|r| r.amplitude_deg).is_none();
        let caveats = self.shown_caveats(r);
        let beats = r.map_or(0, |r| r.beats_used);
        let limits = take_limits(&caveats, self.lift_deg, beats, amplitude_empty);
        if limits.is_empty() {
            return;
        }
        let room = ui.available_width();
        let words = limits_chip_words(&limits, |w| theme::chip_w(ui, w) <= room);
        let sentences: Vec<String> = limits.into_iter().map(|l| l.1).collect();
        theme::chip(ui, &words, SHOWN_CAVEATS_TITLE, &sentences, room);
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            // The notices on the right first, so that what is on the left is
            // cut short with an ellipsis rather than running under them.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.status_notices(ui);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    // Nothing runs under the notices, even past an ellipsis.
                    ui.shrink_clip_rect(ui.max_rect());
                    self.status_left(ui);
                });
            });
        });
    }

    /// Where the take opened has USB lines and a clock log good enough to
    /// correct on its own that measure its sound card apart, core's
    /// caveat for it, short and long, in core's words. The rates stay
    /// corrected by the USB lines.
    pub(super) fn clock_disagreement(&self) -> Option<(String, String)> {
        let caveat = Caveat::from(self.take_clock.as_ref()?.sources_disagree()?);
        Some((
            caveat.short(self.lift_deg, 0)?,
            caveat.long(Some(self.lift_deg))?,
        ))
    }

    /// The status bar's notices, right to left: a word each for what used
    /// to push the window down from the top (an error, the app having
    /// closed unexpectedly, sound kept from an earlier session, a newer
    /// release) and the take's USB lines and clock log disagreeing, each
    /// opening its whole message and buttons over the window. The bar keeps its height whatever it shows.
    fn status_notices(&mut self, ui: &mut egui::Ui) {
        let popover = |resp: &egui::Response, body: &mut dyn FnMut(&mut egui::Ui)| {
            egui::Popup::menu(resp)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_width(440.0);
                    // Size to the notice's contents on each frame. Otherwise its
                    // rows can consume the previous popup height plus their margins.
                    ui.shrink_height_to_current();
                    body(ui);
                });
        };
        if let Some(r) = self.updates.notice.clone() {
            let resp = status_item(
                ui,
                &format!("Timegrapher {} Available…", r.version),
                None,
                "A newer release is published: click for its page",
            );
            popover(&resp, &mut |ui| self.update_notice(ui));
        }
        if let Some(k) = self.kept.first() {
            // Named by what the first one offered is (Hugh on #1806).
            let tip = match k.parts {
                true => {
                    "A Power Reserve Test (Analysis Only) was never saved: click to open, move or \
                     delete it"
                }
                false => "Sound from an earlier session was never saved: click to open, move or delete it",
            };
            let resp = status_item(ui, "Unsaved Take Found…", Some(theme::Alert::Warning), tip);
            popover(&resp, &mut |ui| self.kept_notice(ui));
        }
        if self.crash.is_some() {
            let resp = status_item(
                ui,
                "Closed Unexpectedly…",
                Some(theme::Alert::Warning),
                "The timegrapher closed unexpectedly last time: click to make a report",
            );
            popover(&resp, &mut |ui| self.crash_notice(ui));
        }
        let error =
            self.session.recording_fault().is_some() || matches!(self.message, Some((_, true)));
        if error && !self.setup_sheet {
            let short = match (self.session.recording_fault(), &self.message) {
                (Some(_), _) => "Recording Stopped…".to_string(),
                (None, Some((m, _))) => error_short(m),
                _ => String::new(),
            };
            let resp = status_item(
                ui,
                &short,
                Some(theme::Alert::Bad),
                "Click for the whole message",
            );
            popover(&resp, &mut |ui| self.error_notice(ui));
        }
        // On the right, so a long take name never hides it.
        if let Some((short, long)) = self.clock_disagreement() {
            let resp = status_item(
                ui,
                &format!("{short}…"),
                Some(theme::Alert::Warning),
                "The take's USB lines and clock log measure its sound card apart: click for both figures",
            );
            popover(&resp, &mut |ui| {
                ui.label(long.as_str());
            });
        }
    }

    /// The status bar's left side: what is showing, the signal, the kept
    /// sound and the latest message.
    fn status_left(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        ui.spacing_mut().item_spacing.x = 12.0;
        if let Some(job) = &self.analysis_save {
            let done = f64::from_bits(job.progress.load(std::sync::atomic::Ordering::Relaxed));
            // The sound is read twice: once for the beats, once more for
            // the profiles and the shape.
            let frac = (done / (2.0 * job.total_s).max(1e-9)).clamp(0.0, 1.0);
            ui.add(
                egui::ProgressBar::new(frac as f32)
                    .desired_width(240.0)
                    .desired_height(14.0)
                    .fill(pal.accent)
                    .text(RichText::new("Keeping the analysis").small()),
            );
            let cancel = ui
                .small_button("Cancel")
                .on_hover_text("Stop keeping the analysis; nothing is written")
                .clicked();
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
            if cancel {
                self.analysis_save = None;
            }
        }
        if let Some(s) = &self.saving {
            ui.add(egui::Spinner::new().size(12.0).color(pal.text_secondary));
            ui.label(RichText::new("Saving…").small().color(pal.text_secondary))
                .on_hover_text(format!(
                    "Copying {} of sound into {}. Listening carries on; the take saved before \
                     stays as it was until the copy is done.",
                    length_text(s.secs),
                    s.dest.display()
                ));
        }
        // Always on screen: where the take is going, and whether the strip
        // is live or a take opened. The take's name is cut short with "…"
        // where the facts after it need the room, keeping its watch and
        // state; hovered, it shows whole.
        let facts = {
            let mut sizing = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(ui.available_rect_before_wrap())
                    .layout(egui::Layout::left_to_right(egui::Align::Center))
                    .invisible()
                    .sizing_pass(),
            );
            self.status_facts(&mut sizing);
            sizing.min_rect().width()
        };
        let room = (ui.available_width() - facts - ui.spacing().item_spacing.x).max(0.0);
        let words = self.status_words();
        let (name, rest) = words.split_once(" · ").unwrap_or((&words, ""));
        let font = theme::semibold(13.5);
        let shown = elide(ui, name, &format!(" · {rest}"), &font, room);
        let hover = "The take on show, its watch, and whether it is listening";
        ui.scope(|ui| {
            ui.set_max_width(room);
            ui.add(egui::Label::new(RichText::new(&shown).font(font).color(pal.text)).truncate())
                .on_hover_text(if shown == words {
                    hover.to_string()
                } else {
                    format!("{words}\n\n{hover}")
                });
        });
        self.status_facts(ui);
        // What can be cut short with an ellipsis last, so that the facts
        // before it keep their room. Hovered, a word cut short shows
        // whole; one with no room for a word of it isn't drawn at all.
        let passing = |ui: &mut egui::Ui, text: &str| {
            let font = egui::TextStyle::Small.resolve(ui.style());
            let shown = elide(ui, text, "", &font, ui.available_width());
            if shown.starts_with('…') {
                return;
            }
            let label = ui.label(RichText::new(&shown).small().color(pal.text_secondary));
            if shown != text {
                label.on_hover_text(text);
            }
        };
        if self.batch.is_none() && self.running() && !self.source_label.is_empty() {
            // The input listened to; a take opened is named on the left
            // already.
            passing(ui, &format!("Listening to {}", self.source_label));
        }
        // Errors are a notice at the right instead.
        if let Some((m, false)) = &self.message {
            passing(ui, m);
        }
    }

    /// The recording's state in the status bar, its colour and its hover:
    /// stopped by a fault, saved (and what since isn't), its analysis
    /// kept without the sound, or not saved.
    pub(super) fn recording_state(
        &self,
        r: &Recorder,
        pal: &theme::Palette,
    ) -> (String, Color32, String) {
        let since = self.unsaved_s();
        if let Some(fault) = self.session.recording_fault() {
            (
                "Recording stopped; recovery files retained".to_string(),
                pal.text,
                fault.description(),
            )
        } else if r.saved_copy_gone() {
            saved_copy_gone(self.saved_hours_reachable(), self.streams(), pal)
        } else if self.streams() {
            // Recording into its folder: nothing waits on a Save Again.
            (
                "Saved".to_string(),
                pal.text_secondary,
                format!("Saving to {}: {RECORDING_IN_HINT}.", self.kept_place()),
            )
        } else {
            match self.session.saved_to() {
                Some(p) => (
                    if since >= 1.0 {
                        format!("The last {} not saved", length_text(since))
                    } else if self.metadata_unsaved() {
                        "Settings not saved".to_string()
                    } else {
                        "Saved".to_string()
                    },
                    pal.text_secondary,
                    format!(
                        "Saved in {}. Sound since then is kept too; Save Again adds it. \
                     New Session and quitting ask first, and a quit that can't ask \
                     keeps it when there is more than 5 s of it, \
                     to offer back at the next start.",
                        p.display()
                    ),
                ),
                None if self.analysis_kept_to.is_some() => {
                    let (dir, through_s) = self.analysis_kept_to.clone().unwrap_or_default();
                    let kept = if r.duration_s() - through_s >= 1.0 {
                        format!("the analysis of its first {} kept", length_text(through_s))
                    } else {
                        "its analysis kept".to_string()
                    };
                    (
                        format!("Not saved; {kept}"),
                        pal.text_secondary,
                        format!(
                            "The analysis of the first {} was kept without the sound in {}. \
                             The sound itself isn't saved: Save keeps it. New Session and \
                             quitting ask before throwing it away, and a quit that can't \
                             ask keeps it when there is more than 5 s of it, \
                             to offer back at the next start.",
                            length_text(through_s),
                            dir.display()
                        ),
                    )
                }
                None => (
                    "Not saved".to_string(),
                    pal.text_secondary,
                    "The session's sound is kept as it goes, so Save can keep it at any \
                 point. New Session and quitting ask before throwing it away, and a quit that \
                 can't ask keeps it when there is more than 5 s of it, \
                 to offer back at the next start."
                        .to_string(),
                ),
            }
        }
    }

    /// The status bar's facts after the take's name, none cut short: the
    /// analysis under way, unsaved changes, the recording's state and the
    /// signal.
    pub(super) fn status_facts(&self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        self.shown_caveats_chip(ui);
        // A take opening, or one just stopped being read as its saved
        // take opens (#2151): how far, as one bar.
        let reading = match (&self.batch, self.stop_read_waits()) {
            (Some(b), _) => Some((&b.progress, b.duration_s)),
            (None, Some(s)) => Some((&s.progress, Some(s.duration_s))),
            (None, None) => None,
        };
        if let Some((progress, duration_s)) = reading {
            let done = f64::from_bits(progress.load(std::sync::atomic::Ordering::Relaxed));
            let frac = duration_s.map_or(0.0, |d| (done / d).clamp(0.0, 1.0));
            ui.add(
                egui::ProgressBar::new(frac as f32)
                    .desired_width(240.0)
                    .desired_height(14.0)
                    .fill(pal.accent)
                    .text(RichText::new(batch_words(frac, duration_s.is_some())).small()),
            );
        }
        if let Some(k) = &self.wav_keep {
            // Its count moves and its end comes with no input to wake the
            // window (Walter's walk of #1120).
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
            ui.label(RichText::new(k.words()).small().color(pal.text_secondary))
                .on_hover_text(library_folder::WAV_KEEP_HINT);
        }
        // A take opened, its painting or masks changed: Save keeps them.
        if self.session.recorder.is_none() && self.marks_unsaved() {
            ui.label(RichText::new("Changes not saved").color(pal.text_secondary))
                .on_hover_text(SAVE_MARKS_HINT);
        }
        if let Some(r) = &self.session.recorder {
            let (text, color, hint) = self.recording_state(r, pal);
            let failed = self.session.recording_fault().is_some();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if failed {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                    theme::alert_icon(ui.painter(), rect, theme::Alert::Bad, pal);
                } else {
                    theme::dot(
                        ui,
                        if self.running() {
                            pal.live
                        } else {
                            pal.text_tertiary
                        },
                    );
                }
                ui.label(RichText::new(text).small().color(color));
            })
            .response
            .on_hover_text(hint);
        }
        if self.running() {
            // Always in its place while listening, so what follows it
            // never moves on its own: "signal —" before there is one
            // (#2007).
            let s = self
                .signal()
                .map_or_else(|| "—".to_string(), |s| format!("{s:.0}×"));
            ui.label(
                RichText::new(format!("signal {s}"))
                    .small()
                    .color(pal.text_secondary),
            )
            .on_hover_text(*help::SIGNAL);
        }
    }

    /// The signal heard now: the newest reading's, and while the watch
    /// settles after a move, when nothing is read, the sound's own, as
    /// the signal doesn't wait for the settle (#2007).
    pub(super) fn signal(&self) -> Option<f32> {
        match (self.reading(), self.settling_left()) {
            (Some(r), _) => r.snr,
            (None, Some(_)) => self.live.as_ref()?.reading(self.average_s).snr,
            (None, None) => None,
        }
    }

    /// The starting arrangement for a strip direction, keeping which panes
    /// are hidden.
    pub(super) fn relayout(&mut self, horizontal: bool) {
        let shown: Vec<(Pane, bool)> = Pane::ALL
            .into_iter()
            .map(|p| (p, pane_visible(&self.panes.tiles, p)))
            .collect();
        self.panes = default_layout(horizontal);
        self.panes_dragged.clear();
        for (p, on) in shown {
            set_pane_visible(&mut self.panes.tiles, p, on);
        }
    }

    /// What shows before there is anything to show: how to begin, and the
    /// two ways to.
    /// Before anything is open: the three ways in. While a recording is
    /// being analysed, how far it has got.
    pub(super) fn empty_state(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        // Space above the card, but never so much that the card (as tall as
        // on the last frame) runs out of the pane.
        let h_id = ui.id().with("empty state height");
        let last_h: f32 = ui.data(|d| d.get_temp(h_id)).unwrap_or(0.0);
        let room = (ui.available_height() - last_h).max(0.0);
        ui.add_space((ui.available_height() * 0.16).clamp(16.0, 110.0).min(room));
        ui.vertical_centered(|ui| {
            let w = 520.0_f32.min(ui.available_width());
            ui.allocate_ui(Vec2::new(w, 0.0), |ui| {
                let shown = theme::card().fill(pal.card).show(ui, |ui| {
                    ui.set_width(w - 28.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 10.0;
                        if self.batch.is_some() {
                            self.batch_progress(ui);
                        } else {
                            self.ways_in(ui);
                        }
                    });
                });
                let h = shown.response.rect.height();
                if h != last_h {
                    ui.data_mut(|d| d.insert_temp(h_id, h));
                    ui.ctx().request_repaint();
                }
            });
        });
    }

    /// Nothing that sounds like a watch has been heard in the take, after a
    /// few seconds of listening: one sentence then, not empty charts and
    /// cards of figures that would describe noise.
    pub(super) fn no_watch_yet(&self) -> bool {
        let Some(live) = &self.live else {
            return false;
        };
        self.batch.is_none()
            && live.duration_s() > 6.0
            && !self.trend.iter().any(|p| p.heard)
            && self
                .reading()
                .is_none_or(|r| !r.watch_heard && !r.finding_beat)
            && !self.heard_while_settling(live)
    }

    /// Whether the watch is heard while it settles: nothing is read then,
    /// but its beats are found, so the strip shows them, not "no regular
    /// beat" (Peter, 2026-11-18: a 30 s settle at the start said so for
    /// all 30 s while the Scope and Tick Tock Profile showed the beats).
    fn heard_while_settling(&self, live: &LiveAnalyzer) -> bool {
        self.settling_left().is_some()
            && live
                .reading_at(live.duration_s(), self.average_s)
                .watch_heard
    }

    pub(super) fn no_watch(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        // A stretch with no beat at all: the cards' own words, so the strip
        // and the cards give one verdict (#2044). Beats that aren't
        // regular, or noise in a settle (#1944): why there is no watch.
        let reading = self.reading();
        let (line, below, hover) = match reading.as_ref().and_then(|r| self.waiting(Some(r))) {
            Some((title, text)) => (title, text, None),
            None => (
                reading.as_ref().map_or_else(
                    || timegrapher_core::beats::no_regular_beat_text(None, None),
                    no_regular_beat,
                ),
                "The sound so far has no beats at regular intervals. The readings and \
                 charts appear once a regular beat is found."
                    .to_string(),
                Some(*help::NO_REGULAR_BEAT),
            ),
        };
        ui.add_space((ui.available_height() * 0.16).clamp(16.0, 110.0));
        ui.vertical_centered(|ui| {
            let w = 520.0_f32.min(ui.available_width());
            ui.allocate_ui(Vec2::new(w, 0.0), |ui| {
                theme::card().fill(pal.card).show(ui, |ui| {
                    ui.set_width(w - 28.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        let title = ui.label(RichText::new(&line).font(theme::semibold(17.0)));
                        if let Some(h) = hover {
                            title.on_hover_text(h);
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(&below).small().color(pal.text_secondary),
                            )
                            .wrap(),
                        );
                    });
                });
            });
        });
    }

    /// Recording chosen while a stopped microphone session is in hand: the
    /// screen offers a recording instead of showing the session's figures
    /// as if they were one. The session is kept.
    pub(super) fn mic_session_set_aside(&self) -> bool {
        self.input == Input::File && self.session.mic() && !self.running()
    }

    /// Before anything is open: where to start, since the Watches view
    /// and the File menu hold the ways in.
    fn ways_in(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        ui.label(RichText::new("Timegrapher").font(theme::semibold(20.0)));
        let line = |ui: &mut egui::Ui, text: &str| {
            ui.add(egui::Label::new(RichText::new(text).color(pal.text_secondary)).wrap());
        };
        if self.mic_session_set_aside() {
            line(
                ui,
                "Your microphone session is set aside while this shows: Listen, on the \
                 Listen card, starts a new one.",
            );
        }
        line(
            ui,
            "Listen, on the Listen card, listens to the watch on the microphone without \
             picking a watch; Save keeps what it heard. For a watch's history, pick it on \
             the Calibre card's title, or in the Watches view, then Listen, \
             Positions Test, Dynamic Poise Test or Power Reserve Test, and each take is \
             saved into it. New Watch… adds one.",
        );
        line(
            ui,
            "Recordings and folders open from the File menu, analysed all at once.",
        );
    }

    /// How far the analysis of a recording has got, and a way to stop it.
    fn batch_progress(&mut self, ui: &mut egui::Ui) {
        let pal = theme::pal(ui);
        let Some(b) = &self.batch else { return };
        let done = f64::from_bits(b.progress.load(std::sync::atomic::Ordering::Relaxed));
        analysing_lines(ui, pal, &b.label, done, b.duration_s, b.started);
        if b.files > 1 {
            ui.label(
                RichText::new(format!(
                    "{} files joined in order, {} to {}.",
                    b.files, b.first, b.last
                ))
                .small()
                .color(pal.text_tertiary),
            );
        }
        if let Some(n) = &b.order_note {
            theme::alert_label(ui, theme::Alert::Warning, n, true);
        }
        if ui
            .add(theme::secondary(ui, "Cancel"))
            .on_hover_text("Stop analysing this recording")
            .clicked()
        {
            self.cancel_open();
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// How far the reading of a take has got, as an open shows it: `Analysing
/// {label}`, a bar with its %, and the sound read so far of the take's
/// `duration_s`, with the time left at the pace so far.
fn analysing_lines(
    ui: &mut egui::Ui,
    pal: &theme::Palette,
    label: &str,
    done: f64,
    duration_s: Option<f64>,
    started: Instant,
) {
    ui.label(RichText::new(format!("Analysing {label}")).font(theme::semibold(18.0)));
    let frac = duration_s.map(|d| (done / d.max(1e-9)).clamp(0.0, 1.0));
    // The bar's track in the control colour, to stand out on the card.
    ui.scope(|ui| {
        ui.visuals_mut().extreme_bg_color = pal.control;
        ui.add(
            egui::ProgressBar::new(frac.unwrap_or(0.0) as f32)
                .desired_width(ui.available_width())
                .desired_height(16.0)
                .fill(pal.accent)
                .text(RichText::new(match frac {
                    Some(f) => format!("{:.0}%", 100.0 * f),
                    None => String::new(),
                })),
        );
    });
    let elapsed = started.elapsed().as_secs_f64();
    let mut line = match duration_s {
        Some(d) => format!(
            "{} of {} of sound analysed",
            length_text(done),
            length_text(d)
        ),
        None => format!("{} of sound analysed", length_text(done)),
    };
    // Time left at the pace so far, once there is a pace to go by.
    if let (Some(d), true) = (duration_s, done > 0.0 && elapsed > 3.0) {
        let left = (d - done).max(0.0) * elapsed / done;
        line += &format!(" · about {} to go", length_text(left));
    }
    ui.label(RichText::new(line).color(pal.text_secondary));
}

/// Where a galley's first line sits: its baseline, down from its top.
fn baseline(g: &egui::Galley) -> f32 {
    g.rows
        .first()
        .and_then(|r| Some(r.pos.y + r.row.glyphs.first()?.pos.y))
        .unwrap_or(g.size().y * 0.8)
}

/// The least size, points, a card's figure is made to fit beside its ±
/// and unit; past it the ± stacks, then leaves the card (#825).
const FIGURE_LEAST_PT: f32 = 12.0;

/// The size, points, below which a card's figure is not made smaller to
/// keep its unit whole: past [`FIGURE_LEAST_PT`] only for a figure of
/// thousands on the narrowest window (#1976).
const FIGURE_FLOOR_PT: f32 = 8.0;

/// The reading cards' one figure size: card `card`'s own, `own`, kept
/// for the frame, and the least of the three cards' (Hugh on #1954 and
/// #1976: no figure smaller than its neighbours). A card drawn before the
/// others reads their last frame's, and a change draws again.
fn shared_figure_pt(ctx: &egui::Context, card: usize, own: f32) -> f32 {
    let id = egui::Id::new("reading card figure sizes");
    let mut sizes: [f32; 3] = ctx.data(|d| d.get_temp(id)).unwrap_or([f32::INFINITY; 3]);
    let was = sizes;
    if let Some(s) = sizes.get_mut(card) {
        *s = own;
    }
    if sizes != was {
        ctx.data_mut(|d| d.insert_temp(id, sizes));
        ctx.request_repaint();
    }
    sizes.into_iter().fold(own, f32::min)
}

/// The figure `text` laid out at the largest size up to `full` at which
/// it and its unit fit whole in `room`: never a unit cut to "s/c"
/// (#1976), the figure going below its least size first; on reading card
/// `card`, at the three cards' one size, the largest at which every one
/// fits (Hugh on #1954, #1976). A dash (`is_figure` false) keeps `full`.
fn fitted_figure(
    ui: &egui::Ui,
    (text, colour): (&str, Color32),
    (full, room): (f32, f32),
    is_figure: bool,
    card: Option<usize>,
) -> std::sync::Arc<egui::Galley> {
    let lay = |size: f32| {
        ui.painter()
            .layout_no_wrap(text.to_string(), theme::display(size), colour)
    };
    let mut pt = full;
    let mut big = lay(pt);
    if is_figure {
        while big.size().x > room && pt > FIGURE_FLOOR_PT {
            pt = (pt * room / big.size().x)
                .floor()
                .min(pt - 1.0)
                .max(FIGURE_FLOOR_PT);
            big = lay(pt);
        }
    }
    if let Some(i) = card {
        let shared = shared_figure_pt(ui.ctx(), i, if is_figure { pt } else { full });
        if shared < pt {
            big = lay(shared);
        }
    }
    big
}

/// A readings card's figure row: the big figure ("—" while there is none),
/// then its ± and unit on the figure's baseline, or what the reading waits
/// on in their place. On a compact card the figure is smaller and the ±
/// sits over the unit beside it. A figure too wide for the card (noise
/// read as beats can give thousands of s/d) is made smaller to show whole.
pub(super) fn figure_row(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    value: Option<&str>,
    unit: &str,
    status: Option<&(String, String)>,
    hover: &str,
    (compact, card): (bool, Option<usize>),
) {
    let pal = theme::pal(ui);
    let (text, colour) = match value {
        // A true minus sign, as wide as the plus.
        Some(v) => (true_minus(v), pal.text),
        None => ("—".to_string(), pal.text_tertiary),
    };
    let side_text = status.map_or(unit, |s| s.0.as_str());
    let mut lines = side_lines(side_text, status.is_some(), compact);
    let mut side_font = egui::FontId::proportional(if compact { 12.0 } else { 15.0 });
    let lay_side = |font: &egui::FontId, lines: &[String]| -> Vec<_> {
        lines
            .iter()
            .map(|l| {
                ui.painter()
                    .layout_no_wrap(l.clone(), font.clone(), pal.text_secondary)
            })
            .collect()
    };
    let mut side = lay_side(&side_font, &lines);
    let mut side_w = side.iter().map(|g| g.size().x).fold(0.0, f32::max);
    // Words in the figure's place ("No Regular Beat") show whole: with no
    // figure, the dash gives them its room, and they are made smaller
    // only when even that is too narrow.
    let figure_w = ui
        .painter()
        .layout_no_wrap(
            text.clone(),
            theme::display(if compact {
                theme::FIGURE_COMPACT_PT
            } else {
                theme::FIGURE_PT
            }),
            colour,
        )
        .size()
        .x;
    let no_dash = value.is_none() && status.is_some() && figure_w + 6.0 + side_w > rect.width();
    // A wait too wide at its size steps down to its shorter words first
    // (Hugh on #2036), and only then is made smaller.
    let short = status.and_then(|s| measure::wait_short_title(&s.0));
    if let Some(short) = short.filter(|_| no_dash && side_w > rect.width()) {
        lines = side_lines(&short, true, compact);
        side = lay_side(&side_font, &lines);
        side_w = side.iter().map(|g| g.size().x).fold(0.0, f32::max);
    }
    if no_dash && side_w > rect.width() {
        side_font.size = (side_font.size * rect.width() / side_w).floor().max(9.0);
        side = lay_side(&side_font, &lines);
        side_w = side.iter().map(|g| g.size().x).fold(0.0, f32::max);
    }
    // A ± too wide to sit beside the figure, even with the figure made as
    // small as it is made, stacks over its unit, and, still too wide,
    // leaves the card for the hover: never cut into a number that reads
    // as zero (#825).
    let mut moved = None;
    if status.is_none() && value.is_some() && side_text.starts_with('±') {
        let fits = |l: &[String]| {
            let w = lay_side(&side_font, l)
                .iter()
                .map(|g| g.size().x)
                .fold(0.0, f32::max);
            let least = FIGURE_LEAST_PT
                / if compact {
                    theme::FIGURE_COMPACT_PT
                } else {
                    theme::FIGURE_PT
                };
            figure_w * least + 6.0 + w <= rect.width()
        };
        let (fitted, cut) = pm_fitted(side_text, lines, fits);
        lines = fitted;
        moved = cut.then_some(side_text);
        side = lay_side(&side_font, &lines);
        side_w = side.iter().map(|g| g.size().x).fold(0.0, f32::max);
    }
    let room = rect.width() - if side_w > 0.0 { side_w + 6.0 } else { 0.0 };
    let full = if compact {
        theme::FIGURE_COMPACT_PT
    } else {
        theme::FIGURE_PT
    };
    let big = fitted_figure(ui, (&text, colour), (full, room), value.is_some(), card);
    let top = rect.center().y - big.size().y / 2.0;
    let base = top + baseline(&big);
    let painter = ui.painter().with_clip_rect(rect.expand(2.0));
    let x = if no_dash {
        rect.left()
    } else {
        rect.left() + big.size().x + 6.0
    };
    if !no_dash {
        painter.galley(egui::pos2(rect.left(), top), big, colour);
    }
    let step = side_font.size * 1.2;
    let n = side.len();
    for (i, g) in side.into_iter().enumerate() {
        let line_base = base - (n - 1 - i) as f32 * step;
        let y = line_base - baseline(&g);
        painter.galley(egui::pos2(x, y), g, pal.text_secondary);
    }
    let hover = figure_hover(status, hover, moved.map(|pm| format!("{text} {pm}")));
    if !hover.is_empty() {
        ui.interact(
            rect,
            ui.id().with(("figure", rect.left() as i32)),
            egui::Sense::hover(),
        )
        .on_hover_text(hover);
    }
}

/// The lines beside a card's figure: one line in Body, or on a compact
/// card two small lines, the ± over the unit, or a status's words split
/// as evenly as they allow.
fn side_lines(side_text: &str, status: bool, compact: bool) -> Vec<String> {
    // Beside the figure: one line in Body, or on a compact card two small
    // lines, the ± over the unit.
    if side_text.is_empty() {
        Vec::new()
    } else if !compact {
        vec![side_text.to_string()]
    } else if status {
        // Two lines as even as the words allow: "Settling," over "12 s",
        // "Waiting for" over "Beats". Never between a number and its
        // unit: "Under 10 s" over "Heard" (#2066).
        let number = |w: &str| w.chars().all(|c| c.is_ascii_digit());
        let splits = side_text
            .match_indices(' ')
            .map(|(i, _)| i)
            .filter(|&i| !side_text[..i].rsplit(' ').next().is_some_and(number));
        match splits.min_by_key(|&i| i.abs_diff(side_text.len() - i)) {
            Some(i) => vec![side_text[..i].to_string(), side_text[i + 1..].to_string()],
            None => vec![side_text.to_string()],
        }
    } else {
        // "± 8.6 s/d" stacks; "± 0.8°" and "bph" stay one line.
        match side_text.rsplit_once(' ') {
            Some((a, b)) if a.starts_with('±') && a.contains(' ') => {
                vec![a.to_string(), b.to_string()]
            }
            _ => vec![side_text.to_string()],
        }
    }
}

/// The lines of a ± and its unit (`side_text`, laid out as `lines`) that
/// `fits` beside the figure: as they are, else the ± stacked over the
/// unit, else the unit alone; and whether the ± was left out.
fn pm_fitted(
    side_text: &str,
    lines: Vec<String>,
    fits: impl Fn(&[String]) -> bool,
) -> (Vec<String>, bool) {
    if fits(&lines) {
        return (lines, false);
    }
    let (pm, unit) = match side_text.rsplit_once(' ') {
        Some((a, b)) if a.contains(' ') => (a, b),
        _ => (side_text, ""),
    };
    let stacked = vec![pm.to_string(), unit.to_string()];
    if lines.len() == 1 && !unit.is_empty() && fits(&stacked) {
        return (stacked, false);
    }
    let alone = if unit.is_empty() {
        Vec::new()
    } else {
        vec![unit.to_string()]
    };
    (alone, true)
}

/// A card figure's hover: what the reading waits on, then `hover`, with
/// the figure and the ± the card had no room for (`moved`) first.
fn figure_hover(status: Option<&(String, String)>, hover: &str, moved: Option<String>) -> String {
    let hover = match status {
        Some((_, h)) if !h.is_empty() && !hover.is_empty() => format!("{h}\n\n{hover}"),
        Some((_, h)) if !h.is_empty() => h.clone(),
        _ => hover.to_string(),
    };
    match moved {
        Some(m) if hover.is_empty() => m,
        Some(m) => format!("{m}\n\n{hover}"),
        None => hover,
    }
}

/// A notice in the status bar: a warning or a failure (`alert`) starts
/// with its icon, the words in the text colour; `hint` in its hover.
fn status_item(
    ui: &mut egui::Ui,
    text: &str,
    alert: Option<theme::Alert>,
    hint: &str,
) -> egui::Response {
    let pal = theme::pal(ui);
    let color = if alert.is_some() {
        pal.text
    } else {
        pal.accent
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let icon = |ui: &mut egui::Ui| {
            if let Some(kind) = alert {
                let (icon, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                theme::alert_icon(ui.painter(), icon, kind, pal);
            }
        };
        // The bar's right side runs right to left: the icon still
        // reads first.
        let rtl = ui.layout().prefer_right_to_left();
        if !rtl {
            icon(ui);
        }
        let resp = ui
            .add(
                egui::Button::new(RichText::new(text).small().color(color))
                    .frame_when_inactive(false),
            )
            .on_hover_text(hint);
        if rtl {
            icon(ui);
        }
        resp
    })
    .inner
}

/// A clock timed while listening as the hint gives it, `ppm` as its size
/// and its standard error `sd`, both to the decimals the ± calls for
/// (core's [`timegrapher_core::figure_text::figure_with_pm`], #510 R8):
/// "0.84 ± 0.06", "20.2 ± 0.3". The hint says "fast" or "slow" after it.
pub(super) fn timed_clock_text(ppm: f64, sd: f64) -> String {
    use timegrapher_core::figure_text::{base_digits, figure_with_pm};
    figure_with_pm(ppm.abs(), Some(sd), base_digits::CLOCK, false)
}

/// The day of a UTC timestamp kept as text ("2026-10-12T…").
pub(super) fn day_of(utc: &str) -> &str {
    utc.get(..10).unwrap_or(utc)
}

/// What limits a reading, as facts: a short form for the card's caption
/// and a sentence for its hover, in core's words. Nothing graded: the ±
/// beside the figure says how far it could be off; these say what the ±
/// can't show. Only the caveats that bear on the card titled `title`
/// alone, and, where its figure is `empty`, one that emptied it (#1975).
pub(super) fn reading_limits(
    title: &str,
    caveats: &[Caveat],
    lift_deg: f64,
    beats: usize,
    empty: bool,
) -> Vec<(String, String)> {
    let card = card_scope(title);
    let own: Vec<Caveat> = caveats
        .iter()
        .filter(|c| card.is_some() && (c.reading_scope() == card || (empty && empties(c) == card)))
        .copied()
        .collect();
    caveat_facts(&own, lift_deg, beats)
}

/// The caveats that bear on more than one card, or on the take, as
/// [`reading_limits`] gives a card's: said once, in the status bar. Not
/// one said on a card it emptied, the Amplitude card where
/// `amplitude_empty`.
pub(super) fn take_limits(
    caveats: &[Caveat],
    lift_deg: f64,
    beats: usize,
    amplitude_empty: bool,
) -> Vec<(String, String)> {
    let take: Vec<Caveat> = caveats
        .iter()
        .filter(|c| c.reading_scope().is_none())
        .filter(|c| !(amplitude_empty && empties(c) == Some(Scope::Amplitude)))
        .copied()
        .collect();
    caveat_facts(&take, lift_deg, beats)
}

/// The card a caveat leaves without a figure, said there though it bears
/// on more than that card (Hugh's rule 3 and Norm, #1975): sides too
/// faint to read, on the Amplitude card.
fn empties(c: &Caveat) -> Option<Scope> {
    matches!(c, Caveat::FaintSidesWithheld { .. }).then_some(Scope::Amplitude)
}

/// Each caveat's short form and sentence, but those said elsewhere.
pub(super) fn caveat_facts(
    caveats: &[Caveat],
    lift_deg: f64,
    beats: usize,
) -> Vec<(String, String)> {
    caveats
        .iter()
        .filter(|c| !said_elsewhere(c))
        .filter_map(|c| {
            let long = c.long(Some(lift_deg))?;
            let long = match c {
                Caveat::Clipped { .. } => format!("{long} {}", help::CLIPPING),
                _ => long,
            };
            Some((c.short(lift_deg, beats)?, long))
        })
        .collect()
}

/// The reading a card titled `title` shows, as a caveat's scope.
fn card_scope(title: &str) -> Option<Scope> {
    match title {
        "Rate" => Some(Scope::Rate),
        "Amplitude" => Some(Scope::Amplitude),
        "Beat Error" => Some(Scope::BeatError),
        _ => None,
    }
}

/// Caveats said elsewhere, never in a chip: no regular beat (the status
/// bar), the clocks disagreeing and the input clock (Rate controls row),
/// and the lift angle (Calibre card).
fn said_elsewhere(c: &Caveat) -> bool {
    matches!(
        c,
        Caveat::NoWatch { .. }
            | Caveat::ClockSourcesDisagree { .. }
            | Caveat::LiftUnknown
            | Caveat::ClockUnknown
    )
}

/// An error's words in the status bar: its first line, cut to 48
/// characters, with "…" saying a click shows the whole message.
pub(super) fn error_short(m: &str) -> String {
    let first = m.lines().next().unwrap_or_default();
    if first.chars().count() > 48 {
        format!("{}…", first.chars().take(47).collect::<String>())
    } else {
        format!("{first}…")
    }
}

/// The recording's state once the take it was saved in was found no
/// longer to hold its sound (#2052): never "Saved". `reachable`: the hours
/// it held alone are still there, so a save can keep the sound again: by
/// itself while the take is recorded into (`streams`), its next write
/// putting the whole take back (Hugh 4 on #2060), else by Save.
fn saved_copy_gone(
    reachable: bool,
    streams: bool,
    pal: &theme::Palette,
) -> (String, Color32, String) {
    if reachable {
        let hover = if streams {
            SAVED_COPY_GONE_STREAMING
        } else {
            SAVED_COPY_GONE
        };
        (
            "Not saved".to_string(),
            pal.text_secondary,
            hover.to_string(),
        )
    } else {
        (
            "Can't be saved".to_string(),
            pal.text_secondary,
            super::unsaved::HOURS_GONE.to_string(),
        )
    }
}

/// The hover of a session whose take no longer holds its sound, when a
/// save can keep it again.
const SAVED_COPY_GONE: &str =
    "The take this sound was saved in no longer holds it. Save keeps it again.";

/// The same while the take is recorded into, which Save isn't for.
const SAVED_COPY_GONE_STREAMING: &str = "The take this sound is being saved in no longer holds \
     it. The next few seconds of sound save it whole again.";
