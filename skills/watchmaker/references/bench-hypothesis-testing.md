# Agent-Assisted Bench Hypothesis Testing & Open-Case Inspection

When a measurement or the person's own experience points to an anomaly—a periodic wave, a two-level torque drop, a steep isochronism slope, an odd positional drop, or an intermittent stall—act as an experienced watchmaker working beside them at the bench.

Follow three rules throughout:

1. **Interactive testing stays strictly case-closed.** Never ask the person to open the case back, touch a regulator, or poke inside the movement. Every test they run uses winding, setting the hands, operating pushers, turning the microphone stand, or running a demagnetiser.
2. **Rank your hypotheses, and name the evidence for each.** State what is most likely first, why the numbers point there, and what alternative causes could look similar.
3. **Propose the quickest confirming test first.** Always offer a 1- to 5-minute check that can confirm or discard a hypothesis before asking for an 8- to 12-hour overnight run.

---

## 1. Forming Ranked Hypotheses from Measurements

Before blaming a mechanical part, walk this hierarchy:

| Priority | Layer | Evidence to Check First |
| :--- | :--- | :--- |
| **1** | **Set-up, handling & settling** | `watch_heard` false, `beats_missing` (<95% beats), `clipped` (>1%), unlock coverage <60%, wrong lift angle or beat rate, or a watch tested within 1–2 minutes of a full manual wind (`>330°` amplitude knocking). |
| **2** | **External interference (Hands, dial, date, chronograph pushers)** | Anomaly happens only at a specific second/minute/hour mark, only near midnight, or only with the chronograph running. |
| **3** | **Upstream power & gear train (Mainspring, barrel, train wheels)** | **Both Tick and Tock** amplitude move together (`one_sided: false`, `unlock_jump: false`), repeating at a wheel period (`60 s` fourth wheel, `~3–10 min` third/intermediate wheel, `~5–7 h` barrel) or stepping between two torque levels. |
| **4** | **Escapement (Pallet stones, escape wheel, fork horns)** | Asymmetry between Tick and Tock (`>20°` split on a clean signal with low beat error), wave at the escape-wheel period (`3.75 s` or `5.0 s` at 28,800 bph), or abnormal unlock-to-impulse timing in `shape`. |
| **5** | **Oscillator (Balance pivots, hairspring, regulator pins, poise)** | Large positional rate delta (`d_rate`), vertical amplitude more than 50° below horizontal (`dvh_amplitude`), steep positive/negative isochronism slope (`s_per_day_per_deg`), or huge erratic fast rate (magnetism). |

---

## 2. Catalog of Case-Closed Differential Bench Tests

When two or more causes could explain a finding, ask the person to run the **shortest test** below that separates them.

### Test A — Post-Wind Settling Re-Test (10–15 min wait + 60 s DU)
* **When to propose:** Amplitude reads `>320°–330°` with a fast rate jump and/or `beats_missing` right after the person wound the watch.
* **Hypothesis tested:** Mainspring bridle peak-torque overbanking (knocking) vs. a permanently over-strong mainspring or wrong lift angle.
* **Instruction:** *"Leave the watch untouched for 10 to 15 minutes so the automatic bridle and barrel coils settle, make sure the clamp is snug, and record a fresh 60-second take in Dial Up."*
* **Confirm / Discard:**
  * *Drops back to 275°–305° with clean beats:* Temporary post-wind bridle spike (normal; remind them always to wait 10–15 min after winding).
  * *Stays >330° with jumping trace after 15+ min:* Check lift angle first; if lift angle is right, genuine knocking (wrong mainspring or excessive barrel torque).

### Test B — Firm Re-Clamp & Single-Position Re-Test (60–90 s)
* **When to propose:** One position shows `<95%` beats found (`beats_missing`), `<60%` unlock coverage (`unlock_unreliable`), or a sudden jump in beat error/amplitude while adjacent positions are clean.
* **Hypothesis tested:** Loose acoustic contact in that stand orientation vs. a position-specific mechanical fault.
* **Instruction:** *"Re-seat the watch in the clamp for that position so the case back or crown/side presses firmly against the microphone pickup without wobbling, wait 30 seconds, and record 60 to 90 seconds."*
* **Confirm / Discard:**
  * *Clean beats (>95%) and normal beat error return:* Clamp artifact; discard the earlier reading.
  * *Repeats identically on a strong signal:* Real positional fault in that orientation.

