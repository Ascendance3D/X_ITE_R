//! Page-side JavaScript of oracle/tools/x_ite_particles.mjs (R06), x_ite_particles_events.mjs (EV) and
//! x_ite_particles_b07.mjs (B07), byte for byte. Generated from those files; the unit test
//! `particles_js_is_verbatim` checks every constant still occurs in its source.

pub const INIT_R06: &str = r###"([ s, date0 ]) =>
{
   // Seeded Math.random (mulberry32), reseedable.
   let a = s >>> 0;
   window .__reseed = v => { a = v >>> 0; };
   Math .random = () =>
   {
      a = (a + 0x6D2B79F5) >>> 0;
      let t = a;
      t = Math .imul (t ^ (t >>> 15), t | 1);
      t ^= t + Math .imul (t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
   };
   // X_ITE's ResizeObserver calls browser.reshape () -> a full traverse (and an extra animateParticles)
   // at wall-clock-dependent moments. Make it inert; the canvas size is fixed.
   window .ResizeObserver = class { observe () { } unobserve () { } disconnect () { } };
   // Pin Date.now until anim.html installs its virtual clock.
   Date .now = () => date0;
   // Record every value handed to a randomSeed uniform.
   window .__seedLocs = new Set ();
   window .__seeds = [ ];
   const u1i = WebGL2RenderingContext .prototype .uniform1i;
   WebGL2RenderingContext .prototype .uniform1i = function (loc, v)
   {
      if (loc && window .__seedLocs .has (loc))
         window .__seeds .push (v);
      return u1i .call (this, loc, v);
   };
}"###;

pub const NORMALIZE_R06: &str = r###"([name, s, t0]) =>
   {
      const browser = document .getElementById ("c") .browser;
      const ps = browser .currentScene .getNamedNode (name) .getValue () .getInnerNode ();
      const out = { deltaTimeBeforeReset: ps .deltaTime, numParticlesAfterLoad: ps .numParticles, creationTimeAfterLoad: ps .creationTime };
      // Back to the state before the first animateParticles: nothing alive, zeroed buffers, deltaTime 0.
      // creationTime is Date.now () at set_maxParticles for ordinary emitters (= t0, pinned) but stays 0
      // for explosive ones, whose first burst therefore happens at the first animate call.
      const gl = browser .getContext (), explosive = ps .emitterNode .isExplosive ();
      out .explosive = explosive;
      ps .deltaTime    = 0;
      ps .numParticles = 0;
      ps .creationTime = explosive ? 0 : t0;
      for (const buf of [ ps .inputParticles, ps .outputParticles ])
      {
         gl .bindBuffer (gl .ARRAY_BUFFER, buf);
         gl .bufferData (gl .ARRAY_BUFFER, new Uint8Array (ps .maxParticles * ps .particlesStride), gl .DYNAMIC_DRAW);
      }
      window .__reseed (s);
      window .__seeds .length = 0;
      window .__ps = ps;
      window .__calls = 0;
      window .__stack = [ ]; window .__debug = false;
      const en = ps .emitterNode, orig = en .animate;
      en .animate = function (... a) { ++ window .__calls; if (window .__debug) window .__stack .push (new Error () .stack .split ("\n") .slice (1, 16) .join (" | ")); return orig .apply (this, a); };
      return out;
   }"###;

pub const STEP_R06: &str = r###"([tt, name]) =>
      {
         const ps = window .__ps;
         for (const p of ps .emitterNode .programs .values ()) window .__seedLocs .add (p .randomSeed);
         window .__seeds .length = 0;
         const before = window .__calls;
         window .setDate (tt * 1000);
         window .step (tt);
         return { calls: window .__calls - before, randomSeeds: window .__seeds .slice (), numParticles: ps .numParticles, deltaTime: ps .deltaTime,
            creationTime: ps .creationTime, createParticles: !! ps .createParticles };
      }"###;

pub const DUMP_R06: &str = r###"born =>
         {
            const ps = window .__ps, browser = document .getElementById ("c") .browser, gl = browser .getContext ();
            const n = ps .numParticles, data = new Float32Array (n * 28);
            const prev = gl .getParameter (gl .ARRAY_BUFFER_BINDING);
            gl .bindBuffer (gl .ARRAY_BUFFER, ps .outputParticles);
            if (n) gl .getBufferSubData (gl .ARRAY_BUFFER, 0, data);
            gl .bindBuffer (gl .ARRAY_BUFFER, prev);
            let out = data, count = n;
            if (born)
            {
               // Keep the records created this frame (elapsedTime === 0), in buffer order.
               const keep = [ ];
               for (let i = 0; i < n; ++ i) if (data [i * 28 + 2] === 0) keep .push (i);
               out = new Float32Array (keep .length * 28);
               keep .forEach ((i, j) => out .set (data .subarray (i * 28, i * 28 + 28), j * 28));
               count = keep .length;
            }
            const bytes = new Uint8Array (out .buffer);
            let bin = "";
            for (let i = 0; i < bytes .length; i += 0x8000) bin += String .fromCharCode .apply (null, bytes .subarray (i, i + 0x8000));
            return { n: count, total: n, b64: btoa (bin) };
         }"###;

pub const GEOMETRY_R06: &str = r###"() =>
   {
      const ps = window .__ps, en = ps .emitterNode, arr = a => a ? Array .from (a) : null, g = { };
      const tex = (name, array, rows) => { g [name] = { array: arr (array) }; };
      if (en .surfaceArray) g .surface = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, array: arr (en .surfaceArray) };
      if (en .volumeArray)  g .volume  = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, hierarchyIndex: en .hierarchyIndex, hierarchyRoot: en .hierarchyRoot, array: arr (en .volumeArray) };
      if (ps .boundedHierarchyRoot > -1) g .bounded = { vertices: arr (ps .boundedVertices), normals: arr (ps .boundedNormals), hierarchyIndex: ps .boundedHierarchyIndex, hierarchyRoot: ps .boundedHierarchyRoot };
      g .numForces = ps .numForces;
      return g;
   }"###;

