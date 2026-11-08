// Drawing a WebXR frame into the session's XRWebGLLayer (issue #83), after X_ITE 16.5.0 `Browser/WebXR/X3DWebXRContext.js`:
// xrStartSession (`await gl .makeXRCompatible ()`), xrUpdateBaseLayer (the XRWebGLLayer options and
// `session .updateRenderState ({ baseLayer })`) and xrUpdatePose (one draw per view, into `baseLayer .getViewport (view)`;
// a view of width zero is left out).
//
// X_ITE draws straight into `baseLayer .framebuffer`. X_ITE_R draws with wgpu, which owns its WebGL2 context and draws
// only into the canvas: the engine's `xrFrame` draws every view into its viewport of the canvas (sized to the layer's
// framebuffer), and `drawXRFrame` then copies the canvas into the layer's framebuffer with `blitFramebuffer`, in the same
// task, before the browser composites. This needs the engine's WebGL2 backend; a WebGPU canvas has no XR layer in a
// stable browser, and `xrFrame` refuses it.
//
// Matrices are passed as WebXR gives them (16 numbers, column-major), unchanged, as X_ITE copies them.

/** The XRWebGLLayer options of X_ITE's xrUpdateBaseLayer; `contentScale` is the browser option ContentScale. */
export function layerOptions(contentScale = 1) {
  return {
    antialias: false,
    alpha: true,
    depth: false,
    ignoreDepthValues: true,
    framebufferScaleFactor: contentScale,
  };
}

/**
 * Make the WebGL2 context of `canvas` XR-compatible, then make the session's base layer on it.
 * @param {HTMLCanvasElement} canvas the canvas the engine draws into (WebGL2 backend).
 * @param {XRSession} session
 * @param {object} [options] `contentScale`, and `XRWebGLLayer` (the constructor; the global one by default).
 * @returns {Promise<{gl: WebGL2RenderingContext, layer: XRWebGLLayer}>}
 */
export async function prepareLayer(canvas, session, { contentScale = 1, XRWebGLLayer = globalThis.XRWebGLLayer } = {}) {
  // getContext returns the context wgpu made; it gives no other context type for a canvas that has one.
  const gl = canvas.getContext("webgl2");

  if (!gl) {
    throw new Error("XR needs the engine's WebGL2 canvas (graphicsBackend webgl2)");
  }

  if (!XRWebGLLayer) {
    throw new Error("this browser has no XRWebGLLayer");
  }

  await gl.makeXRCompatible();

  const layer = new XRWebGLLayer(session, gl, layerOptions(contentScale));

  session.updateRenderState({ baseLayer: layer });

  return { gl, layer };
}

/** The pose and view numbers of `engine.xrFrame` for an XRViewerPose and its layer. */
export function poseArrays(pose, layer) {
  const views = pose.views
    .map((view) => {
      const { x, y, width, height } = layer.getViewport(view);

      return [...view.projectionMatrix, ...view.transform.inverse.matrix, x, y, width, height];
    })
    .flat();

  return {
    pose: Float64Array.from([...pose.transform.matrix, ...pose.transform.inverse.matrix]),
    views: Float64Array.from(views),
  };
}

/**
 * Draw one XR frame: the engine draws each view of `pose` into the canvas, then the canvas is copied into the layer's
 * framebuffer. `now` is the frame time in seconds, as for `engine.frame`.
 */
export function drawXRFrame(engine, gl, layer, pose, now) {
  const width = layer.framebufferWidth;
  const height = layer.framebufferHeight;
  const { pose: poseNumbers, views } = poseArrays(pose, layer);

  engine.xrFrame(now, width, height, poseNumbers, views);

  // The canvas (the default framebuffer, which wgpu presented into) into the layer. Both are GL framebuffers, y up, so
  // the rows keep their order.
  gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
  gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, layer.framebuffer);
  gl.blitFramebuffer(0, 0, width, height, 0, 0, width, height, gl.COLOR_BUFFER_BIT, gl.NEAREST);
  gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
  gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
}
