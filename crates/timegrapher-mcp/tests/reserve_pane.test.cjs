// The session view's Power Reserve pane (#1729): the hours table with core's
// figures and Read %, and a chart per measure of the hourly levels as steps,
// one line broken only where an hour has no figure; a thin figure grey.
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
const context = vm.createContext({ MINUS: "−" });
vm.runInContext(["PANE_MIN_CHART", "paneShown", "PANE_KINDS"].map(constant).join("\n"), context);
vm.runInContext(["esc", "fixedDigits", "roundStep", "gridFor", "axisLabel", "gridDown", "gridAcross", "valueTick",
  "testPane", "paneToggles", "reservePane", "stepChart"].map(definition).join("\n"), context);

const step = (value, text, thin) => ({ value, text, thin: !!thin });
const reserve = {
  title: "Power Reserve", hours_title: "Hours", read_title: "Read %", position: "DU",
  hours: [0, 1, 2, 3].map((hour) => ({ hour, span_h: hour === 3 ? 0.5 : 1, read: hour === 1 ? null : "90%" })),
  series: [
    { series: "rate", title: "Rate", unit: "s/d", axis: [-2, 4, 2],
      steps: [step(-1, "−1.0 s/d"), step(null, null), step(2, "+2.0 s/d", true), step(3, "+3.0 s/d")] },
    { series: "amplitude", title: "Amplitude", unit: "°", axis: [260, 300, 10],
      steps: [step(290, "290°"), step(285, "285°"), step(280, "280°"), step(270, "270°")] },
    { series: "beat_error", title: "Beat Error", unit: "ms", axis: null,
      steps: [step(null, null), step(null, null), step(null, null), step(null, null)] },
  ],
};

test("the table gives each hour core's figures and its Read %", () => {
  const markup = context.reservePane(reserve, "r");
  assert.ok(markup.includes("<h2>Power Reserve</h2>"), markup);
  const head = [...markup.match(/<thead>(.*?)<\/thead>/)[1].matchAll(/<th>([^<]*)<\/th>/g)].map((m) => m[1]);
  assert.deepEqual(head, ["Hours", "Rate", "Amplitude", "Read %"]);
  const row = (i) => [...markup.match(new RegExp(`<tr data-pos="h${i}">(.*?)</tr>`))[1].matchAll(/<td[^>]*>([^<]*)<\/td>/g)].map((m) => m[1]);
  assert.deepEqual(row(0), ["0 h", "−1.0 s/d", "290°", "90%"]);
  assert.deepEqual(row(1), ["1 h", "–", "285°", "–"]);
  assert.ok(/<td class="thin">\+2\.0 s\/d<\/td>/.test(markup), "a thin figure is grey");
});

test("each measure's hours are one line of steps, broken where an hour has none", () => {
  const chart = context.stepChart(reserve.series[0], reserve.hours, "r-rate");
  const paths = [...chart.matchAll(/<path d="([^"]*)"/g)].map((m) => m[1]);
  assert.equal(paths.length, 1, "one line");
  assert.equal((paths[0].match(/M/g) || []).length, 2, "hour 1 has no rate: the line breaks there");
  assert.ok(/V[\d.]+ H/.test(paths[0]), "hours 2 and 3 joined by a riser");
  assert.ok(!chart.includes("<rect class=\"bar"), "no bars");
});

test("a measure's toggle hides its chart, and one not measured has none", () => {
  const charts = (m) => [...m.matchAll(/data-pl="r-([a-z_]+)"/g)].map((x) => x[1]);
  assert.deepEqual(charts(context.reservePane(reserve, "r")), ["rate", "amplitude"]);
  context.paneShown.rate = false;
  try {
    assert.deepEqual(charts(context.reservePane(reserve, "r")), ["amplitude"]);
  } finally {
    context.paneShown.rate = true;
  }
});

test("the rate's ticks carry their sign, the unit after the last", () => {
  const chart = context.stepChart(reserve.series[0], reserve.hours, "r-rate");
  const ticks = [...chart.matchAll(/text-anchor="end">([^<]*)<\/text>/g)].map((m) => m[1]);
  assert.deepEqual(ticks.slice(0, 4), ["−2", "0", "+2", "+4 s/d"]);
});