pub const INFO_R06: &str = r###"() =>
   {
      const ps = window .__ps;
      return { geometryType: ps .geometryType, maxParticles: ps .maxParticles, particleLifetime: ps .particleLifetime,
         lifetimeVariation: ps .lifetimeVariation, emitter: ps .emitterNode .getTypeName (), numForces: ps .numForces,
         boundedHierarchyRoot: ps .boundedHierarchyRoot, stride: ps .particlesStride / 4 };
   }"###;

pub const INIT_EV: &str = r###"([ s, date0 ]) =>
{
   // Seeded Math.random (mulberry32), reseedable.
   let a = s >>> 0;
   window .__reseed = v => { a = v >>> 0; };
   Math .random = () =>
   {
      a = (a + 0x6D2B79F5) >>> 0;
      let t = a;
      t = Math .imul (t ^ (t >>> 15), t | 1);
      t ^= t + Math .imul (t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
   };
   // X_ITE's ResizeObserver calls browser.reshape () -> a full traverse (and an extra animateParticles)
   // at wall-clock-dependent moments. Make it inert; the canvas size is fixed.
   window .ResizeObserver = class { observe () { } unobserve () { } disconnect () { } };
   // Pin Date.now until anim.html installs its virtual clock.
   Date .now = () => date0;
   // Record every value handed to a randomSeed uniform.
   window .__seedLocs = new Set ();
   window .__seeds = [ ];
   // A new program (an emitter toggled on / off builds another one) registers its randomSeed location at once.
   const gul = WebGL2RenderingContext .prototype .getUniformLocation;
   WebGL2RenderingContext .prototype .getUniformLocation = function (program, name)
   {
      const loc = gul .call (this, program, name);
      if (name === "randomSeed" && loc) window .__seedLocs .add (loc);
      return loc;
   };
   const u1i = WebGL2RenderingContext .prototype .uniform1i;
   WebGL2RenderingContext .prototype .uniform1i = function (loc, v)
   {
      if (loc && window .__seedLocs .has (loc))
         window .__seeds .push (v);
      return u1i .call (this, loc, v);
   };
}"###;

pub const NORMALIZE_EV: &str = r###"([name, s, t0]) =>
   {
      const browser = document .getElementById ("c") .browser;
      const ps = browser .currentScene .getNamedNode (name) .getValue () .getInnerNode ();
      const out = { deltaTimeBeforeReset: ps .deltaTime, numParticlesAfterLoad: ps .numParticles, creationTimeAfterLoad: ps .creationTime };
      // Back to the state before the first animateParticles: nothing alive, zeroed buffers, deltaTime 0.
      // creationTime is Date.now () at set_maxParticles for ordinary emitters (= t0, pinned) but stays 0
      // for explosive ones, whose first burst therefore happens at the first animate call.
      const gl = browser .getContext (), explosive = ps .emitterNode .isExplosive ();
      out .explosive = explosive;
      ps .deltaTime    = 0;
      ps .numParticles = 0;
      ps .creationTime = explosive ? 0 : t0;
      for (const buf of [ ps .inputParticles, ps .outputParticles ])
      {
         gl .bindBuffer (gl .ARRAY_BUFFER, buf);
         gl .bufferData (gl .ARRAY_BUFFER, new Uint8Array (ps .maxParticles * ps .particlesStride), gl .DYNAMIC_DRAW);
      }
      window .__reseed (s);
      window .__seeds .length = 0;
      window .__ps = ps;
      window .__calls = 0;
      window .__stack = [ ]; window .__debug = false;
      const en = ps .emitterNode, orig = en .animate;
      en .animate = function (... a) { ++ window .__calls; if (window .__debug) window .__stack .push (new Error () .stack .split ("\n") .slice (1, 16) .join (" | ")); return orig .apply (this, a); };
      return out;
   }"###;

pub const STEP_EV: &str = r###"([tt, name, events]) =>
      {
         const ps = window .__ps;
         const browser = document .getElementById ("c") .browser;
         window .__seeds .length = 0;
         const before = window .__calls;
         window .setDate (tt * 1000);

         for (const e of events)
            browser .currentScene .getNamedNode (e .node) .getValue () .getInnerNode () .getField (e .field) .setValue (e .value);

         window .step (tt);
         return { calls: window .__calls - before, randomSeeds: window .__seeds .slice (), numParticles: ps .numParticles, deltaTime: ps .deltaTime,
            creationTime: ps .creationTime, createParticles: !! ps .createParticles, isActive: ps ._isActive .getValue (), maxParticles: ps .maxParticles };
      }"###;

pub const DUMP_EV: &str = r###"born =>
         {
            const ps = window .__ps, browser = document .getElementById ("c") .browser, gl = browser .getContext ();
            const n = ps .numParticles, data = new Float32Array (n * 28);
            const prev = gl .getParameter (gl .ARRAY_BUFFER_BINDING);
            gl .bindBuffer (gl .ARRAY_BUFFER, ps .outputParticles);
            if (n) gl .getBufferSubData (gl .ARRAY_BUFFER, 0, data);
            gl .bindBuffer (gl .ARRAY_BUFFER, prev);
            let out = data, count = n;
            if (born)
            {
               // Keep the records created this frame (elapsedTime === 0), in buffer order.
               const keep = [ ];
               for (let i = 0; i < n; ++ i) if (data [i * 28 + 2] === 0) keep .push (i);
               out = new Float32Array (keep .length * 28);
               keep .forEach ((i, j) => out .set (data .subarray (i * 28, i * 28 + 28), j * 28));
               count = keep .length;
            }
            const bytes = new Uint8Array (out .buffer);
            let bin = "";
            for (let i = 0; i < bytes .length; i += 0x8000) bin += String .fromCharCode .apply (null, bytes .subarray (i, i + 0x8000));
            return { n: count, total: n, b64: btoa (bin) };
         }"###;

pub const GEOMETRY_EV: &str = r###"() =>
   {
      const ps = window .__ps, en = ps .emitterNode, arr = a => a ? Array .from (a) : null, g = { };
      const tex = (name, array, rows) => { g [name] = { array: arr (array) }; };
      if (en .surfaceArray) g .surface = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, array: arr (en .surfaceArray) };
      if (en .volumeArray)  g .volume  = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, hierarchyIndex: en .hierarchyIndex, hierarchyRoot: en .hierarchyRoot, array: arr (en .volumeArray) };
      if (ps .boundedHierarchyRoot > -1) g .bounded = { vertices: arr (ps .boundedVertices), normals: arr (ps .boundedNormals), hierarchyIndex: ps .boundedHierarchyIndex, hierarchyRoot: ps .boundedHierarchyRoot };
      g .numForces = ps .numForces;
      return g;
   }"###;