### Test C — Targeted Positional Pairs (2 min per position)
* **When to propose:** A single Dial Up run is complete, or the watch keeps a different rate on the wrist than on the stand.
* **Hypotheses tested:**
  1. **Dial Up (DU) vs. Dial Down (DD):** Tests balance endstones, upper vs. lower pivot wear, endshake, or a hairspring out of flat.
  2. **Horizontal (DU/DD) vs. Verticals (9H/CD, 6H/CL, 3H/CU, 12H/CR):**
     * *Amplitude drops >50° (`dvh_amplitude < -50°`):* Pivot-side friction in the hole jewels (worn/dirty pivot walls or dry oil).
     * *Vertical amplitudes are even (~240°–270°), but vertical rates diverge strongly between opposite verticals (e.g., 3H vs. 9H, or 6H vs. 12H):* Balance poise error or hairspring off-centre.

### Test D — Complication Load Test: Chronograph ON vs. OFF (2–5 min DU)
* **When to propose:** The watch is a chronograph (e.g., Valjoux 7750), or the user reports stalling, sluggishness, or time loss when the chronograph runs.
* **Hypothesis tested:** Excessive chronograph coupling drag, oscillating-pinion depth error, brake/hammer rub, or a once-per-minute bind on the central chronograph seconds wheel.
* **Best practice:** Run it both near full wind and after 12–24 hours unwound (when lower baseline torque exposes marginal binds).
* **Instruction:** *"With the watch in Dial Up, press Start to engage the chronograph and record for 3 to 5 minutes so the chrono seconds hand makes several full turns."*
* **Confirm / Discard:**
  * *Small to moderate amplitude drop (~15°–30°) that stabilizes above ~235°–240° with no 60 s blips or stalls:* Normal chronograph friction load. (Note: on a movement with a positive isochronism slope, a 25°–30° amplitude drop will naturally shift the rate slower by `drop × slope`.)
  * *Amplitude slides continuously below ~225°–230°, shows a sharp 60-second notch, or stalls at a fixed second mark (e.g., 15 s or 58–60 s):* Confirmed chronograph-train drag or hand interference. Advise leaving the chronograph off until serviced.

### Test E — Hand-Repositioning Phase-Shift Test (2–5 min DU)
* **When to propose:** A periodic dip or stall happens once per minute (e.g., every time the seconds or chrono-seconds hand passes the 15-second mark).
* **Hypothesis tested:** Seconds/chrono hand grazing the **minute or hour hand** vs. hitting a fixed **dial marker, crystal, or wheel-tooth burr**.
* **Instruction:** *"Pull the crown to time-setting, advance the hour and minute hands by 4 to 6 hours so they are well clear of the point where the stall/dip happened, push the crown back in, and record another 3 to 5 minutes."*
* **Confirm / Discard:**
  * *The dip/stall disappears or moves to the new minute-hand position:* **Hand-to-hand grazing** (bent seconds or minute hand).
  * *The dip/stall stays locked to the exact same second mark (e.g., still at 15 s):* Fixed angular cause—either the hand touching the dial/crystal at that spot, or a tooth/cam fault on the 60-second wheel itself.

### Test F — Date-Change Window Test (30–90 min across midnight)
* **When to propose:** The watch loses time or stops overnight around midnight.
* **Hypothesis tested:** Excessive date-jumper spring tension or dry date-driving wheel binding the motion work.
* **Instruction:** *"Advance the hands gently until the date just clicks over, then set the time to 22:30 and record in Dial Up for 2 hours as it passes through midnight."*
* **Confirm / Discard:** Compare the amplitude drop during engagement against the baseline before 22:30 and after the date jumps.

### Test G — Crown-Turn Wind-Up Progression Test (From Dead Stop)
* **When to propose:** Suspected early bridle slipping, worn barrel wall, or low full-wind amplitude.
* **Instruction:** *"When the watch has completely run down and stopped, wind it 10 turns at a time (up to 40 turns), recording 60 seconds in Dial Up after each set of 10 turns."*
* **Confirm / Discard:** Amplitude should climb steadily from 10 to 30 turns and plateau around 35–40 turns. If amplitude peaks at 15 turns and never climbs higher (or drops sharply on further winding), the automatic bridle is slipping prematurely.

