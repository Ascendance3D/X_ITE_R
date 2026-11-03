// The same emulated device drives X_ITE 16.5.0's own X3DWebXRContext (oracle/x_ite-pin, the pinned source, loaded by Node with
// the browser and WebGL objects it needs replaced by stubs) and web/xr_session.mjs, and what each does is compared:
// the requests to `navigator.xr`, the session listeners, the render state, the gamepads handed to the viewer, the viewer
// pose and view matrices, the framebuffer reshapes and the haptic pulses. Pose matrices are compared exactly: both sides
// multiply float64 numbers in the same order.
//
// Stubbed for the pinned side only: ScreenLine and ScreenPoint (they draw the input rays with WebGL), the page globals
// (`document`, `window`) and `libtess`. Nothing of the pinned source is edited.

import assert from "node:assert/strict";
import { register } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";
import { XRSessionController } from "../../web/xr_session.mjs";
import { MockXR, frame, inputSource, gamepad } from "./mock_xr.mjs";

const pin = fileURLToPath(new URL("../../../../oracle/x_ite-pin/src/", import.meta.url));

// Page globals the pinned modules touch while they are imported.
const element = () =>
  new Proxy(function () {}, {
    get: (_t, k) => (k === "classList" ? { add() {}, remove() {} } : k === "style" ? {} : k === Symbol.toPrimitive ? () => "" : element()),
    apply: () => element(),
    set: () => true,
  });

globalThis.window = globalThis;
globalThis.document = { createElement: element, createElementNS: element, head: element(), body: element(), documentElement: element(), querySelector: element(), addEventListener() {}, location: { href: "http://localhost/" } };
globalThis.location ??= { href: "http://localhost/", origin: "http://localhost", pathname: "/" };
globalThis.libtess = {
  GluTesselator: class { constructor() { return new Proxy(this, { get: (t, k) => (k in t ? t[k] : () => {}) }); } },
  gluEnum: new Proxy({}, { get: () => 0 }),
  windingRule: new Proxy({}, { get: () => 0 }),
};

register(
  "data:text/javascript," +
    encodeURIComponent(`export async function resolve(s, c, next) {
      if (/\\/Rendering\\/Screen(Line|Point)\\.js$/.test(s)) return { url: "data:text/javascript,export default class {}", shortCircuit: true };
      return next(s, c);
    }`),
  import.meta.url,
);

const { default: X3DWebXRContext } = await import(pathToFileURL(pin + "x_ite/Browser/WebXR/X3DWebXRContext.js").href);

const setNavigator = (xr) => Object.defineProperty(globalThis, "navigator", { value: { xr }, configurable: true, writable: true });

class XRWebGLLayer {
  constructor(session, gl, options) {
    this.session = session;
    this.options = options;
    this.framebuffer = { layer: true };
  }

  getViewport(view) {
    return view.viewport;
  }
}

globalThis.XRWebGLLayer = XRWebGLLayer;

/** X_ITE's browser object, reduced to what the WebXR context calls, recording what it is told. */
function pinnedBrowser({ viewpoint }) {
  const rec = { gamepads: [], reshapes: [], framebuffers: [], made: 0, mode: "IMMERSIVE_VR" };
  const browser = Object.create(X3DWebXRContext.prototype);

  X3DWebXRContext.call(browser);

  let current = window;

  Object.assign(browser, {
    rec,
    getSession: () => current,
    setSession: (s) => (current = s),
    getContext: () => ({ makeXRCompatible: async () => ++rec.made }),
    getBrowserOption: () => rec.mode,
    startAudioElements() {},
    displayFinishEvents: () => ({ addInterest() {}, removeInterest() {} }),
    removeHit() {},
    getHit: () => ({ copy: () => ({ sensors: new Map(), id: 0 }) }),
    getBrowserOptions: () => ({ checkUpdate() {} }),
    getRenderingProperties: () => ({ _ContentScale: { addInterest() {}, removeInterest() {} }, set _XRSession(v) { rec.xrSession = v; } }),
    getRenderingProperty: () => 1,
    setDefaultFramebuffer(f) { rec.defaultFramebuffer = f; },
    getActiveNavigationInfo: () => null,
    getActiveViewpoint: () => viewpoint,
    getViewer: () => ({ gamepads: (g) => rec.gamepads.push([...g]) }),
    reshapeFramebuffer: (...a) => rec.reshapes.push(a),
    getFramebuffers: () => rec.framebuffers,
    addBrowserEvent() {},
    getSurface: () => ({ querySelector: () => ({ append() {} }) }),
  });
  browser.xrAddButton();

  return browser;
}

