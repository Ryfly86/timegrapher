# Bench Hypothesis Testing: Differential Diagnosis at the Case-Closed Stage

When a measurement or the person's own experience points to an anomaly—a periodic wave, a two-level torque drop, a steep isochronism slope, an odd positional drop, or an intermittent stall—act as an experienced watchmaker working beside them at the bench.

Follow three core principles throughout:

1. **Keep every test strictly case-closed.** Never ask the person to open the case back, touch a regulator, or poke inside the movement. Every test they run uses winding, setting the hands, operating pushers, turning or re-seating in the microphone stand, waiting, or running a demagnetiser.
2. **Form and rank hypotheses from what the program measures.** State what is most likely first, name the exact figures and caveats that point there, and explain what alternative causes could look similar.
3. **Pick the cheapest change at the bench that tells them apart.** Propose the fastest, least disruptive check first (a 1- to 5-minute check of clamp, settling, hand position, or complication state) before asking for an overnight run.

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

1. **Observe what was measured:** Read the rate, amplitude, and beat error, their standard errors (±), and every caveat the program reports (`watch_heard`, `beats_missing`, `clipped`, `sound_card_clock_uncertain`). Check Steadiness findings (`findings[]`) and period matches.
2. **Rank candidate hypotheses:** Trace from easiest setup artifacts down to deeper mechanical faults.
3. **Select the simplest differentiating action:** Pick one specific bench change that produces a distinct outcome for each competing hypothesis.
4. **Re-measure and update:** Judge the new readings against the baseline. If an artifact is ruled out and a mechanical fault is confirmed, refer to [diagnosis.md](diagnosis.md#once-the-case-is-open-what-to-inspect) for what a watchmaker should look for once the case is opened.

---

## Diagnostic Hierarchy (Where to Look First)

Work down this hierarchy so setup and handling issues are eliminated before suspecting a mechanical defect:

| Priority | Layer | What the Program Shows |
| :--- | :--- | :--- |
| **1** | **Set-up, acoustic contact & settling** | `watch_heard: false`, `beats_missing` caveat, `clipped` caveat, high `timing_scatter_fraction`, or an `Amplitude high` finding right after manual winding. |
| **2** | **External interference (Hands, dial, date, chronograph)** | Anomaly coincides with an angular position of the hands, occurs across the midnight date jump, or appears only when a chronograph is running. |
| **3** | **Power & gear train (Mainspring, barrel, train wheels)** | Both Tick and Tock amplitudes move together (`one_sided: false`, `unlock_jump: false`), repeating at a train wheel period identified by `periods` or stepping between torque levels in CUSUM. |
| **4** | **Escapement (Pallet stones, escape wheel, impulse pin)** | Asymmetry between Tick and Tock amplitudes, period matching the escape wheel, or extra events around unlock and drop in `shape`. |
| **5** | **Oscillator (Balance pivots, hairspring, poise)** | Divergence between positions in `d_rate` or vertical-to-horizontal amplitude drop `dvh_amplitude`, steep isochronism slope (`s_per_day_per_deg`), or erratic high rate with magnetism. |

---

## Bench Differential Tests (Examples to Draw On)

Select the test that best discriminates between your ranked hypotheses. The agent should judge which test fits the measured symptoms:

### Settling Check (Post-Wind Wait)
* **When to consider:** The program flags `Amplitude high` (or the trace shows jumps or double lines) immediately after the person manually wound the watch.
* **Competing hypotheses:** Temporary mainspring bridle torque peak (overbanking/knocking while freshly wound) vs. an incorrect lift angle setting or permanently over-strong mainspring.
* **Bench action:** Leave the watch undisturbed in the stand for 10 to 15 minutes to allow the mainspring coils and braking grease to settle, verify the lift angle in `calibres`, and record a fresh 60-second take in Dial Up.
* **How to judge:** If amplitude drops back into the calibre's normal full-wind range (see `normal_amplitude_full_wind` in `calibres`) and the trace is smooth, it was temporary settling. If amplitude remains excessively high with a jumping trace or knocking sounds in `shape` after settling, verify the lift angle; if lift angle is correct, genuine knocking is present.

### Clamp & Acoustic Contact Verification
* **When to consider:** A single position shows a `beats_missing` caveat, `watch_heard: false`, high `timing_scatter_fraction`, or an unexpected jump in beat error while adjacent positions are clean.
* **Competing hypotheses:** Loose acoustic coupling or stand resonance in that orientation vs. a true position-dependent fault.
* **Bench action:** Re-seat the watch in the clamp so the case back or crown firmly contacts the sensor without slipping or pinching, check microphone level with `timegrapher doctor`, and re-test that position for 60 to 90 seconds.
* **How to judge:** If normal beat detection returns and caveats clear, the earlier reading was a clamp artifact. If the symptom reproduces identically under good acoustic contact, it is a genuine positional anomaly.

### Targeted Positional Comparison
* **When to consider:** Suspected balance staff wear, endshake issue, or poise error after an initial take.
* **Competing hypotheses:**
  * **Dial Up (DU) vs. Dial Down (DD):** Compares upper and lower balance staff endstones, endshake, or hairspring sag.
  * **Horizontal (DU/DD) vs. Verticals:** Compares pivot-side friction in hole jewels (read via `dvh_amplitude`) against balance poise errors (rate divergence between opposite vertical positions, e.g. 3H vs. 9H, or 6H vs. 12H).
* **Bench action:** Record targeted 2-minute takes in the contrasting positions with 30 seconds rest after repositioning.
* **How to judge:** A uniform drop in amplitude across all vertical positions points to pivot-wall friction. Equal vertical amplitudes with rates that swing positive and negative between opposite verticals point to balance poise imbalance.

### Complication Load Check (Chronograph ON vs. OFF)
* **When to consider:** Chronograph watches, or when the user reports sluggishness or stoppage while timing.
* **Competing hypotheses:** Normal friction of the chronograph mechanism vs. excessive coupling-wheel engagement, friction-spring drag, or brake rubbing.
* **Bench action:** Record 2 to 3 minutes in Dial Up with the chronograph stopped, then start the chronograph and record another 3 to 5 minutes so the central seconds hand completes several full revolutions.
* **How to judge:** A small amplitude drop that stabilizes with no cyclical dips is normal chronograph load (rate may shift slightly following the calibre's isochronism slope). An ongoing amplitude slide, sudden stalls, or a recurring dip at the chronograph wheel period indicates excessive drag or mechanical binding.

### Hand Phase-Shift Check (Hand-to-Hand vs. Fixed Obstacle)
* **When to consider:** A periodic dip in amplitude or momentary rate jump occurs once per minute at a specific second mark.
* **Competing hypotheses:** The seconds or chronograph hand is grazing the minute or hour hand vs. hitting a fixed obstacle (dial marker, crystal underside) or a tooth burr on the wheel.
* **Bench action:** Pull the crown to the hand-setting position, advance the hour and minute hands by 4 to 6 hours away from the problem location, push the crown back in, and record another 3 to 5 minutes.
* **How to judge:** If the dip moves with the minute hand or disappears, it is hand-to-hand interference (hands out of level). If the dip remains locked to the exact same seconds mark regardless of hand position, it is a fixed dial/crystal clearance issue or a defect on that wheel.

### Date-Change Window Check
* **When to consider:** The watch loses time or stops overnight during the calendar change.
* **Competing hypotheses:** Excessive date-jumper friction or driving-wheel binding vs. general power reserve exhaustion.
* **Bench action:** Set the hands to 22:30 and record across midnight through the date change.
* **How to judge:** Compare amplitude before 23:00, during date-finger engagement, and after the calendar snaps over.

### Progressive Wind-Up from Dead Stop
* **When to consider:** Low full-wind amplitude or suspected premature mainspring bridle slip.
* **Competing hypotheses:** Mainspring bridle slipping prematurely vs. general gear-train friction.
* **Bench action:** Let the watch run down completely, then wind 10 turns at a time (e.g. 10, 20, 30, 40 turns), recording 60 seconds in Dial Up after each increment.
* **How to judge:** Amplitude should rise progressively and level off near full wind. A premature plateau (e.g. peaking at 15 turns with no further gain) points to bridle slippage.

### Multi-Hour or Overnight Wind-Down Run
* **When to consider:** Quick bench checks are clear, but long-term isochronism, CUSUM level shifts, or slow wheel periods (third wheel, centre wheel, barrel) need evaluation.
* **Bench action:** Fully wind the watch, allow it to settle, and record an extended run (e.g. 8 to 12 hours) in Dial Up.

---

## Escalating to the Watchmaker

When case-closed testing confirms a fault that cannot be resolved externally:
1. Summarize the confirmed findings and ruled-out causes.
2. Quote the specific figures and caveats measured by the program (calibre, rate, amplitude, beat error, `dvh_amplitude`, period matches).
3. Refer the watchmaker to the targeted open-case inspection points in [diagnosis.md](diagnosis.md#once-the-case-is-open-what-to-inspect).
