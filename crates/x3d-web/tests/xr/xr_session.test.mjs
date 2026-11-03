// The WebXR session infrastructure (web/xr_session.mjs) on an emulated device: no headset, no browser.
import assert from "node:assert/strict";
import test from "node:test";
import { XRSessionController, multRight, sessionModeOf } from "../../web/xr_session.mjs";
import { MockXR, frame, gamepad, inputSource, translation } from "./mock_xr.mjs";

const controller = (xr, extra = {}) =>
  new XRSessionController({ xr, getSessionMode: () => "IMMERSIVE_VR", ...extra });

test("the option value becomes a WebXR session mode", () => {
  assert.equal(sessionModeOf("IMMERSIVE_VR"), "immersive-vr");
  assert.equal(sessionModeOf("IMMERSIVE_AR"), "immersive-ar");
  assert.equal(sessionModeOf("INLINE"), "inline");
});

test("a start requests the session mode and the local reference space, and listens for three session events", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  await c.start();

  assert.deepEqual(xr.requests, ["immersive-vr"]);
  assert.deepEqual(xr.session.spaces, ["local"]);
  assert.deepEqual([...xr.session.listenerNames].sort(), ["end", "inputsourceschange", "visibilitychange"]);
  assert.equal(c.active, true);
  assert.equal(c.visible, true);
});

test("a second start while a session runs requests nothing", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  await c.start();
  await c.start();
  assert.equal(xr.requests.length, 1);
});

test("starts run one after the other", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  await Promise.all([c.start(), c.start(), c.start()]);
  assert.equal(xr.requests.length, 1);
});

test("no XR device, or a refused mode, rejects and leaves no session", async () => {
  await assert.rejects(controller(undefined).start(), /not available/);
  await assert.rejects(controller({}).start(), /not available/);

  const refused = new MockXR({ reject: "NotSupportedError" });
  const c = controller(refused);

  await assert.rejects(c.start(), /NotSupportedError/);
  assert.equal(c.active, false);
  assert.equal(c.pose, null);
});

test("a render layer that cannot be prepared ends the session and rejects", async () => {
  const xr = new MockXR();
  const c = controller(xr, { prepare: async () => { throw new Error("no layer"); } });

  await assert.rejects(c.start(), /no layer/);
  assert.equal(xr.session.endCalls, 1);
  assert.equal(c.active, false);
});

test("stop ends the session once and clears every piece of state; the session's own end event does the same", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  await c.start();
  await c.stop();
  assert.equal(xr.session.endCalls, 1);
  assert.equal(c.active, false);
  assert.equal(c.inputSources, null);
  await c.stop();
  assert.equal(xr.session.endCalls, 1);

  await c.start();
  xr.session.fire("end");
  await c.lock.acquire(async () => {});
  assert.equal(c.active, false);
});

test("stop clears every piece of state when end() throws synchronously or rejects (review of PR #366)", async () => {
  for (const fail of [
    () => { throw new Error("sync"); },
    () => Promise.reject(new Error("async")),
  ]) {
    const xr = new MockXR();
    const c = controller(xr);

    await c.start();
    xr.session.end = fail;
    await c.stop();
    assert.equal(c.active, false);
    assert.equal(c.session, null);
    assert.equal(c.listeners.length, 0);
    assert.equal(c.pose, null);
  }
});

test("updateTargetRays before the first frame, or after stop, does nothing", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  c.updateTargetRays();
  await c.start();
  c.updateTargetRays();
  await c.stop();
  c.updateTargetRays();
});

test("the visibility state decides whether the session is visible", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  await c.start();

  for (const [state, visible] of [["hidden", false], ["visible-blurred", false], ["visible", true]]) {
    xr.session.visibilityState = state;
    xr.session.fire("visibilitychange");
    assert.equal(c.visible, visible, state);
  }
});

test("input sources: added ones are tracked with a fresh state, removed ones are dropped, only gamepads with four axes count", async () => {
  const xr = new MockXR();
  const c = controller(xr);
  const [a, b, d] = [inputSource(4), inputSource(2), inputSource(null)];

  await c.start();
  xr.session.fire("inputsourceschange", { added: [a, b, d], removed: [] });
  assert.equal(c.inputSources.size, 3);
  assert.deepEqual([...c.gamepads], [a.gamepad]);
  assert.equal(a.active, false);
  assert.equal(a.hit.pulse, true);

  xr.session.fire("inputsourceschange", { added: [], removed: [a] });
  assert.equal(c.inputSources.size, 2);
  assert.deepEqual([...c.gamepads], []);
});

test("a frame sets the near and far planes, hands the gamepads over and computes the viewer pose and its views", async () => {
  const xr = new MockXR();
  const reshapes = [];
  const handed = [];
  const c = controller(xr, {
    getNearFar: () => ({ depthNear: 0.25, depthFar: 5000 }),
    gamepads: (g) => handed.push([...g]),
    layer: () => ({ getViewport: (v) => v.viewport }),
    reshape: (...a) => reshapes.push(a),
  });
  const pad = inputSource(4);

  await c.start();
  xr.session.fire("inputsourceschange", { added: [pad], removed: [] });
  c.frameUpdate(
    frame({
      viewer: [1, 2, 3],
      views: [
        { at: [-0.03, 0, 0], viewport: { x: 0, y: 0, width: 100, height: 100 } },
        { at: [0.03, 0, 0], viewport: { x: 100, y: 0, width: 100, height: 100 } },
        { at: [0, 0, 0], viewport: { x: 0, y: 0, width: 0, height: 0 } },
      ],
    }),
  );

  assert.deepEqual(xr.session.renderStates.at(-1), { depthNear: 0.25, depthFar: 5000 });
  assert.deepEqual(handed, [[pad.gamepad]]);
  assert.equal(c.pose.views.length, 2, "a view of width zero is left out");
  assert.deepEqual(reshapes, [[0, 0, 0, 100, 100], [1, 100, 0, 100, 100]]);
  assert.deepEqual([...c.pose.cameraSpaceMatrix], [...translation(1, 2, 3)]);
  assert.deepEqual([...c.pose.viewMatrix], [...translation(-1, -2, -3)]);
  // matrix = camera * view.viewMatrix: a translation by (1, 2, 3) then by (0.03, 0, 0).
  assert.deepEqual([...c.pose.views[0].matrix.slice(12, 15)].map((x) => +x.toFixed(6)), [1.03, 2, 3]);
});