const viewpoint = { getNearDistance: () => 0.25, getFarDistance: () => 5000 };

function views() {
  return [
    { at: [-0.03, 0, 0], viewport: { x: 0, y: 0, width: 100, height: 100 } },
    { at: [0.03, 0.01, 0], viewport: { x: 100, y: 0, width: 100, height: 100 } },
    { at: [0, 0, 0], viewport: { x: 0, y: 0, width: 0, height: 0 } },
  ];
}

/** Runs one scenario on the pinned context and on the port with equal devices; returns what each observed. */
async function both(withViewpoint) {
  // pinned
  const xrP = new MockXR();
  const pinned = pinnedBrowser({ viewpoint: withViewpoint ? viewpoint : null });

  setNavigator(xrP);
  await pinned.xrStartSession();

  const sourcesP = [inputSource(4), inputSource(2), inputSource(null)];

  xrP.session.fire("inputsourceschange", { added: sourcesP, removed: [] });
  const gamepadsAfterAdd = pinned.rec.gamepads.length;

  pinned.xrFrame(frame({ viewer: [1, 2, 3], views: views() }));
  xrP.session.fire("inputsourceschange", { added: [], removed: [sourcesP[0]] });
  pinned.xrFrame(frame({ viewer: [1, 2, 3], views: views() }));
  await pinned.xrStopSession();

  // port
  const xrQ = new MockXR();
  const reshapes = [];
  const handed = [];
  const layer = new XRWebGLLayer();
  const port = new XRSessionController({
    xr: xrQ,
    getSessionMode: () => "IMMERSIVE_VR",
    getNearFar: () => (withViewpoint ? { depthNear: 0.25, depthFar: 5000 } : null),
    prepare: async (session) => session.updateRenderState({ baseLayer: layer }),
    layer: () => layer,
    gamepads: (g) => handed.push([...g]),
    reshape: (...a) => reshapes.push(a),
  });
  const sourcesQ = [inputSource(4), inputSource(2), inputSource(null)];

  await port.start();
  xrQ.session.fire("inputsourceschange", { added: sourcesQ, removed: [] });
  port.frameUpdate(frame({ viewer: [1, 2, 3], views: views() }));
  const poseQ1 = JSON.stringify(port.pose, (_k, v) => (ArrayBuffer.isView(v) ? [...v] : v));
  xrQ.session.fire("inputsourceschange", { added: [], removed: [sourcesQ[0]] });
  port.frameUpdate(frame({ viewer: [1, 2, 3], views: views() }));
  await port.stop();

  return { xrP, xrQ, pinned, sourcesP, sourcesQ, reshapes, handed, poseQ1, gamepadsAfterAdd, layer };
}

