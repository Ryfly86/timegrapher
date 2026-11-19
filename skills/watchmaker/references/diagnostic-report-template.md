# 4-Page Technical Diagnostic Report & Open-Case Bench Sheet

When the person asks for a formal diagnostic report (or at the conclusion of an assisted bench-testing session), produce a publication-grade **4-page Technical Report** containing:

1. **Executive Summary for Watchmaker** (including the verdict or "no verdict yet" for single-position takes)
2. **Section 1 — Key Metrics Table** (`Rate`, `Amplitude`, `Beat Error`, `Intrinsic Stability (ADEV)`, `Isochronism`, with Measured Value/Median, Middle-80% Operating Range, and Technical Assessment)
3. **Section 2 — Time Series, CUSUM Control & Gear Train Periodogram** (with data-driven SVG charts)
4. **Section 3 — Autocorrelation, Allan Deviation & Escapement/Acoustic Summary** (with data-driven ACF and ADEV SVG charts)
5. **Section 4 — Isochronism Curve & 4-Stage "Once the Case Is Opened" Watchmaker Inspection Sequence** (Stage 1: Dial & Hands → Stage 2: Movement Running in Holder → Stage 3: Power Let-Down & Free-Spin → Stage 4: Disassembly & Microscope)

---

## Two Ways to Generate the Report (Python Optional)

Because some users do not have Python installed, **never require Python to produce a report**. Check whether `uv` or `python3` is available first, or write the HTML/Markdown directly:

### Option A — Optional Python CLI Generator (When `uv` or `python3` Is Available)

Run `.agents/skills/watchmaker/scripts/generate_bench_report.py` on the `check_steadiness` output file:

```sh
uv run .agents/skills/watchmaker/scripts/generate_bench_report.py \
  --steadiness /path/to/check_steadiness_output.txt \
  --watch "Watch Name" \
  --calibre "Calibre Name (28,800 bph, 49° lift angle)" \
  --date "November 18, 2026" \
  --out /path/to/diagnostic_report.html
```

*(If `uv` is not installed but `python3` is present, invoke `python3 .agents/skills/watchmaker/scripts/generate_bench_report.py ...`. The script uses only the Python 3 standard library: `argparse`, `json`, `math`, `os`.)*

You can also pass custom 4-stage open-case inspection notes determined during interactive hypothesis testing:

```sh
  --stage1 "Check chrono-seconds hand clearance at the 15-second mark (3 o'clock) against minute hand and dial markers." \
  --stage2 "Inspect oscillating-pinion eccentric screw engagement depth (30° chrono-ON drop) and regulator-pin gap (+0.37 s/d/° slope)." \
  --stage3 "With balance and fork removed, check free-spin of the intermediate/third wheel for the 176.5 s (7.7°) two-level torque dip." \
  --stage4 "Microscope check of central chronograph wheel teeth at 90° (15 s) and barrel-wall braking grease."
```

### Option B — Direct File Write Fallback (No Python Required)

If neither `uv` nor `python3` is available on the computer, read the JSON fields directly from `check_steadiness`, `long_run`, and `run_session` and write the `.html` (and/or Markdown) report directly with your file-writing tool using the layout below:

#### Required Data Fields to Extract from `check_steadiness` / `run_session`
- **Overall Distributions (`distributions.overall`):**
  - `rate` (`values: "readings"`): `median`, `p10`, `p90`, `n`
  - `amplitude` (`values: "readings"`): `median`, `p10`, `p90`, `n`
  - `beat_error` (`values: "readings"`): `median`, `p10`, `p90`, `n`
- **Hourly Stretches (`distributions.stretches[]`):** Each stretch's `start_s`, `end_s`, and `rate_readings` / `amplitude_readings` / `beat_error_readings` (`median`, `p10`, `p90`).
- **Series Statistics (`series[]` for `rate`, `amplitude`, `beat_error`):**
  - `allan`: `tau_s[]`, `deviation[]`, `best_tau_s`, `best_deviation`
  - `autocorrelation`: `lag_s[]`, `r[]`, `band`, `repeat_lag_s`, `repeat_r`, `p_value`
  - `cusum`: `max`, `at_s`, `band`, `p_value`, `shift`, `t_s[]`, `s[]`
  - `cycle` & `period`: `period_s`, `peak_to_peak`, `wheel`, `explained`
  - `two_state`: `two_levels`, `high`, `low`, `dwell_high_s`, `dwell_low_s`, `one_sided`
- **Isochronism (`isochronism`):**
  - `s_per_day_per_deg`, `s_per_day_per_deg_se`, `turn_deg`, `amplitude_from_deg`, `amplitude_to_deg`, `bins[]` (`amplitude_deg`, `rate_s_per_day`, `rate_q25`, `rate_q75`), `curve` (`[c0, c1, c2]`), `curve_centre_deg`.

#### 4-Page Print CSS Skeleton
Use `@page { size: letter portrait; margin: 18mm 16mm; }` and wrap each of the 4 pages in `<div class="page">` with `page-break-after: always;` so opening the HTML file in any browser and choosing **Print → Save as PDF** produces an exact 4-page PDF report.