pub const INFO_EV: &str = r###"() =>
   {
      const ps = window .__ps;
      return { geometryType: ps .geometryType, maxParticles: ps .maxParticles, particleLifetime: ps .particleLifetime,
         lifetimeVariation: ps .lifetimeVariation, emitter: ps .emitterNode .getTypeName (), numForces: ps .numForces,
         boundedHierarchyRoot: ps .boundedHierarchyRoot, stride: ps .particlesStride / 4 };
   }"###;

pub const INIT_B07: &str = r###"([ s, date0 ]) =>
{
   // Seeded Math.random (mulberry32), reseedable.
   let a = s >>> 0;
   window .__reseed = v => { a = v >>> 0; };
   Math .random = () =>
   {
      a = (a + 0x6D2B79F5) >>> 0;
      let t = a;
      t = Math .imul (t ^ (t >>> 15), t | 1);
      t ^= t + Math .imul (t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
   };
   // Record every Math.random value (the force models draw on the CPU).
   window .__draws = [ ];
   window .__forceDraws = [ ];
   const baseRandom = Math .random;
   Math .random = () => { const v = baseRandom (); window .__draws .push (v); return v; };
   // Wrap addForce on the prototype of every force model of the system, once per prototype: covers instances added
   // later (physics changed at run time), whose first addForce happens inside the step.
   window .__wrapProto = proto =>
   {
      if (Object .prototype .hasOwnProperty .call (proto, "__b07wrapped")) return;
      proto .__b07wrapped = true;
      const orig = proto .addForce;
      proto .addForce = function (... a)
      {
         const s0 = window .__draws .length, r = orig .apply (this, a);
         for (const v of window .__draws .slice (s0)) window .__forceDraws .push (v);
         return r;
      };
   };
   // X_ITE's ResizeObserver calls browser.reshape () -> a full traverse (and an extra animateParticles)
   // at wall-clock-dependent moments. Make it inert; the canvas size is fixed.
   window .ResizeObserver = class { observe () { } unobserve () { } disconnect () { } };
   // Pin Date.now until anim.html installs its virtual clock.
   Date .now = () => date0;
   // Record every value handed to a randomSeed uniform.
   window .__seedLocs = new Set ();
   window .__seeds = [ ];
   // A new program (an emitter toggled on / off builds another one) registers its randomSeed location at once.
   const gul = WebGL2RenderingContext .prototype .getUniformLocation;
   WebGL2RenderingContext .prototype .getUniformLocation = function (program, name)
   {
      const loc = gul .call (this, program, name);
      if (name === "randomSeed" && loc) window .__seedLocs .add (loc);
      return loc;
   };
   const u1i = WebGL2RenderingContext .prototype .uniform1i;
   WebGL2RenderingContext .prototype .uniform1i = function (loc, v)
   {
      if (loc && window .__seedLocs .has (loc))
         window .__seeds .push (v);
      return u1i .call (this, loc, v);
   };
}"###;

pub const NORMALIZE_B07: &str = r###"([name, s, t0]) =>
   {
      const browser = document .getElementById ("c") .browser;
      const ps = browser .currentScene .getNamedNode (name) .getValue () .getInnerNode ();
      const out = { deltaTimeBeforeReset: ps .deltaTime, numParticlesAfterLoad: ps .numParticles, creationTimeAfterLoad: ps .creationTime };
      // Back to the state before the first animateParticles: nothing alive, zeroed buffers, deltaTime 0.
      // creationTime is Date.now () at set_maxParticles for ordinary emitters (= t0, pinned) but stays 0
      // for explosive ones, whose first burst therefore happens at the first animate call.
      const gl = browser .getContext (), explosive = ps .emitterNode .isExplosive ();
      out .explosive = explosive;
      ps .deltaTime    = 0;
      ps .numParticles = 0;
      ps .creationTime = explosive ? 0 : t0;
      for (const buf of [ ps .inputParticles, ps .outputParticles ])
      {
         gl .bindBuffer (gl .ARRAY_BUFFER, buf);
         gl .bufferData (gl .ARRAY_BUFFER, new Uint8Array (ps .maxParticles * ps .particlesStride), gl .DYNAMIC_DRAW);
      }
      window .__reseed (s);
      window .__seeds .length = 0;
      window .__ps = ps;
      for (const type of [ "ForcePhysicsModel", "WindPhysicsModel" ])
         window .__wrapProto (Object .getPrototypeOf (browser .currentScene .createNode (type) .getValue () .getInnerNode ()));
      window .__calls = 0;
      window .__stack = [ ]; window .__debug = false;
      const en = ps .emitterNode, orig = en .animate;
      en .animate = function (... a) { ++ window .__calls; if (window .__debug) window .__stack .push (new Error () .stack .split ("\n") .slice (1, 16) .join (" | ")); return orig .apply (this, a); };
      return out;
   }"###;

pub const STEP_B07: &str = r###"([tt, name, events]) =>
      {
         const ps = window .__ps;
         const browser = document .getElementById ("c") .browser;
         window .__seeds .length = 0;
         const before = window .__calls;
         window .setDate (tt * 1000);

         for (const e of events)
         {
            const scene = browser .currentScene;
            const field = scene .getNamedNode (e .node) .getValue () .getInnerNode () .getField (e .field);
            const v = e .value;

            if (v !== null && typeof v === "object" && !Array .isArray (v))
            {
               if ("def" in v)
                  field .setValue (v .def === null ? null : scene .getNamedNode (v .def) .getValue ());
               else if ("nodes" in v)
                  field .setValue (v .nodes .map (n => scene .getNamedNode (n) .getValue ()));
               else
                  throw new Error ("unsupported event value " + JSON .stringify (v));
            }
            else if (Array .isArray (v))
               field .fromString (field .getTypeName () .startsWith ("MF") ? "[ " + v .join (" ") + " ]" : v .join (" "), scene);
            else
               field .setValue (v);

            (window .__eventLog ??= [ ]) .push (e .node + "." + e .field + " = " + field .toString ());
         }

         window .__draws .length = 0;
         window .__forceDraws .length = 0;

         window .step (tt);
         return { calls: window .__calls - before, randomSeeds: window .__seeds .slice (), numParticles: ps .numParticles, deltaTime: ps .deltaTime,
            creationTime: ps .creationTime, createParticles: !! ps .createParticles, isActive: ps ._isActive .getValue (), maxParticles: ps .maxParticles,
            forceDraws: window .__forceDraws .slice (), numForces: ps .numForces, forces: Array .from (ps .forces .subarray (0, ps .numForces * 4)), eventLog: window .__eventLog ?.splice (0) ?? [ ] };
      }"###;

pub const DUMP_B07: &str = r###"born =>
         {
            const ps = window .__ps, browser = document .getElementById ("c") .browser, gl = browser .getContext ();
            const n = ps .numParticles, data = new Float32Array (n * 28);
            const prev = gl .getParameter (gl .ARRAY_BUFFER_BINDING);
            gl .bindBuffer (gl .ARRAY_BUFFER, ps .outputParticles);
            if (n) gl .getBufferSubData (gl .ARRAY_BUFFER, 0, data);
            gl .bindBuffer (gl .ARRAY_BUFFER, prev);
            let out = data, count = n;
            if (born)
            {
               // Keep the records created this frame (elapsedTime === 0), in buffer order.
               const keep = [ ];
               for (let i = 0; i < n; ++ i) if (data [i * 28 + 2] === 0) keep .push (i);
               out = new Float32Array (keep .length * 28);
               keep .forEach ((i, j) => out .set (data .subarray (i * 28, i * 28 + 28), j * 28));
               count = keep .length;
            }
            const bytes = new Uint8Array (out .buffer);
            let bin = "";
            for (let i = 0; i < bytes .length; i += 0x8000) bin += String .fromCharCode .apply (null, bytes .subarray (i, i + 0x8000));
            return { n: count, total: n, b64: btoa (bin) };
         }"###;

pub const GEOMETRY_B07: &str = r###"() =>
   {
      const ps = window .__ps, en = ps .emitterNode, arr = a => a ? Array .from (a) : null, g = { };
      const tex = (name, array, rows) => { g [name] = { array: arr (array) }; };
      if (en .surfaceArray) g .surface = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, array: arr (en .surfaceArray) };
      if (en .volumeArray)  g .volume  = { verticesIndex: en .verticesIndex, normalsIndex: en .normalsIndex, hierarchyIndex: en .hierarchyIndex, hierarchyRoot: en .hierarchyRoot, array: arr (en .volumeArray) };
      if (ps .boundedHierarchyRoot > -1) g .bounded = { vertices: arr (ps .boundedVertices), normals: arr (ps .boundedNormals), hierarchyIndex: ps .boundedHierarchyIndex, hierarchyRoot: ps .boundedHierarchyRoot };
      if (en .polylinesArray)
      {
         const n = en .verticesIndex < 0 ? 0 : en .polylinesNode .getVertices () .length / 4;
         g .polyline = { verticesIndex: en .verticesIndex, numVertices: n, array: arr (en .polylinesArray .subarray (0, (n ? en .verticesIndex + n : 0) * 4)) };
      }
      g .numForces = ps .numForces;
      return g;
   }"###;

