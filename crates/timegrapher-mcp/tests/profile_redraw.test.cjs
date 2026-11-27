// The Tick and Tock profile drawn from a compact result, whose curves ride
// in _meta (views.rs profile_charts, #2097), is the profile drawn from the
// CLI's whole document. Runs the full embedded script with host doubles.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");
const { setImmediate: flush } = require("node:timers/promises");

const html = fs.readFileSync(process.env.TIMEGRAPHER_TEST_VIEW_HTML ||
  path.join(__dirname, "../src/view.html"), "utf8");
const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)];
assert.equal(scripts.length, 1, "execute the entire embedded script");

async function drawn(toolResult) {
  const messages = [];
  const listeners = new Map();
  const root = { innerHTML: "", clientWidth: 640, querySelectorAll: () => [] };
  const parent = { postMessage(m) { messages.push(structuredClone(m)); } };
  const window = { parent, matchMedia: () => ({ matches: false }),
    addEventListener(name, callback) { listeners.set(name, callback); } };
  const context = vm.createContext({ window,
    document: { documentElement: { dataset: {}, scrollWidth: 640, scrollHeight: 200 }, body: {},
      getElementById: () => root },
    ResizeObserver: class { observe() {} },
    setTimeout() { return 0; }, clearTimeout() {},
  });
  vm.runInContext(scripts[0][1], context, { filename: "embedded-view.js" });
  const send = (data) => listeners.get("message")({ source: parent, data: structuredClone(data) });
  const init = messages.find((m) => m.method === "ui/initialize");
  send({ jsonrpc: "2.0", id: init.id, result: { protocolVersion: "2026-01-26",
    hostInfo: { name: "test-host", version: "1" }, hostCapabilities: {}, hostContext: { theme: "light" } } });
  await flush();
  send({ jsonrpc: "2.0", method: "ui/notifications/tool-result", params: toolResult });
  await flush();
  return root.innerHTML;
}

const curves = (k) => ({ median: [0.1, 0.5 + k, 0.2], p10: [0.05, 0.3, 0.1], p90: [0.2, 0.9, 0.3] });
const marks = (beats) => ({ beats, t0_ms: -20, step_ms: 0.1, floor: 0.01, unlock_ms: -8.7,
  drop_ms: -0.6, peak_ms: 0, sound1_ms: null, sound2_ms: -5.6, sound3_ms: -0.6, amplitude_deg: 268, side: "a" });
const whole = { schema: "timegrapher.profile/1", input: { files: [{ path: "/x/take.flac" }] },
  a: { ...marks(120), ...curves(0) }, b: { ...marks(119), ...curves(0.1) } };
const compact = { schema: "timegrapher.profile/1", input: whole.input, a: marks(120), b: marks(119),
  projection: { schema: "timegrapher.mcp.profile_compact/1" } };
const meta = { "timegrapher/charts": { curves: { a: curves(0), b: curves(0.1) } } };

test("a compact profile draws its curves from _meta, as the whole document draws", async () => {
  const fromWhole = await drawn({ structuredContent: whole, content: [] });
  const fromMeta = await drawn({ structuredContent: compact, content: [], _meta: meta });
  const bare = await drawn({ structuredContent: compact, content: [] });
  assert.match(fromWhole, /Tick and Tock/);
  assert.notEqual(fromWhole, bare, "the curves draw something");
  assert.equal(fromMeta.replace(/compact/g, ""), fromWhole.replace(/compact/g, ""));
});
