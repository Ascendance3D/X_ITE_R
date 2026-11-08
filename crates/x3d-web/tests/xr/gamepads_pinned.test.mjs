// X_ITE 16.5.0's own gamepad viewers (oracle/x_ite-pin `Browser/WebXR/xrExamineViewer.js` and `xrX3DFlyViewer.js`) run
// on a fixed synthetic frame sequence, and every viewer call they make is recorded with its arguments. The record must
// equal crates/x3d-viewer/tests/data/xr_gamepads_pinned.json, which the Rust test `xr_gamepads.rs` checks against
// `examine_gamepad_plan` and `fly_gamepad_plan`. The two numbers paths multiply in the same order, so the compare is exact.
//
// Stubbed for the pinned side only: the parent classes ExamineViewer and X3DFlyViewer (one stub module each) (their navigation math is compared
// by the navigation suites); they record the calls. Nothing of the pinned source is edited.
//
//   node --test tests/xr/gamepads_pinned.test.mjs            (compare)
//   XR_WRITE_FIXTURE=1 node --test tests/xr/gamepads_pinned.test.mjs   (write the record from pinned X_ITE)

import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { register } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

const pin = fileURLToPath(new URL("../../../../oracle/x_ite-pin/src/", import.meta.url));
const fixture = fileURLToPath(new URL("../../../x3d-viewer/tests/data/xr_gamepads_pinned.json", import.meta.url));

const stub = `export default function Viewer () { }
const record = (name) => function (...args) { globalThis .viewerCalls .push ([ name, ... args ]); };
for (const name of [ "disconnect", "zoom", "startPan", "pan", "startRotate", "rotate", "fly" ])
   Viewer .prototype [name] = record (name);`;

register(
  "data:text/javascript," +
    encodeURIComponent(`export async function resolve(s, c, next) {
      const m = /\\/Navigation\\/(ExamineViewer|X3DFlyViewer)\\.js$/.exec(s);
      if (m) return { url: "data:text/javascript," + encodeURIComponent("// " + m[1] + "\\n" + ${JSON.stringify(stub)}), shortCircuit: true };
      return next(s, c);
    }`),
  import.meta.url,
);

const { default: ExamineViewer } = await import(pathToFileURL(pin + "x_ite/Browser/Navigation/ExamineViewer.js").href);
const { default: X3DFlyViewer } = await import(pathToFileURL(pin + "x_ite/Browser/Navigation/X3DFlyViewer.js").href);
await import(pathToFileURL(pin + "x_ite/Browser/WebXR/xrExamineViewer.js").href);
await import(pathToFileURL(pin + "x_ite/Browser/WebXR/xrX3DFlyViewer.js").href);
const { default: Vector3 } = await import(pathToFileURL(pin + "standard/Math/Numbers/Vector3.js").href);

// A gamepad: [ axes 2, axes 3, button 0, button 1 ]; "NaN" stands for NaN (JSON has none).
const num = (v) => (v === "NaN" ? NaN : v);
const pad = ([x, y, b0, b1]) => ({ axes: [0, 0, num(x), num(y)], buttons: [{ pressed: b0 }, { pressed: b1 }] });

// The frames: the gamepads of each frame and the browser's currentFrameRate.
const EXAMINE = [
  { pads: [], rate: 60 },
  { pads: [], rate: 60 },
  { pads: [[0.5, 0, false, false]], rate: 60 },
  { pads: [[0.5, 0, false, false]], rate: 30 },
  { pads: [[0, -0.8, true, false]], rate: 72 },
  { pads: [[0.25, 0, true, false]], rate: 72 },
  { pads: [[0.3, 0.25, false, true]], rate: 90 },
  { pads: [["NaN", 0, false, false]], rate: 60 },
  { pads: [[0, 0, true, true]], rate: 60 },
  { pads: [[0, 0, false, false], [-0.7, 0.1, false, false]], rate: 57.3 },
  { pads: [[0.1, 0.2, true, true]], rate: 61 },
];

const FLY = [
  { pads: [] },
  { pads: [[0, -1, false, false]] },
  { pads: [[0, -1, false, false]] },
  { pads: [[0.2, -1, true, false]] },
  { pads: [[1, 0.5, false, true]] },
  { pads: [[1, 0.5, true, true]] },
  { pads: [["NaN", 0, false, false]] },
  { pads: [[0, 0.3, false, false]] },
];

function pinnedRecord() {
  const examine = Object.assign(Object.create(ExamineViewer.prototype), { rate: 60 });
  examine.getBrowser = () => ({ currentFrameRate: examine.rate });

  // X3DWebXRContext: `this [_gamepads] = [ ]`, then `Object .assign (this [_gamepads], { action: true })` at session start.
  const gamepads = Object.assign([], { action: true });
  const examineFrames = EXAMINE.map(({ pads, rate }) => {
    globalThis.viewerCalls = [];
    examine.rate = rate;
    gamepads.length = 0;
    gamepads.push(...pads.map(pad));
    examine.gamepads(gamepads);
    return { pads, rate, calls: globalThis.viewerCalls };
  });

  const fly = Object.assign(Object.create(X3DFlyViewer.prototype), { direction: new Vector3(), startTime: -1 });
  const flyFrames = FLY.map(({ pads }) => {
    globalThis.viewerCalls = [];
    fly.startTime = -1;
    fly.gamepads(pads.map(pad));
    const calls = globalThis.viewerCalls.map(([name]) => name);
    if (fly.startTime !== -1) calls.unshift("restart");
    return { pads, calls, direction: [fly.direction.x, fly.direction.y, fly.direction.z] };
  });

  return { source: "oracle/x_ite-pin 16.5.0 Browser/WebXR/xrExamineViewer.js, xrX3DFlyViewer.js", examine: examineFrames, fly: flyFrames };
}

test("pinned X_ITE's gamepad viewers make the recorded calls (#83)", () => {
  const record = pinnedRecord();

  if (process.env.XR_WRITE_FIXTURE === "1") writeFileSync(fixture, JSON.stringify(record, null, 1) + "\n");

  assert.deepEqual(record, JSON.parse(readFileSync(fixture, "utf8")));
});