pub const INFO_B07: &str = r###"() =>
   {
      const ps = window .__ps;
      return { geometryType: ps .geometryType, maxParticles: ps .maxParticles, particleLifetime: ps .particleLifetime,
         lifetimeVariation: ps .lifetimeVariation, emitter: ps .emitterNode .getTypeName (), numForces: ps .numForces,
         boundedHierarchyRoot: ps .boundedHierarchyRoot, stride: ps .particlesStride / 4 };
   }"###;

pub const GET_LOG_OPT: &str = r###"() => window .getLog ?.() ?? [ ]"###;

pub const CALLS: &str = r###"() => window .__calls"###;

/// Additive (not in the Node tools): script key `pickProbes` ({ frames, points }). After each listed frame,
/// `browser .touch (x, y)` picks every point into the pointing buffer, like anim's PICK_PROBES, and also records the
/// DEF name of the hit shape node (a ParticleSystem or a Shape; "" when unnamed).
pub const PICK_PROBES_NAMED: &str = r###"points =>
      {
         const browser = document .getElementById ("c") .browser;

         return points .map (([ x, y ]) =>
         {
            if (!browser .touch (x, y))
               return { id: 0 };

            const h = browser .getHit ();

            return { id: 1, shape: h .shapeNode .getName (), point: [ h .point .x, h .point .y, h .point .z ],
                     normal: [ h .normal .x, h .normal .y, h .normal .z ], texCoord: [ h .texCoord .x, h .texCoord .y, h .texCoord .z, h .texCoord .w ] };
         });
      }"###;
