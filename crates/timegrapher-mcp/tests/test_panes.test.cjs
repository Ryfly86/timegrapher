// The session view's Positions Test panes (#1729): one per state of wind,
// headed in core's words (a reading whose wind was not given is never shown
// as full wind, #1665), the table from core's position lines with the
// averages under it, a chart per measure each shown by its toggle, and core's
// figures as they are: the view writes none of its own.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML || path.join(__dirname, "../src/view.html"), "utf8");
function definition(name) {
  const match = html.match(new RegExp(`^function ${name}\\([^\\n]+\\}\\s*$`, "m")) || html.match(new RegExp(`^function ${name}\\([^\\n]*\\{[\\s\\S]*?^\\}`, "m"));
  if (!match) throw new Error(`Missing ${name}`);
  return match[0];
}
function constant(name) {
  const match = html.match(new RegExp(`^const ${name} = [^\\n]+$`, "m"));
  if (!match) throw new Error(`Missing ${name}`);
  return match[0].replace(/^const /, "var ");
}
const context = vm.createContext({ NO_BEAT_TITLE: "", POSITIONS: null, GROUP_NAMES: { horizontal: "Horizontal", vertical: "Vertical" },
  MINUS: "−", tile: () => "", signed: String, num: String, abs: Math.abs, limitsCard: () => "",
  CHAR_W: 6.6, CALL_OUT_W: 1.2, DASH: { dashed: "4 3" } });
vm.runInContext(["PANE_MIN_CHART", "paneShown", "PANE_KINDS", "LANE_H", "MARKS"].map(constant).join("\n"), context);
vm.runInContext(["esc", "fixedDigits", "axisLabel", "gridDown", "markOrder", "markPlace", "markPlaceRow", "markWidth", "markText", "laneAbove",
  "sessionView", "testPane", "paneToggles", "positionsPane", "positionLineChart", "valueTick"].map(definition).join("\n"), context);

const lane = (position, group, value, text, full) => ({ position, group, name: `${position} name`, value, text,
  full_wind: full == null ? null : full, full_wind_text: full == null ? null : `+${full}.0 s/d`, hover: `${position} name ${text}` });
const line = (series, title, unit, lanes, key) => ({ series, title, unit, lanes, key: key || null,
  averages: [{ of: "all", name: "Average", value: 2, text: "+2.0 s/d", hover: "Average +2.0 s/d" }],
  axis: [0, 4, 1] });
const state = (wind_h, name, key) => ({ wind_h, name,
  rate: line("rate", "Rate", "s/d", [lane("DU", "horizontal", 1, "+1.0 s/d", key && 3), lane("CU", "vertical", 3, "+3.0 s/d")], key),
  amplitude: line("amplitude", "Amplitude", "°", [lane("DU", "horizontal", 270, "270°"), lane("CU", "vertical", 260, "260°")]),
  beat_error: line("beat_error", "Beat Error", "ms", [lane("DU", "horizontal", 0.2, "0.20 ms"), lane("CU", "vertical", 0.3, "0.30 ms")]) });
const doc = { readings: [], report: {
  states: [{ wind_h: 0 }, { wind_h: 24 }, { wind_h: null }],
  position_lines: [state(0, "Full wind"), state(24, "24 h after winding", ["This test", "Full wind test"]), state(null, "Wind not given")],
} };

test("each state of wind is a pane of its own, headed in core's words", () => {
  const markup = context.sessionView(doc, null);
  const heads = [...markup.matchAll(/<h2>([^<]*)<\/h2>/g)].map((m) => m[1]);
  assert.deepEqual(heads, ["Full wind", "24 h after winding", "Wind not given"]);
});

test("the table gives core's figures for each lane, then the averages", () => {
  const markup = context.positionsPane(doc.report.position_lines[0], [], "s0", false);
  const row = markup.match(/<tr data-pos="CU">(.*?)<\/tr>/)[1];
  assert.deepEqual([...row.matchAll(/<td>([^<]*)<\/td>/g)].map((m) => m[1]), ["CU name", "+3.0 s/d", "260°", "0.30 ms"]);
  assert.ok(/<tr class="avg"><td>Average<\/td>/.test(markup), markup);
});

test("a chart per measure, each shown only while its toggle is on", () => {
  const charts = (m) => [...m.matchAll(/data-pl="s0-([a-z_]+)"/g)].map((x) => x[1]);
  assert.deepEqual(charts(context.positionsPane(doc.report.position_lines[0], [], "s0", false)), ["rate", "amplitude", "beat_error"]);
  context.paneShown.amplitude = false;
  try {
    const markup = context.positionsPane(doc.report.position_lines[0], [], "s0", false);
    assert.deepEqual(charts(markup), ["rate", "beat_error"]);
    assert.ok(markup.includes(`data-series="amplitude" aria-pressed="false"`), markup);
  } finally {
    context.paneShown.amplitude = true;
  }
});

test("a later state draws the full-wind figure hollow with an arrow, and keys it", () => {
  const chart = context.positionLineChart(doc.report.position_lines[1].rate, "s1-rate");
  assert.ok(chart.includes("This test") && chart.includes("Full wind test"), chart);
  assert.ok(chart.includes('marker-end="url(#pl-arrow-s1-rate)"'), "an arrow from the full-wind figure to this test's");
  assert.ok(chart.includes('data-val="+1.0 s/d · Full wind test +3.0 s/d"'), chart);
  assert.ok(!context.positionLineChart(doc.report.position_lines[0].rate, "s0-rate").includes("marker-end"));
});

test("lanes run under their group's name, and no position is joined to another", () => {
  const chart = context.positionLineChart(doc.report.position_lines[0].rate, "s0-rate");
  assert.ok(chart.indexOf(">Horizontal<") < chart.indexOf('data-pos="DU"'));
  assert.ok(chart.indexOf(">Vertical<") < chart.indexOf('data-pos="CU"'));
  assert.ok(!chart.includes("<path"), "no line through the dots");
});

test("a rate's ticks carry their sign and the unit follows the last", () => {
  const chart = context.positionLineChart(doc.report.position_lines[0].rate, "s0-rate");
  const ticks = [...chart.matchAll(/text-anchor="(?:middle|end)">([^<]*)<\/text>/g)].map((m) => m[1]);
  assert.deepEqual(ticks.slice(-5), ["0", "+1", "+2", "+3", "+4 s/d"]);
});
