#!/usr/bin/env python3
"""Generate a 4-page printable HTML horological diagnostic report from timegrapher check_steadiness output.

Uses only the Python 3 standard library so no third-party packages are required.
"""

import argparse
import datetime
import html
import json
import math
import os


def load_steadiness_json(path: str) -> dict:
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    idx = text.find('{"beat_error_source"')
    if idx == -1:
        idx = text.find("{")
    if idx == -1:
        raise ValueError(f"No JSON payload found in {path}")
    data, _ = json.JSONDecoder().raw_decode(text[idx:])
    return data


def build_hourly_or_twostate_svg(data: dict, rate_s: dict, amp_s: dict) -> tuple[str, str]:
    stretches = data.get("distributions", {}).get("stretches", [])
    w, h = 740, 185
    pad_l, pad_r, pad_t, pad_b = 52, 52, 22, 28
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b

    if len(stretches) >= 2:
        max_h = max(st["end_s"] / 3600.0 for st in stretches)
        r_vals, a_vals = [], []
        for st in stretches:
            d_map = {d["series"] + "_" + d["values"]: d for d in st["distributions"]}
            r = d_map.get("rate_readings", {})
            a = d_map.get("amplitude_readings", {})
            if "p10" in r and "p90" in r:
                r_vals.extend([r["p10"], r["p90"], r["median"]])
            if "median" in a:
                a_vals.append(a["median"])

        r_min = math.floor(min(r_vals) - 1.0) if r_vals else -5.0
        r_max = math.ceil(max(r_vals) + 1.0) if r_vals else 15.0
        a_min = math.floor((min(a_vals) - 5.0) / 5.0) * 5.0 if a_vals else 250.0
        a_max = math.ceil((max(a_vals) + 5.0) / 5.0) * 5.0 if a_vals else 300.0

        def rx(t_h):
            return pad_l + (t_h / max(max_h, 0.1)) * plot_w

        def ry_rate(val):
            return pad_t + plot_h - ((val - r_min) / max(r_max - r_min, 1.0)) * plot_h

        def ry_amp(val):
            return pad_t + plot_h - ((val - a_min) / max(a_max - a_min, 1.0)) * plot_h

        rate_pts, amp_pts, rate_top, rate_bot = [], [], [], []
        for st in stretches:
            t_mid = (st["start_s"] + st["end_s"]) / 7200.0
            d_map = {d["series"] + "_" + d["values"]: d for d in st["distributions"]}
            r = d_map.get("rate_readings", {})
            a = d_map.get("amplitude_readings", {})
            x = rx(t_mid)
            if "median" in r:
                rate_pts.append(f"{x:.1f},{ry_rate(r['median']):.1f}")
                rate_top.append(f"{x:.1f},{ry_rate(r.get('p90', r['median'])):.1f}")
                rate_bot.append(f"{x:.1f},{ry_rate(r.get('p10', r['median'])):.1f}")
            if "median" in a:
                amp_pts.append(f"{x:.1f},{ry_amp(a['median']):.1f}")

        band_poly = " ".join(rate_top + list(reversed(rate_bot)))
        grid = ""
        for i in range(4):
            rv = r_min + (i + 0.5) * (r_max - r_min) / 4.0
            y = ry_rate(rv)
            grid += f'<line x1="{pad_l}" y1="{y:.1f}" x2="{w-pad_r}" y2="{y:.1f}" stroke="#1e293b" stroke-width="1"/>'
            grid += f'<text x="{pad_l-6}" y="{y+3:.1f}" fill="#38bdf8" font-size="8.5" text-anchor="end" font-family="monospace">{rv:+.1f}</text>'
        for i in range(4):
            av = a_min + (i + 0.5) * (a_max - a_min) / 4.0
            y = ry_amp(av)
            grid += f'<text x="{w-pad_r+6}" y="{y+3:.1f}" fill="#fbbf24" font-size="8.5" text-anchor="start" font-family="monospace">{av:.0f}°</text>'

        xticks = "".join(
            f'<text x="{rx(hr):.1f}" y="{h-8}" fill="#64748b" font-size="8.5" text-anchor="middle" font-family="monospace">{hr:.1f}h</text>'
            for hr in [0.0, max_h * 0.25, max_h * 0.5, max_h * 0.75, max_h]
        )
        svg = f"""
        <svg viewBox="0 0 {w} {h}" style="width:100%; height:auto; display:block;">
          <rect width="{w}" height="{h}" fill="#090d16" rx="4"/>
          {grid}
          <line x1="{pad_l}" y1="{h-pad_b}" x2="{w-pad_r}" y2="{h-pad_b}" stroke="#334155" stroke-width="1"/>
          {xticks}
          <polygon points="{band_poly}" fill="#38bdf8" fill-opacity="0.14"/>
          <polyline points="{' '.join(amp_pts)}" fill="none" stroke="#fbbf24" stroke-width="2" stroke-dasharray="4 3"/>
          <polyline points="{' '.join(rate_pts)}" fill="none" stroke="#38bdf8" stroke-width="2.4"/>
          <text x="{pad_l}" y="14" fill="#38bdf8" font-size="8.5" font-weight="bold" font-family="monospace">Rate Median &amp; 80% Band (s/d)</text>
          <text x="{w-pad_r}" y="14" fill="#fbbf24" font-size="8.5" font-weight="bold" text-anchor="end" font-family="monospace">Amplitude Median (deg)</text>
        </svg>"""
        caption = f"<strong>Figure 1. Multi-Hour Progression (Real Data, {max_h:.1f} h):</strong> Hourly medians and middle-80% rate band (blue) alongside hourly amplitude medians (gold)."
        return svg, caption

    # Fallback for shorter takes (<2 hours): plot two-state / baseline summary
    r_med = rate_s.get("median", 0.0)
    a_med = amp_s.get("median", 270.0)
    svg = f"""
    <svg viewBox="0 0 {w} {h}" style="width:100%; height:auto; display:block;">
      <rect width="{w}" height="{h}" fill="#090d16" rx="4"/>
      <line x1="{pad_l}" y1="{h/2:.1f}" x2="{w-pad_r}" y2="{h/2:.1f}" stroke="#38bdf8" stroke-width="2.2"/>
      <text x="{pad_l}" y="16" fill="#38bdf8" font-size="9" font-family="monospace">Median Rate: {r_med:+.2f} s/d | Median Amplitude: {a_med:.1f}°</text>
    </svg>"""
    return svg, "<strong>Figure 1. Session Baseline Overview:</strong> Summary of rate and amplitude across the recording."