### Test H — Overnight / Multi-Hour Wind-Down Run (8–12 h DU)
* **When to propose:** After quick checks are done and you want to verify the full mainspring torque curve, full-reserve isochronism slope, CUSUM level segments, Allan deviation floor, and multi-hour wheel periodograms (third wheel, centre wheel, barrel).
* **Instruction:** *"Give the watch a full wind (30–40 turns), let it settle for 10 to 15 minutes, prevent the computer from sleeping (on macOS: `caffeinate -ims -t 46800`), and record 8 to 12 hours in Dial Up undisturbed."*

---

## 3. The 4-Stage "Once the Case Is Opened" Watchmaker Checklist

At the end of an assisted diagnostic session—or whenever generating a formal diagnostic report—translate the confirmed findings into a **4-stage bench inspection sequence** for the watchmaker who opens the case.

Order the steps strictly from **least invasive (dial/hands intact)** to **full disassembly**, including only the checks relevant to what was measured (plus standard baseline checks for that calibre):

### Stage 1 — Dial Side & Hands (Before Uncasing the Movement)
* **Hand clearances:** Sight across the dial at eye level under a loupe while sweeping the hands through 12 hours (and sweeping the chronograph hands through 60 seconds). Check clearance between the seconds/chrono-seconds hand and the minute tube, hour hand, applied markers, subdial hands, and crystal—especially at any angular sector flagged during testing (e.g., 15 s / 3 o'clock).
* **Dial & motion work:** If a 60-minute or midnight amplitude dip was detected, check dial-washer height, cannon-pinion slip torque, and date-jumper lubrication before pulling the dial.

### Stage 2 — Movement Running in Holder (Balance & Train Intact)
* **Hairspring & regulator pins:**
  * If a steep positive or negative **isochronism slope** (`s_per_day_per_deg`) was measured on an index-regulated calibre (e.g., Valjoux 7750, ETA 2824-2), inspect regulator-pin clearance and centering: ensure the outer hairspring coil is centered without excessive gap or permanent bias against one pin, and verify hairspring flatness/concentricity.
  * If **beat error is low (<0.2 ms)** and **isochronism is flat** (e.g., free-sprung or well-tuned balance), explicitly note: **"Preserve balance/hairspring stud and collet geometry; regulate via index/timing screws only."**
* **Chronograph coupling & blocking levers (if applicable):**
  * Engage and disengage the chronograph while observing under the microscope: check **oscillating-pinion / coupling-wheel engagement depth** (adjust the eccentric screw if set too deep and stealing >30° of amplitude, or too shallow and skipping).
  * Check that the **brake lever** and **reset hammers** clear the central seconds and counter heart cams cleanly in the running state across all 360° of rotation.
* **Endshakes:** Check balance-staff and escapement endshakes in horizontal and vertical attitudes if `dvh_amplitude` or DU vs. DD differed significantly.

### Stage 3 — Power Let-Down & Free-Spin Check (Balance & Pallet Fork Removed)
* Safely let down the mainspring, remove the balance assembly and pallet cock/fork, and apply a light wind to observe the going train spinning freely:
  * **Wheel runout & meshing:** Watch the wheels identified by the periodogram (e.g., the **fourth wheel** for a 60 s cycle, or the **third / intermediate / chronograph driving wheel** for a non-60s cycle such as ~176 s). Look for radial runout (eccentricity), axial wobble (bent pivot or worn jewel hole), or binding at a specific quadrant.
  * **Sweep-seconds tension / friction spring:** On indirect or friction-sprung seconds calibres (e.g., 31xx, 7750), inspect the friction spring pressing on the seconds pinion/runner for excessive downward drag, burrs, or dry contact.

### Stage 4 — Disassembly & Microscope Inspection
* **Specific wheel teeth & pinion leaves:** Inspect every tooth and pinion leaf of any wheel that showed a periodic rate/amplitude wave or two-state dip under high magnification for burrs, impacted debris, or uneven wear.
* **Escapement & balance pivots:** Inspect pallet-stone locking and impulse faces for grooving, balance pivots for burnishing ormushrooming (especially if vertical amplitude dropped >50°), and cap jewels for epilame failure or oil pooling.
* **Mainspring barrel & slipping bridle:**
  * If post-wind knocking (`>330°`) or multi-hour CUSUM torque steps were observed, open the barrel drum, clean old braking grease, inspect the barrel wall and bridle for scoring, and re-lubricate with the calibre-appropriate braking grease (e.g., Kluber P125 or Moebius 8217) so the bridle slips smoothly at full wind.
