// WebXR session infrastructure that needs no headset (issue #83): the session life cycle, input sources, the viewer pose,
// near/far planes and the target-ray pose of the input sources, after X_ITE 16.5.0 `Browser/WebXR/X3DWebXRContext.js`
// (xrStartSession, xrStopSession, xrUpdateVisibility, xrUpdateInputSources, xrUpdateGamepads, xrUpdateNearFarPlanes,
// xrFrame, xrUpdatePose, the target-ray loop of xrUpdatePointers, xrSensorHitPulse).
//
// What it does not do: draw into the session. X_ITE draws into an `XRWebGLLayer` of its WebGL context; X_ITE_R draws with
// WebGPU, and a WebGPU layer for an immersive session (`XRGPUBinding`) is not a stable browser feature that this module
// can rely on. The embedder passes `prepare(session)` to bind whatever layer it has (X_ITE: makeXRCompatible, then an
// XRWebGLLayer) and `reshape(i, x, y, width, height)` to size the per-view framebuffer. Without them the controller still
// negotiates the session, tracks its state and computes the poses, which an emulated device (a mock `navigator.xr`) tests.
//
// Matrices are 16 numbers, row-major as X_ITE's Matrix4 holds them (a WebXR matrix is copied unchanged, as X_ITE does),
// and `multRight` is the product `this * other` as in X_ITE.

/** `a * b` into `a`. */
export function multRight(a, b) {
  for (let row = 0; row < 16; row += 4) {
    const [a0, a1, a2, a3] = [a[row], a[row + 1], a[row + 2], a[row + 3]];

    for (let c = 0; c < 4; ++c) {
      a[row + c] = a0 * b[c] + a1 * b[4 + c] + a2 * b[8 + c] + a3 * b[12 + c];
    }
  }

  return a;
}

const identity = () => Float64Array.of(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1);

/** The XR session mode of the browser option `XRSessionMode`, as X_ITE passes it to `requestSession`. */
export function sessionModeOf(optionValue) {
  return String(optionValue).toLowerCase().replaceAll("_", "-");
}

/** Runs the callbacks of one key one after the other (X_ITE's standard/Utility/Lock.js). */
class Lock {
  #tail = Promise.resolve();

  acquire(callback) {
    const run = this.#tail.then(callback, callback);

    this.#tail = run.then(() => {}, () => {});
    return run;
  }
}

export class XRSessionController {
  /**
   * @param {object} options
   * @param {object} options.xr `navigator.xr` or an emulated device (`requestSession(mode)`).
   * @param {() => string} options.getSessionMode the value of the browser option `XRSessionMode`.
   * @param {() => ({depthNear:number, depthFar:number}|null)} [options.getNearFar] the active viewpoint's planes.
   * @param {(session) => Promise<void>} [options.prepare] binds the render layer after the session is requested.
   * @param {(session) => object|null} [options.layer] the layer whose `getViewport(view)` sizes the views.
   * @param {(i:number, x:number, y:number, width:number, height:number) => void} [options.reshape]
   * @param {(gamepads: object[]) => void} [options.gamepads] the gamepads with at least four axes, to the viewer.
   * @param {(event:string, detail?:object) => void} [options.log] a diagnostic sink.
   */
  constructor(options) {
    this.options = options;
    this.lock = new Lock();
    this.session = null;
    this.referenceSpace = null;
    this.visible = false;
    this.inputSources = null;
    this.gamepads = [];
    this.pose = null;
    this.frame = null;
    this.tracking = false;
    this.listeners = [];
  }

  /** Whether a session is running. */
  get active() {
    return this.session !== null;
  }

  /** A session start that fails (no XR device, mode refused) rejects; no state is left behind. */
  start() {
    return this.lock.acquire(async () => {
      if (this.session) {
        return;
      }

      const { xr, prepare, getSessionMode } = this.options;

      if (!xr || typeof xr.requestSession !== "function") {
        throw new Error("WebXR is not available.");
      }

      const mode = sessionModeOf(getSessionMode());
      const session = await xr.requestSession(mode);
      let referenceSpace;

      try {
        referenceSpace = await session.requestReferenceSpace("local");
        await prepare?.(session);
      } catch (error) {
        await session.end?.().catch?.(() => {});
        throw error;
      }

      const on = (name, handler) => {
        session.addEventListener(name, handler);
        this.listeners.push([name, handler]);
      };

      on("visibilitychange", () => this.updateVisibility());
      on("inputsourceschange", (event) => this.updateInputSources(event));
      on("end", () => this.stop());

      this.referenceSpace = referenceSpace;
      this.visible = true;
      this.inputSources = new Set();
      this.gamepads.action = true;
      this.pose = { cameraSpaceMatrix: identity(), viewMatrix: identity(), views: [] };
      this.session = session;
      this.options.log?.("session-start", { mode });
    });
  }

  stop() {
    return this.lock.acquire(async () => {
      const session = this.session;

      if (!session) {
        return;
      }

      // The session may already have ended (the "end" event): ending it again is not an error.
      await Promise.resolve(session.end?.()).catch(() => {});

      for (const [name, handler] of this.listeners) {
        session.removeEventListener?.(name, handler);
      }

      this.listeners = [];
      this.session = null;
      this.referenceSpace = null;
      this.pose = null;
      this.visible = false;
      this.inputSources = null;
      this.frame = null;
      this.tracking = false;
      this.gamepads.length = 0;
      this.options.log?.("session-end");
    });
  }