def build_cusum_svg(rate_s: dict) -> str:
    w, h = 350, 160
    pad_l, pad_r, pad_t, pad_b = 34, 12, 22, 24
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b
    cusum = rate_s.get("cusum", {})
    cs = cusum.get("s", [0.0, 0.0])
    ts = cusum.get("t_s", [0.0, 1.0])
    if not isinstance(cs, list) or not isinstance(ts, list) or len(cs) < 2:
        cs, ts = [0.0, 0.0], [0.0, 1.0]
    band = cusum.get("band", 1.36)
    max_t = max(ts[-1], 1.0)
    c_min = min(min(cs), -band * 1.2)
    c_max = max(max(cs), band * 1.2)

    def cx(t):
        return pad_l + (t / max_t) * plot_w

    def cy(v):
        return pad_t + plot_h - ((v - c_min) / max(c_max - c_min, 0.5)) * plot_h

    step = max(1, len(ts) // 120)
    pts = [f"{cx(t):.1f},{cy(v):.1f}" for t, v in zip(ts[::step], cs[::step])]
    return f"""
    <svg viewBox="0 0 {w} {h}" style="width:100%; height:auto;">
      <text x="10" y="14" fill="#94a3b8" font-size="9" font-family="monospace" font-weight="bold">Rate CUSUM (max {cusum.get('max', 0.0):.2f}, band ±{band:.2f})</text>
      <line x1="{pad_l}" y1="{cy(0):.1f}" x2="{w-pad_r}" y2="{cy(0):.1f}" stroke="#334155" stroke-width="1"/>
      <line x1="{pad_l}" y1="{cy(band):.1f}" x2="{w-pad_r}" y2="{cy(band):.1f}" stroke="#ef4444" stroke-dasharray="3 2"/>
      <line x1="{pad_l}" y1="{cy(-band):.1f}" x2="{w-pad_r}" y2="{cy(-band):.1f}" stroke="#ef4444" stroke-dasharray="3 2"/>
      <polyline points="{' '.join(pts)}" fill="none" stroke="#22c55e" stroke-width="1.8"/>
      <text x="{cx(0):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" font-family="monospace">0h</text>
      <text x="{cx(max_t/2):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" text-anchor="middle" font-family="monospace">{max_t/7200:.1f}h</text>
      <text x="{cx(max_t):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" text-anchor="end" font-family="monospace">{max_t/3600:.1f}h</text>
    </svg>"""


def build_acf_svg(rate_s: dict) -> str:
    w, h = 350, 160
    pad_l, pad_r, pad_t, pad_b = 34, 12, 22, 24
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b
    acf = rate_s.get("autocorrelation", {})
    lags = acf.get("lag_s", [0.0, 600.0])
    rs = acf.get("r", [1.0, 0.0])
    band = acf.get("band", 0.05)
    max_lag = max(lags[-1], 1.0)

    def ax(lag):
        return pad_l + (lag / max_lag) * plot_w

    def ay(rv):
        return pad_t + plot_h - ((rv - (-0.35)) / 1.4) * plot_h

    pts = [f"{ax(l):.1f},{ay(r):.1f}" for l, r in zip(lags, rs)]
    y0, yb_hi, yb_lo = ay(0.0), ay(band), ay(-band)
    rep_lag = acf.get("repeat_lag_s")
    rep_r = acf.get("repeat_r")
    lbl = f"repeat {rep_lag:.0f}s (r={rep_r:.2f})" if rep_lag and rep_r else "ACF"

    return f"""
    <svg viewBox="0 0 {w} {h}" style="width:100%; height:auto;">
      <text x="10" y="14" fill="#94a3b8" font-size="9" font-family="monospace" font-weight="bold">Rate Autocorrelation ({lbl})</text>
      <line x1="{pad_l}" y1="{y0:.1f}" x2="{w-pad_r}" y2="{y0:.1f}" stroke="#334155" stroke-width="1"/>
      <rect x="{pad_l}" y="{yb_hi:.1f}" width="{plot_w}" height="{max(yb_lo-yb_hi, 1.0):.1f}" fill="#475569" fill-opacity="0.25"/>
      <polyline points="{' '.join(pts)}" fill="none" stroke="#38bdf8" stroke-width="1.6"/>
      <text x="{ax(0):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" font-family="monospace">0s</text>
      <text x="{ax(max_lag/2):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" text-anchor="middle" font-family="monospace">{max_lag/2:.0f}s</text>
      <text x="{ax(max_lag):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" text-anchor="end" font-family="monospace">{max_lag:.0f}s</text>
    </svg>"""


def build_allan_svg(rate_s: dict) -> str:
    w, h = 350, 160
    pad_l, pad_r, pad_t, pad_b = 38, 14, 22, 24
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b
    allan = rate_s.get("allan", {})
    taus = allan.get("tau_s", [10.0, 100.0])
    devs = allan.get("deviation", [1.0, 0.3])

    min_lt, max_lt = math.log10(min(taus)), math.log10(max(taus))
    min_ld = math.log10(max(min(devs) * 0.7, 0.05))
    max_ld = math.log10(max(devs) * 1.4)

    def lx(t):
        return pad_l + ((math.log10(t) - min_lt) / max(max_lt - min_lt, 0.1)) * plot_w

    def ly(d):
        return pad_t + plot_h - ((math.log10(max(d, 0.01)) - min_ld) / max(max_ld - min_ld, 0.1)) * plot_h

    pts = [f"{lx(t):.1f},{ly(d):.1f}" for t, d in zip(taus, devs)]
    dots = "".join(f'<circle cx="{lx(t):.1f}" cy="{ly(d):.1f}" r="2.5" fill="#a855f7"/>' for t, d in zip(taus, devs))
    w_pts = [f"{lx(t):.1f},{ly(devs[0]*math.sqrt(taus[0]/t)):.1f}" for t in taus]

    best_t = allan.get("best_tau_s", taus[-1])
    best_d = allan.get("best_deviation", devs[-1])

    return f"""
    <svg viewBox="0 0 {w} {h}" style="width:100%; height:auto;">
      <text x="10" y="14" fill="#94a3b8" font-size="9" font-family="monospace" font-weight="bold">Rate Allan Deviation (best {best_d:.2f} s/d at {best_t:.0f}s)</text>
      <line x1="{pad_l}" y1="{h-pad_b}" x2="{w-pad_r}" y2="{h-pad_b}" stroke="#334155" stroke-width="1"/>
      <line x1="{pad_l}" y1="{pad_t}" x2="{pad_l}" y2="{h-pad_b}" stroke="#334155" stroke-width="1"/>
      <polyline points="{' '.join(w_pts)}" fill="none" stroke="#475569" stroke-width="1" stroke-dasharray="3 3"/>
      <polyline points="{' '.join(pts)}" fill="none" stroke="#a855f7" stroke-width="2"/>
      {dots}
      <text x="{lx(taus[0]):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" text-anchor="middle" font-family="monospace">{taus[0]:.0f}s</text>
      <text x="{lx(taus[-1]):.1f}" y="{h-8}" fill="#64748b" font-size="7.5" text-anchor="end" font-family="monospace">{taus[-1]:.0f}s</text>
    </svg>"""


def build_iso_svg(iso: dict) -> str:
    w, h = 740, 185
    pad_l, pad_r, pad_t, pad_b = 50, 24, 22, 28
    plot_w = w - pad_l - pad_r
    plot_h = h - pad_t - pad_b
    bins = iso.get("bins", [])
    if not bins:
        return f'<svg viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#090d16"/></svg>'

    amps = [b["amplitude_deg"] for b in bins]
    r_all = []
    for b in bins:
        r_all.extend([b["rate_s_per_day"], b.get("rate_q25", b["rate_s_per_day"]), b.get("rate_q75", b["rate_s_per_day"])])

    a_min, a_max = min(amps) - 2.0, max(amps) + 2.0
    r_min, r_max = min(r_all) - 1.5, max(r_all) + 1.5

    def ix(amp):
        return pad_l + ((amp - a_min) / max(a_max - a_min, 1.0)) * plot_w

    def iy(rate):
        return pad_t + plot_h - ((rate - r_min) / max(r_max - r_min, 1.0)) * plot_h

    bars = ""
    for b in bins:
        x = ix(b["amplitude_deg"])
        ym = iy(b["rate_s_per_day"])
        yq25 = iy(b.get("rate_q25", b["rate_s_per_day"]))
        yq75 = iy(b.get("rate_q75", b["rate_s_per_day"]))
        bars += f'<line x1="{x:.1f}" y1="{yq75:.1f}" x2="{x:.1f}" y2="{yq25:.1f}" stroke="#94a3b8" stroke-width="1.4"/>'
        bars += f'<circle cx="{x:.1f}" cy="{ym:.1f}" r="3.2" fill="#38bdf8"/>'

    fit_pts = []
    if "curve" in iso and "curve_centre_deg" in iso:
        c0, c1, c2 = iso["curve"]
        xc = iso["curve_centre_deg"]
        for i in range(41):
            amp_v = min(amps) + (i / 40.0) * (max(amps) - min(amps))
            d = amp_v - xc
            fit_pts.append(f"{ix(amp_v):.1f},{iy(c0 + c1*d + c2*d*d):.1f}")

    grid = ""
    for i in range(4):
        rv = r_min + (i + 0.5) * (r_max - r_min) / 4.0
        y = iy(rv)
        grid += f'<line x1="{pad_l}" y1="{y:.1f}" x2="{w-pad_r}" y2="{y:.1f}" stroke="#1e293b" stroke-width="1"/>'
        grid += f'<text x="{pad_l-6}" y="{y+3:.1f}" fill="#64748b" font-size="8.5" text-anchor="end" font-family="monospace">{rv:+.1f}</text>'

    xticks = "".join(
        f'<text x="{ix(b["amplitude_deg"]):.1f}" y="{h-8}" fill="#64748b" font-size="8" text-anchor="middle" font-family="monospace">{b["amplitude_deg"]:.0f}°</text>'
        for b in bins[::max(1, len(bins) // 7)]
    )

    return f"""
    <svg viewBox="0 0 {w} {h}" style="width:100%; height:auto; display:block;">
      <rect width="{w}" height="{h}" fill="#090d16" rx="4"/>
      {grid}
      <line x1="{pad_l}" y1="{h-pad_b}" x2="{w-pad_r}" y2="{h-pad_b}" stroke="#334155" stroke-width="1"/>
      {xticks}
      {bars}
      <polyline points="{' '.join(fit_pts)}" fill="none" stroke="#22c55e" stroke-width="2.4"/>
    </svg>"""


def infer_default_stages(rate_s: dict, amp_s: dict, be_s: dict, iso: dict) -> list[str]:
    slope = iso.get("s_per_day_per_deg", 0.0) or 0.0
    be_med = abs(be_s.get("median", 0.0) or 0.0)
    r_cyc = rate_s.get("cycle") or {}
    a_cyc = amp_s.get("cycle") or {}

    s1 = "Sight across the dial under a loupe while sweeping the hands through 12 hours (and sweeping chronograph hands if applicable) to verify hand-to-hand, marker, and crystal clearances."
    if abs(slope) > 0.15:
        s2 = f"Inspect regulator-pin clearance and hairspring concentricity in the movement holder (measured isochronism slope {slope:+.3f} s/d/°). Beat error is {be_med:.2f} ms."
    else:
        s2 = f"Isochronism is flat ({slope:+.3f} s/d/°) and beat error is {be_med:.2f} ms: preserve balance/hairspring stud and collet geometry; regulate via index/timing screws only."

    cyc_notes = []
    if r_cyc.get("period_s"):
        cyc_notes.append(f"rate cycle at {r_cyc['period_s']:.1f} s ({r_cyc.get('peak_to_peak', 0.0):.1f} s/d p-p)")
    if a_cyc.get("period_s"):
        cyc_notes.append(f"amplitude cycle at {a_cyc['period_s']:.1f} s ({a_cyc.get('peak_to_peak', 0.0):.1f}° p-p)")
    cyc_str = ", ".join(cyc_notes) if cyc_notes else "fourth and third/intermediate wheels"
    s3 = f"Let down mainspring power safely, remove balance and pallet fork, and perform a free-spin train check focusing on {cyc_str} and seconds-pinion friction spring tension."
    s4 = "Under the microscope, inspect wheel teeth and pinion leaves at the detected periodicities, check pallet-stone locking faces, and verify mainspring barrel-wall braking grease."
    return [s1, s2, s3, s4]


def main():
    parser = argparse.ArgumentParser(description="Generate a 4-page printable HTML watchmaker diagnostic report.")
    parser.add_argument("--steadiness", required=True, help="Path to check_steadiness output file (JSON or MCP text+JSON)")
    parser.add_argument("--watch", required=True, help="Watch name/model")
    parser.add_argument("--calibre", required=True, help="Calibre name and specs")
    parser.add_argument("--date", default=datetime.date.today().strftime("%B %d, %Y"), help="Report date")
    parser.add_argument("--position", default="Dial Up (DU)", help="Test position(s)")
    parser.add_argument("--summary", default="", help="Custom executive summary text")
    parser.add_argument("--stage1", default="", help="Stage 1 Open-Case Check (Dial & Hands)")
    parser.add_argument("--stage2", default="", help="Stage 2 Open-Case Check (Movement Running in Holder)")
    parser.add_argument("--stage3", default="", help="Stage 3 Open-Case Check (Power Let-Down & Free-Spin)")
    parser.add_argument("--stage4", default="", help="Stage 4 Open-Case Check (Disassembly & Microscope)")
    parser.add_argument("--out", required=True, help="Output HTML file path")
    args = parser.parse_args()

    data = load_steadiness_json(args.steadiness)
    rate_s = next(s for s in data["series"] if s["series"] == "rate")
    amp_s = next(s for s in data["series"] if s["series"] == "amplitude")
    be_s = next(s for s in data["series"] if s["series"] == "beat_error")
    iso = data.get("isochronism", {})

    dist_map = {d["series"] + "_" + d["values"]: d for d in data.get("distributions", {}).get("overall", [])}
    r_dist = dist_map.get("rate_readings", {})
    a_dist = dist_map.get("amplitude_readings", {})
    b_dist = dist_map.get("beat_error_readings", {})

    dur_s = data.get("duration_s", rate_s.get("readings", 0) * 10.0)
    dur_h = int(dur_s // 3600)
    dur_m = int((dur_s % 3600) // 60)
    dur_sec = int(dur_s % 60)
    dur_str = f"{dur_h} h {dur_m:02d} min {dur_sec:02d} s" if dur_h > 0 else f"{dur_m} min {dur_sec:02d} s"

    hourly_svg, fig1_cap = build_hourly_or_twostate_svg(data, rate_s, amp_s)
    cusum_svg = build_cusum_svg(rate_s)
    acf_svg = build_acf_svg(rate_s)
    allan_svg = build_allan_svg(rate_s)
    iso_svg = build_iso_svg(iso)

    def_s1, def_s2, def_s3, def_s4 = infer_default_stages(rate_s, amp_s, be_s, iso)
    s1 = args.stage1 or def_s1
    s2 = args.stage2 or def_s2
    s3 = args.stage3 or def_s3
    s4 = args.stage4 or def_s4

    r_med = r_dist.get("median", rate_s.get("median", 0.0))
    a_med = a_dist.get("median", amp_s.get("median", 0.0))
    b_med = b_dist.get("median", be_s.get("median", 0.0))
    slope = iso.get("s_per_day_per_deg", 0.0) or 0.0
    slope_se = iso.get("s_per_day_per_deg_se", 0.0) or 0.0
    best_tau = rate_s.get("allan", {}).get("best_tau_s", 0.0)
    best_adev = rate_s.get("allan", {}).get("best_deviation", 0.0)

    exec_summary = args.summary or (
        f"Across {dur_str} in {args.position}, the movement recorded a median rate of "
        f"<strong>{r_med:+.2f} s/d</strong> (middle 80%: {r_dist.get('p10', 0.0):+.2f} to {r_dist.get('p90', 0.0):+.2f} s/d), "
        f"median amplitude of <strong>{a_med:.1f}°</strong> (middle 80%: {a_dist.get('p10', 0.0):.1f}° to {a_dist.get('p90', 0.0):.1f}°), "
        f"and beat error of <strong>{b_med:.3f} ms</strong>. "
        f"Isochronism slope measured <strong>{slope:+.3f} ± {slope_se:.3f} s/d per degree</strong>, "
        f"and intrinsic Allan deviation reached <strong>{best_adev:.2f} s/d</strong> at {best_tau:.0f} s."
    )

    html_doc = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Technical Report: {html.escape(args.watch)} — {html.escape(args.calibre)}</title>
<style>
  @page {{ size: letter portrait; margin: 18mm 16mm; }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; color: #0f172a; background: #e2e8f0; line-height: 1.5; font-size: 10.2pt; }}
  .page {{ width: 215.9mm; min-height: 279.4mm; padding: 18mm 16mm; margin: 20px auto; background: #fff; box-shadow: 0 4px 15px rgba(0,0,0,0.1); display: flex; flex-direction: column; justify-content: space-between; page-break-after: always; }}
  @media print {{ body {{ background: transparent; }} .page {{ margin: 0; padding: 0; box-shadow: none; width: 100%; min-height: auto; }} }}
  .doc-eyebrow {{ font-size: 8pt; font-weight: 800; text-transform: uppercase; letter-spacing: 0.1em; color: #b91c1c; margin-bottom: 4px; }}
  .doc-title {{ font-size: 21pt; font-weight: 800; color: #0f172a; margin-bottom: 8px; }}
  .doc-meta {{ font-size: 8.5pt; color: #334155; padding-bottom: 12px; border-bottom: 1.5px solid #0f172a; margin-bottom: 16px; }}
  .callout-summary {{ background: #f8fafc; border-left: 4px solid #b91c1c; padding: 12px 14px; margin-bottom: 18px; font-size: 9.3pt; }}
  h2.section-heading {{ font-size: 12.5pt; font-weight: 800; color: #0f172a; margin: 14px 0 8px 0; }}
  table.metrics-table {{ width: 100%; border-collapse: collapse; margin-bottom: 14px; font-size: 8.8pt; }}
  table.metrics-table th {{ background: #0f172a; color: #fff; text-align: left; padding: 8px 10px; }}
  table.metrics-table td {{ padding: 8px 10px; border-bottom: 1px solid #e2e8f0; vertical-align: top; }}
  .chart-pair {{ display: grid; grid-template-columns: 1fr 1fr; gap: 14px; margin: 10px 0; }}
  .chart-box {{ border: 1px solid #1e293b; border-radius: 4px; padding: 8px; background: #090d16; }}
  .chart-caption {{ font-size: 7.9pt; color: #cbd5e1; margin-top: 6px; }}
  .rec-card {{ border: 1px solid #e2e8f0; border-radius: 6px; padding: 14px 16px; margin-top: 8px; }}
  .rec-item {{ margin-bottom: 11px; font-size: 8.9pt; }}
  .page-footer {{ border-top: 1px solid #e2e8f0; padding-top: 8px; font-size: 8pt; color: #64748b; display: flex; justify-content: space-between; }}
</style>
</head>
<body>
<div class="page">
  <div>
    <div class="doc-eyebrow">Horological Diagnostics • Bench Diagnostic Report</div>
    <div class="doc-title">Technical Report: {html.escape(args.watch)}</div>
    <div class="doc-meta">
      <strong>Watch:</strong> {html.escape(args.watch)} &nbsp;|&nbsp;
      <strong>Movement:</strong> {html.escape(args.calibre)} &nbsp;|&nbsp;
      <strong>Date:</strong> {html.escape(args.date)} &nbsp;|&nbsp;
      <strong>Run:</strong> {html.escape(args.position)}, {dur_str} ({r_dist.get('n', 0)} readings)
    </div>
    <div class="callout-summary"><strong>Executive Summary for Watchmaker:</strong> {exec_summary}</div>
    <h2 class="section-heading">1. Key Metrics ({html.escape(args.position)})</h2>
    <table class="metrics-table">
      <thead><tr><th>Parameter</th><th>Measured Value / Median</th><th>Operating Range (Middle 80%)</th><th>Headline Assessment</th></tr></thead>
      <tbody>
        <tr><td><strong>Rate</strong></td><td><strong>{r_med:+.2f} s/d</strong></td><td>{r_dist.get('p10', 0.0):+.2f} to {r_dist.get('p90', 0.0):+.2f} s/d</td><td>{html.escape(rate_s.get('headline', ''))}</td></tr>
        <tr><td><strong>Amplitude</strong></td><td><strong>{a_med:.1f}°</strong></td><td>{a_dist.get('p10', 0.0):.1f}° to {a_dist.get('p90', 0.0):.1f}°</td><td>{html.escape(amp_s.get('headline', ''))}</td></tr>
        <tr><td><strong>Beat Error</strong></td><td><strong>{b_med:.3f} ms</strong></td><td>{b_dist.get('p10', 0.0):.3f} to {b_dist.get('p90', 0.0):.3f} ms</td><td>{html.escape(be_s.get('headline', 'Steady unlock beat error'))}</td></tr>
        <tr><td><strong>Intrinsic Stability (ADEV)</strong></td><td><strong>{best_adev:.2f} s/d</strong> (at {best_tau:.0f} s)</td><td>—</td><td>Best Allan deviation across the run</td></tr>
        <tr><td><strong>Isochronism</strong></td><td><strong>{slope:+.3f} ± {slope_se:.3f} s/d/°</strong></td><td>{iso.get('amplitude_from_deg', 0.0):.0f}° to {iso.get('amplitude_to_deg', 0.0):.0f}°</td><td>{html.escape(iso.get('headline', ''))}</td></tr>
      </tbody>
    </table>
  </div>
  <div class="page-footer"><span>Timegrapher Diagnostic Report — {html.escape(args.watch)}</span><span>Page 1 of 4</span></div>
</div>

<div class="page">
  <div>
    <h2 class="section-heading">2. Time Series Progression &amp; CUSUM Control</h2>
    <div class="chart-box" style="margin-bottom:12px;">{hourly_svg}<div class="chart-caption">{fig1_cap}</div></div>
    <div class="chart-pair">
      <div class="chart-box">{cusum_svg}<div class="chart-caption"><strong>Figure 2. Rate CUSUM Control:</strong> Tracks cumulative mean stability over the recording.</div></div>
      <div class="chart-box">{acf_svg}<div class="chart-caption"><strong>Figure 3. Rate Autocorrelation (ACF):</strong> Identifies repeating wheel-turn comb periodicities.</div></div>
    </div>
  </div>
  <div class="page-footer"><span>Timegrapher Diagnostic Report — {html.escape(args.watch)}</span><span>Page 2 of 4</span></div>
</div>

<div class="page">
  <div>
    <h2 class="section-heading">3. Allan Deviation &amp; Isochronism Curve</h2>
    <div class="chart-box" style="margin-bottom:12px;">{allan_svg}<div class="chart-caption"><strong>Figure 4. Rate Allan Deviation (ADEV):</strong> Multi-interval pacing stability compared against white-noise 1/√τ decay.</div></div>
    <div class="chart-box">{iso_svg}<div class="chart-caption"><strong>Figure 5. Isochronism Curve:</strong> Median rate and interquartile spread across 2° amplitude bins.</div></div>
  </div>
  <div class="page-footer"><span>Timegrapher Diagnostic Report — {html.escape(args.watch)}</span><span>Page 3 of 4</span></div>
</div>

<div class="page">
  <div>
    <h2 class="section-heading">4. Once the Case Is Opened: 4-Stage Watchmaker Inspection Sequence</h2>
    <div class="rec-card">
      <div class="rec-item"><strong>Stage 1 — Dial Side &amp; Hands (Before Uncasing):</strong> {html.escape(s1)}</div>
      <div class="rec-item"><strong>Stage 2 — Movement Running in Holder (Balance &amp; Train Intact):</strong> {html.escape(s2)}</div>
      <div class="rec-item"><strong>Stage 3 — Power Let-Down &amp; Free-Spin Check (Balance &amp; Fork Removed):</strong> {html.escape(s3)}</div>
      <div class="rec-item"><strong>Stage 4 — Disassembly &amp; Microscope Inspection:</strong> {html.escape(s4)}</div>
    </div>
  </div>
  <div class="page-footer"><span>Timegrapher Diagnostic Report — {html.escape(args.watch)}</span><span>Page 4 of 4</span></div>
</div>
</body>
</html>"""

    with open(args.out, "w", encoding="utf-8") as f:
        f.write(html_doc)
    print(f"Generated 4-page bench diagnostic report: {args.out} ({os.path.getsize(args.out)} bytes)")


if __name__ == "__main__":
    main()