for (const withViewpoint of [true, false]) {
  test(`session life cycle and frames match pinned X_ITE (${withViewpoint ? "a viewpoint is active" : "no viewpoint"})`, async () => {
    const r = await both(withViewpoint);
    const { xrP, xrQ } = r;

    assert.deepEqual(xrQ.requests, xrP.requests, "session mode requested");
    assert.deepEqual(xrQ.session.spaces, xrP.session.spaces, "reference spaces requested");
    assert.deepEqual([...xrQ.session.listenerNames].sort(), [...xrP.session.listenerNames].sort(), "session events listened to");
    assert.equal(xrQ.session.endCalls, xrP.session.endCalls, "session.end calls");

    // Render state: the base layer first, then the planes of every frame.
    const shape = (states) => states.map((s) => ("baseLayer" in s ? "baseLayer" : JSON.stringify(s)));

    assert.deepEqual(shape(xrQ.session.renderStates), shape(xrP.session.renderStates), "render state updates");
    assert.equal(r.pinned.rec.made, 1, "the pinned side made the context XR compatible");
  });

  test(`gamepads, reshapes and the pose match pinned X_ITE (${withViewpoint ? "viewpoint" : "no viewpoint"})`, async () => {
    const r = await both(withViewpoint);
    const idx = (list, sources) => list.map((g) => sources.findIndex((s) => s.gamepad === g));

    // Gamepads handed to the viewer at each frame: the sources with four axes that are still present.
    assert.deepEqual(
      r.handed.map((g) => idx(g, r.sourcesQ)),
      r.pinned.rec.gamepads.map((g) => idx(g, r.sourcesP)),
    );
    assert.deepEqual(r.handed.map((g) => idx(g, r.sourcesQ)), [[0], []]);
    assert.deepEqual(r.reshapes, r.pinned.rec.reshapes, "framebuffer reshapes (zero-width view left out)");
  });
}

test("viewer pose and view matrices equal pinned X_ITE's, number for number", async () => {
  const xrP = new MockXR();
  const pinned = pinnedBrowser({ viewpoint: null });

  setNavigator(xrP);
  await pinned.xrStartSession();
  pinned.xrFrame(frame({ viewer: [1, 2, 3], views: views() }));
  const pose = pinned.getPose();

  const xrQ = new MockXR();
  const layer = new XRWebGLLayer();
  const port = new XRSessionController({ xr: xrQ, getSessionMode: () => "IMMERSIVE_VR", layer: () => layer });

  await port.start();
  port.frameUpdate(frame({ viewer: [1, 2, 3], views: views() }));

  const same = (a, b, what) => assert.deepEqual([...a], [...b], what);

  same(port.pose.cameraSpaceMatrix, pose.cameraSpaceMatrix, "cameraSpaceMatrix");
  same(port.pose.viewMatrix, pose.viewMatrix, "viewMatrix");
  assert.equal(port.pose.views.length, pose.views.length);

  for (const [i, v] of pose.views.entries()) {
    for (const k of ["projectionMatrix", "cameraSpaceMatrix", "viewMatrix", "matrix", "inverse"]) {
      same(port.pose.views[i][k], v[k], `view ${i} ${k}`);
    }
  }

  assert.ok(pose.views[0].matrix[12] !== 0 || pose.views[0].matrix[13] !== 0, "the views are not trivial");
});

test("a start that finds the session already running does nothing on both sides", async () => {
  const xrP = new MockXR();
  const pinned = pinnedBrowser({ viewpoint: null });

  setNavigator(xrP);
  await pinned.xrStartSession();
  await pinned.xrStartSession();

  const xrQ = new MockXR();
  const port = new XRSessionController({ xr: xrQ, getSessionMode: () => "IMMERSIVE_VR" });

  await port.start();
  await port.start();
  assert.equal(xrQ.requests.length, xrP.requests.length);
});

test("the haptic pulse rule equals pinned X_ITE's", () => {
  const pinned = pinnedBrowser({ viewpoint: null });
  const port = new XRSessionController({ xr: new MockXR(), getSessionMode: () => "INLINE" });
  const run = (call) => {
    const pulses = [];
    const pad = gamepad(4, false, pulses);
    const hit = { sensors: new Map(), pulse: true };
    const trace = [];

    for (const entering of [true, true, false, true, false, false, true]) {
      entering ? hit.sensors.set(1, 1) : hit.sensors.clear();
      call(hit, pad);
      trace.push([pulses.length, hit.pulse]);
    }

    return trace;
  };

  assert.deepEqual(
    run((h, p) => port.sensorHitPulse(h, p)),
    run((h, p) => pinned.xrSensorHitPulse(h, p)),
  );
});