  updateVisibility() {
    switch (this.session?.visibilityState) {
      case "visible-blurred":
      case "hidden":
        this.visible = false;
        break;
      default:
        this.visible = true;
        break;
    }
  }

  updateInputSources(event) {
    if (!this.inputSources) {
      return;
    }

    for (const inputSource of event.added ?? []) {
      Object.assign(inputSource, {
        active: false,
        matrix: identity(),
        inverse: identity(),
        hit: { id: 0, button: false, pressed: false, pulse: true, sensors: new Map() },
      });

      this.inputSources.add(inputSource);
    }

    for (const inputSource of event.removed ?? []) {
      this.inputSources.delete(inputSource);
    }

    this.updateGamepads();
  }

  updateGamepads() {
    this.gamepads.length = 0;

    for (const { gamepad } of this.inputSources ?? []) {
      // A controller without a thumbstick (fewer than four axes) does not drive navigation.
      if (gamepad && gamepad.axes.length >= 4) {
        this.gamepads.push(gamepad);
      }
    }
  }

  /** The render state's near and far planes: the active viewpoint's, else 0.1 and 10 000. */
  updateNearFar() {
    const planes = this.options.getNearFar?.() ?? { depthNear: 0.1, depthFar: 10_000 };

    this.session.updateRenderState({ depthNear: planes.depthNear, depthFar: planes.depthFar });
  }

  /** One XR frame: planes, gamepads to the viewer, the viewer pose and its views. */
  frameUpdate(frame) {
    if (!frame || !this.session) {
      return;
    }

    this.frame = frame;
    this.updateNearFar();
    this.options.gamepads?.(this.gamepads);
    this.updatePose();
  }

  updatePose() {
    const original = this.frame.getViewerPose(this.referenceSpace);
    const pose = this.pose;

    // WebXR returns null when the device cannot give a pose for this frame (tracking lost, the reference space not yet
    // located). The pinned X_ITE reads `originalPose.transform` and would throw. Here the last good pose and views stay
    // as they were, `tracking` turns false until a frame has a pose, and the frame's other updates (planes, gamepads)
    // still happen: a pose that is held, not an invented one, and the session stays valid (it ends only by the user agent's
    // `end` event or `stop`).
    if (!original) {
      this.tracking = false;
      return -1;
    }

    this.tracking = true;

    pose.cameraSpaceMatrix.set(original.transform.matrix);
    pose.viewMatrix.set(original.transform.inverse.matrix);

    const layer = this.options.layer?.(this.session);
    let i = 0;

    for (const originalView of original.views) {
      const { x, y, width, height } = layer?.getViewport(originalView) ?? { x: 0, y: 0, width: 1, height: 1 };

      // WebXR Emulator: the second view has width zero in non-stereo mode.
      if (!width) {
        continue;
      }

      this.options.reshape?.(i, x, y, width, height);

      const view = (pose.views[i] ??= {
        projectionMatrix: identity(),
        cameraSpaceMatrix: identity(),
        viewMatrix: identity(),
        matrix: identity(),
        inverse: identity(),
      });

      view.projectionMatrix.set(originalView.projectionMatrix);
      view.cameraSpaceMatrix.set(originalView.transform.matrix);
      view.viewMatrix.set(originalView.transform.inverse.matrix);
      view.matrix.set(pose.cameraSpaceMatrix);
      multRight(view.matrix, view.viewMatrix);
      view.inverse.set(view.cameraSpaceMatrix);
      multRight(view.inverse, pose.viewMatrix);
      ++i;
    }

    pose.views.length = i;
    return i;
  }

  /** The first loop of X_ITE's xrUpdatePointers: the target-ray pose of every input source (`active` when it has one). */
  updateTargetRays() {
    for (const inputSource of this.inputSources ?? []) {
      const targetRayPose = this.frame.getPose(inputSource.targetRaySpace, this.referenceSpace);

      inputSource.active = !!targetRayPose;

      if (targetRayPose) {
        inputSource.matrix.set(targetRayPose.transform.matrix);
        inputSource.inverse.set(targetRayPose.transform.inverse.matrix);
      }
    }
  }

  /** Button 0 of an active source: press once, release once (X_ITE's press and release loop). */
  updateButtons({ press, release }) {
    for (const { active, gamepad, hit } of this.inputSources ?? []) {
      if (!active) {
        continue;
      }

      if (gamepad?.buttons[0]?.pressed) {
        if (!hit.button) {
          hit.button = true;
          hit.pressed ||= !!press(hit);
        }
      } else {
        hit.button = false;

        if (hit.pressed) {
          hit.pressed = false;
          release(hit);
        }
      }
    }
  }

  /** One haptic pulse when the ray starts to hit a sensor, again only after it left (xrSensorHitPulse). */
  sensorHitPulse(hit, gamepad) {
    if (hit.sensors.size) {
      if (hit.pulse) {
        gamepad?.hapticActuators?.[0]?.pulse(0.25, 10);
        hit.pulse = false;
      }
    } else {
      hit.pulse = true;
    }
  }
}