test("without an active viewpoint the planes are 0.1 and 10000", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  await c.start();
  c.frameUpdate(frame({ views: [{ at: [0, 0, 0], viewport: { x: 0, y: 0, width: 1, height: 1 } }] }));
  assert.deepEqual(xr.session.renderStates.at(-1), { depthNear: 0.1, depthFar: 10000 });
});

test("a frame after the session ended, or no frame, changes nothing", async () => {
  const xr = new MockXR();
  const c = controller(xr);

  await c.start();
  c.frameUpdate(null);
  assert.equal(xr.session.renderStates.length, 0);
  await c.stop();
  c.frameUpdate(frame({ views: [] }));
  assert.equal(c.pose, null);
});

test("target rays: a source with a pose is active and has its matrices, one without is not", async () => {
  const xr = new MockXR();
  const c = controller(xr);
  const [a, b] = [inputSource(4), inputSource(4)];

  await c.start();
  xr.session.fire("inputsourceschange", { added: [a, b], removed: [] });
  c.frameUpdate(frame({ views: [], rays: new Map([[a.targetRaySpace, [0, 1, -1]]]) }));
  c.updateTargetRays();
  assert.equal(a.active, true);
  assert.deepEqual([...a.matrix.slice(12, 15)], [0, 1, -1]);
  assert.equal(b.active, false);
});

test("button 0 presses once and releases once; a press that no sensor takes is not released", async () => {
  const xr = new MockXR();
  const c = controller(xr);
  const a = inputSource(4);
  const log = [];

  await c.start();
  xr.session.fire("inputsourceschange", { added: [a], removed: [] });
  a.active = true;

  const run = () => c.updateButtons({ press: () => (log.push("press"), true), release: () => log.push("release") });

  a.gamepad.buttons[0].pressed = true;
  run();
  run();
  a.gamepad.buttons[0].pressed = false;
  run();
  run();
  assert.deepEqual(log, ["press", "release"]);
});

test("the haptic pulse fires once per sensor entry", () => {
  const c = controller(new MockXR());
  const pulses = [];
  const pad = gamepad(4, false, pulses);
  const hit = { sensors: new Map([[1, 1]]), pulse: true };

  c.sensorHitPulse(hit, pad);
  c.sensorHitPulse(hit, pad);
  assert.equal(pulses.length, 1);
  hit.sensors.clear();
  c.sensorHitPulse(hit, pad);
  hit.sensors.set(1, 1);
  c.sensorHitPulse(hit, pad);
  assert.equal(pulses.length, 2);
  assert.deepEqual(pulses[0], [0.25, 10]);
});

test("multRight is the product this * other", () => {
  const a = Float64Array.of(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
  const identity = Float64Array.of(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1);
  const shift = translation(1, 0, 0);

  assert.deepEqual([...multRight(a.slice(), identity)], [...a]);
  assert.deepEqual([...multRight(identity.slice(), a)], [...a]);
  assert.deepEqual([...multRight(a.slice(), shift)].slice(0, 4), [1 + 4, 2, 3, 4]);
});

test("emulated device: a frame without a viewer pose (tracking lost) keeps the last good pose and the session valid, then recovers", async () => {
  const xr = new MockXR();
  const handed = [];
  const c = controller(xr, { gamepads: (g) => handed.push(g.length), getNearFar: () => ({ depthNear: 1, depthFar: 2 }) });
  const good = frame({ viewer: [1, 2, 3], views: [{ at: [0, 0, 0], viewport: { x: 0, y: 0, width: 10, height: 10 } }] });
  const lost = { getViewerPose: () => null, getPose: () => null };

  await c.start();
  c.frameUpdate(good);
  assert.equal(c.tracking, true);
  const before = JSON.stringify([...c.pose.cameraSpaceMatrix]);
  const views = c.pose.views.length;

  assert.doesNotThrow(() => c.frameUpdate(lost));
  assert.equal(c.tracking, false);
  assert.equal(JSON.stringify([...c.pose.cameraSpaceMatrix]), before, "the last good pose is held");
  assert.equal(c.pose.views.length, views);
  assert.equal(c.active, true, "the session stays valid");
  assert.deepEqual(xr.session.renderStates.at(-1), { depthNear: 1, depthFar: 2 }, "the planes are still set");
  assert.equal(handed.length, 2, "the gamepads are still handed over");

  c.frameUpdate(good);
  assert.equal(c.tracking, true);
});

test("emulated device: a lost pose before any good frame leaves the identity pose and no views", async () => {
  const c = controller(new MockXR());

  await c.start();
  c.frameUpdate({ getViewerPose: () => null, getPose: () => null });
  assert.equal(c.pose.views.length, 0);
  assert.deepEqual([...c.pose.viewMatrix], [...translation(0, 0, 0)]);
});
