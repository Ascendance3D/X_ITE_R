// An emulated WebXR device for the tests: `navigator.xr`, an XRSession, input sources and frames, with no headset. The
// calls are recorded so that a test can compare them with what X_ITE makes of the same device.

export class MockSession extends EventTarget {
  constructor(mode) {
    super();
    this.mode = mode;
    this.visibilityState = "visible";
    this.spaces = [];
    this.renderStates = [];
    this.endCalls = 0;
    this.listenerNames = new Set();
  }

  addEventListener(name, handler, ...rest) {
    this.listenerNames.add(name);
    super.addEventListener(name, handler, ...rest);
  }

  async requestReferenceSpace(type) {
    this.spaces.push(type);
    return { type };
  }

  updateRenderState(state) {
    this.renderStates.push(state);
  }

  async end() {
    ++this.endCalls;
  }

  fire(name, props = {}) {
    const event = Object.assign(new Event(name), props);

    this.dispatchEvent(event);
    return event;
  }
}

export class MockXR {
  constructor({ reject } = {}) {
    this.reject = reject;
    this.requests = [];
    this.session = null;
  }

  async requestSession(mode) {
    this.requests.push(mode);

    if (this.reject) {
      throw new Error(this.reject);
    }

    return (this.session = new MockSession(mode));
  }
}

/** A matrix of 16 numbers from a translation (row-major, as WebXR hands it over unchanged). */
export const translation = (x, y, z) => Float32Array.of(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1);

export const transform = (x, y, z) => ({ matrix: translation(x, y, z), inverse: { matrix: translation(-x, -y, -z) } });

export const gamepad = (axes, pressed = false, pulses = []) => ({
  axes: new Array(axes).fill(0),
  buttons: [{ pressed }],
  hapticActuators: [{ pulse: (a, b) => pulses.push([a, b]) }],
});

export const inputSource = (axes, extra = {}) => ({
  targetRaySpace: { id: Symbol("ray") },
  gamepad: axes === null ? undefined : gamepad(axes),
  ...extra,
});

/** A frame whose viewer pose has the given views, and whose input-source poses come from `rays`. */
export function frame({ viewer = [0.1, 1.6, 0], views, rays = new Map() }) {
  return {
    getViewerPose: () => ({
      transform: transform(...viewer),
      views: views.map((v) => ({
        projectionMatrix: Float32Array.of(...(v.projection ?? translation(0, 0, 0))),
        transform: transform(...v.at),
        viewport: v.viewport,
      })),
    }),
    getPose: (space) => (rays.has(space) ? { transform: transform(...rays.get(space)) } : null),
  };
}
