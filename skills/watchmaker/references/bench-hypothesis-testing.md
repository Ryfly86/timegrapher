# Bench Hypothesis Testing: Differential Diagnosis at the Case-Closed Stage

When a measurement or the person's own experience points to an anomaly—a periodic wave, a two-level torque drop, a steep isochronism slope, a positional difference, or an intermittent stall—act as an experienced watchmaker working beside them at the bench.

Follow three core principles throughout:

1. **Keep every test strictly case-closed.** Never ask the person to open the case back, touch a regulator, or poke inside the movement. Every test they run uses winding, setting the hands, operating pushers, turning or re-seating in the microphone stand, waiting, or running a demagnetiser.
2. **Form and rank hypotheses from what the program measures.** State what is most likely first, name the exact figures, `±` uncertainties, and caveats that point there, and explain what alternative causes could look similar.
3. **Pick the cheapest change at the bench that tells them apart.** Propose the fastest, least disruptive check first (such as re-clamping, letting the watch settle, shifting the hands, or toggling a complication) before asking for an overnight run.

---

## The Reasoning Loop

Rather than running a rigid sequence of tests, follow this iterative diagnostic loop:

```
    ┌────────────────────────────────────────────────────────┐
    │ 1. Observe Program Output & Caveats                    │
    │    (rate, amplitude, beat error, ±, flags, caveats)    │
    └───────────────────────────┬────────────────────────────┘
                                │
                                ▼
    ┌────────────────────────────────────────────────────────┐
    │ 2. Form & Rank Hypotheses                              │
    │    (Setup/settling → Hands/complications → Train → ...)│
    └───────────────────────────┬────────────────────────────┘
                                │
                                ▼
    ┌────────────────────────────────────────────────────────┐
    │ 3. Choose the Cheapest Case-Closed Differentiating Test│
    │    (re-clamp, settle wait, hand shift, chrono ON/OFF)  │
    └───────────────────────────┬────────────────────────────┘
                                │
                                ▼
    ┌────────────────────────────────────────────────────────┐
    │ 4. Measure Again & Update Hypotheses                   │
    │    (confirm, rule out, or escalate to watchmaker)      │
    └────────────────────────────────────────────────────────┘
```

1. **Observe what was measured:** Read the rate, amplitude, and beat error, their standard errors (`±`), and every caveat the program reports (`no_watch`, `beats_missing`, `clipped`, `clock_unknown`, `clock_sources_disagree`). Check Steadiness `findings[]` and period matches (`periods`).
2. **Rank candidate hypotheses:** Trace from setup and handling artefacts down to deeper mechanical faults.
3. **Select the simplest differentiating action:** Pick one specific bench change that produces a distinct outcome for each competing hypothesis.
4. **Re-measure and update:** Record long enough for the program to display a `±` on the metric being compared—rate `±` requires 30 × 10 s values (about 5 minutes after settle, per `measure::RATE_SE_MIN_S` and `confidence::MIN_NOISE_VALUES`), while amplitude `±` requires 30 × 2 s readings (1 minute after settle). Treat a shift between two readings as real only when it exceeds the `±` on both readings. Quick clamp or settling checks are valid when comparing amplitude because amplitude's `±` stabilises sooner. If an artefact is ruled out and a mechanical fault is confirmed, refer to [diagnosis.md](diagnosis.md#once-the-case-is-open-what-to-inspect) for what a watchmaker should inspect once the case is opened.

---

## Diagnostic Hierarchy (Where to Look First)

Work down this hierarchy so setup and handling issues are eliminated before suspecting a mechanical defect:

| Priority | Layer | What the Program Shows |
| :--- | :--- | :--- |
| **1** | **Set-up, acoustic contact & settling** | `watch_heard: false` (and its `no_watch` scatter caveats), `beats_missing`, `clipped`, or `amplitude_deg` well above the calibre's normal range right after manual winding. |
| **2** | **External interference (Hands, dial, date, chronograph)** | Anomaly coincides with an angular position of the hands, occurs across the midnight date jump, or appears only when a chronograph is running. |
| **3** | **Power & gear train (Mainspring, barrel, train wheels)** | In a two-state finding (`twostate`), both Tick and Tock amplitudes move together (`one_sided: false`, `unlock_jump: false`), repeating at a train wheel period identified by `periods` or stepping between torque levels in CUSUM. |
| **4** | **Escapement (Pallet stones, escape wheel, impulse pin)** | Asymmetry between Tick and Tock amplitudes, a period matching the escape wheel (`periods`), or extra events around unlock and drop in `shape`. |
| **5** | **Oscillator (Balance pivots, hairspring, poise)** | Positional rate or amplitude differences (`d_rate`, `dvh_amplitude`), an isochronism slope (`s_per_day_per_deg`), or a large, erratic fast rate with magnetism—evaluated using the program's reported figures and their `±`s (the program sets no built-in threshold for these). |

---

## Bench Differential Tests (Examples to Draw On)

Select the test that best discriminates between your ranked hypotheses. The agent should judge which test fits the measured symptoms:

### Settling Check (Post-Wind Wait)
* **When to consider:** `amplitude_deg` reads well above the calibre's normal range (see [when-to-see-a-watchmaker.md](when-to-see-a-watchmaker.md)) or the trace shows jumps or double lines immediately after the person manually wound the watch.
* **Competing hypotheses:** Temporary mainspring bridle torque peak while freshly wound vs. an incorrect lift angle setting or genuine knocking (overbanking).
* **Bench action:** Let the watch run untouched in the stand for a few minutes to allow the mainspring coils and braking grease to settle, verify the lift angle in `calibres`, and record a fresh take in Dial Up until amplitude displays its `±` (at least 1 minute after the program's 20 s settle, `take::SETTLE_S`).
* **How to judge:** If amplitude drops back into the calibre's normal full-wind range (per [when-to-see-a-watchmaker.md](when-to-see-a-watchmaker.md)) and the trace is smooth, it was temporary settling. If amplitude remains unusually high with a jumping trace or extra events in `shape` after settling, verify the lift angle; if lift angle is right, genuine knocking is present.

### Clamp & Acoustic Contact Verification
* **When to consider:** A single position shows `beats_missing`, `watch_heard: false` with its scatter caveats, or an unexpected jump in beat error while adjacent positions are clean.
* **Competing hypotheses:** Loose acoustic coupling or stand resonance in that orientation vs. a true position-dependent fault.
* **Bench action:** Re-seat the watch in the clamp so the case back or crown firmly contacts the sensor without slipping or pinching, check microphone level with `timegrapher doctor`, and re-test that position (allowing the program's 20 s settle, `take::SETTLE_S`, plus at least 1 minute for amplitude `±`, or about 5 minutes for rate `±`).
* **How to judge:** If normal beat detection returns and caveats clear, the earlier reading was a clamp artefact. If the symptom reproduces identically under good acoustic contact and exceeds the `±` on both readings, it is a genuine positional anomaly.

### Targeted Positional Comparison & Poise
* **When to consider:** Suspected balance staff wear, endshake issue, or poise error after an initial take.
* **Competing hypotheses:**
  * **Dial Up vs. Dial Down (`DU` vs. `DD`):** Compares upper and lower balance staff endstones, endshake, or hairspring sag.
  * **Horizontal vs. Vertical (`dvh_amplitude` and opposite verticals):** Compares pivot-side friction in hole jewels against balance poise errors (rate divergence between opposite vertical positions, such as Crown Left vs. Crown Right, or Crown Up vs. Crown Down).
* **Bench action:** Run the app's **Positions Test** (six positions: Dial Up, Dial Down, Crown Down, Crown Left, Crown Up, Crown Right, using the pane's settle and measure settings—settable in the pane, defaulting to 20 s `take::SETTLE_S`) to compare positional pairs, or run the app's **Dynamic Poise Test** (eight upright positions, 45° apart) when opposite verticals diverge and you need to separate poise from other positional effects.
* **How to judge:** A large drop in `dvh_amplitude` across all vertical positions points to pivot-wall friction. Even vertical amplitudes with a clear sine wave in rate across the eight upright positions of the Dynamic Poise Test point to balance poise imbalance.

### Complication Load Check (Chronograph ON vs. OFF)
* **When to consider:** Chronograph watches, or when the user reports sluggishness or stoppage while timing.
* **Competing hypotheses:** Normal friction of the chronograph mechanism vs. excessive coupling-wheel engagement, friction-spring drag, or brake rubbing.
* **Bench action:** Direct the user to run the app's **Complication Test** (in the Listen card / Complication pane, which runs chronograph off, then on, then off again with its own settle and measure windows and returns the program's **On less Off** table with its `±`). As a CLI fallback when not using the desktop app, record Dial Up with the chronograph stopped, then engaged, then stopped again—each long enough for a `±` on the metric compared (1 minute after settle for amplitude, about 5 minutes after settle for rate).
* **How to judge:** A modest amplitude drop in **On less Off** that stabilises with no cyclical dips is normal chronograph load (rate may shift slightly following the calibre's isochronism slope). An ongoing amplitude slide, sudden stalls, or a recurring dip matching the chronograph wheel in `periods` indicates excessive drag or mechanical binding.

### Hand Phase-Shift Check (Hand-to-Hand vs. Fixed Obstacle)
* **When to consider:** A periodic dip in amplitude or momentary rate jump matches a line the program identifies as the fourth wheel (`periods`) or occurs once per minute.
* **Competing hypotheses:** The seconds or chronograph hand is grazing the minute or hour hand vs. hitting a fixed obstacle (dial marker, crystal underside) or a tooth burr on the wheel.
* **Bench action:** Pull the crown to the hand-setting position, advance the hour and minute hands by several hours away from the suspect sector, push the crown back in, and record again (after the program's 20 s settle, long enough to capture several revolutions and a `±` on the metric compared).
* **How to judge:** If the dip moves with the minute hand or disappears, it is hand-to-hand interference (hands out of level). If the dip remains locked to the exact same seconds mark regardless of hand position, it is a fixed dial/crystal clearance issue or a defect on that wheel.

### Date-Change Window Check
* **When to consider:** The watch loses time or stops overnight during the calendar change.
* **Competing hypotheses:** Excessive date-jumper friction or driving-wheel binding vs. general power reserve exhaustion.
* **Bench action:** Set the hands well before the date begins to change for this calibre (check calibre notes if known) and record across midnight through the date change.
* **How to judge:** Compare amplitude before engagement begins, during date-finger engagement, and after the calendar snaps over, checking whether any drop exceeds the `±` on both stretches.

### Progressive Wind-Up from Dead Stop
* **When to consider:** Low full-wind amplitude or suspected premature mainspring bridle slip.
* **Competing hypotheses:** Mainspring bridle slipping prematurely vs. general gear-train friction.
* **Bench action:** Let the watch run down completely, then wind 10 turns at a time (e.g. 10, 20, 30, 40 turns), recording Dial Up for settle plus at least 1 minute (`≥ 80 s` total with the default 20 s `take::SETTLE_S`) after each increment so every step collects the 30 amplitude readings needed for its `±`.
* **How to judge:** Amplitude should rise progressively across steps and level off near full wind. A premature plateau—for example, peaking after the second step with no further gain—points to bridle slippage.

### Multi-Hour or Overnight Wind-Down Run
* **When to consider:** Quick bench checks are clear, but long-term isochronism, CUSUM level shifts, or slow wheel periods (third wheel, centre wheel, barrel) need evaluation.
* **Bench action:** Fully wind the watch, let it run for a few minutes to settle, and record an extended run (e.g. 8 to 12 hours) in Dial Up.

---

## Escalating to the Watchmaker

When case-closed testing confirms a fault that cannot be resolved externally:
1. Summarise the confirmed findings and ruled-out causes.
2. Quote the specific figures, `±` uncertainties, and caveats measured by the program (calibre, rate, amplitude, beat error, `dvh_amplitude`, period matches).
3. Refer the watchmaker to the targeted open-case inspection points in [diagnosis.md](diagnosis.md#once-the-case-is-open-what-to-inspect).
