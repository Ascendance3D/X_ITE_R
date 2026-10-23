//! Page-side JavaScript of oracle/x_ite-anim.mjs, byte for byte (the `page.evaluate` / `page.addInitScript`
//! bodies). Generated from that file; the unit test `page_js_is_verbatim` checks every constant still occurs
//! in oracle/x_ite-anim.mjs (and the MSAA / sized variants use the same text).

pub const AUDIO_INIT: &str = r###"a =>
   {
      let context = null;

      // The page's resume () resolves and does nothing, like resume () of a running AudioContext:
      // X3DSoundContext calls it again on every startAudioElement while the first call has not
      // resolved, and on an OfflineAudioContext that would restart rendering in the middle of a
      // frame. Only __audioResume resumes rendering.
      let resume = null;

      const make = () => context ??= (() =>
      {
         const ctx = new OfflineAudioContext ({ numberOfChannels: a .channels, length: a .length, sampleRate: a .sampleRate });

         resume = ctx .resume .bind (ctx);
         ctx .resume = () => Promise .resolve ();
         return ctx;
      })();

      window .AudioContext = function () { return make (); };

      window .MediaStreamAudioDestinationNode = function (ctx)
      {
         const node = new GainNode (ctx, { channelCount: 2, channelCountMode: "explicit", channelInterpretation: "speakers" });

         node .connect (ctx .destination);
         node .stream = null;
         return node;
      };

      // decodeAudioData finishes on a decoder thread, at no fixed frame. During the frames the gate
      // holds each result until the start of the next frame (__audioFlushDecodes, before that
      // frame's events), so a decode started before frame k resolves at the start of frame k.
      // x3d-drive does the same.
      // Decodes started while the world loads (before __audioArm) are not held: the load waits
      // for them.
      const pendingDecodes = [ ];
      const decodeAudioData = OfflineAudioContext .prototype .decodeAudioData;
      let gated = false;

      OfflineAudioContext .prototype .decodeAudioData = function (data, success, failure)
      {
         if (!gated)
            return decodeAudioData .call (this, data, success, failure);

         const result = decodeAudioData .call (this, data) .then (value => ({ value }), error => ({ error }));

         return new Promise (release => pendingDecodes .push ({ result, release })) .then (r =>
         {
            if ("error" in r)
            {
               failure ?.(r .error);
               throw r .error;
            }

            success ?.(r .value);
            return r .value;
         });
      };

      window .__audioFlushDecodes = async () =>
      {
         for (const d of pendingDecodes .splice (0))
            d .release (await d .result);

         await new Promise (resolve =>
         {
            const channel = new MessageChannel ();
            channel .port1 .onmessage = () => resolve ();
            channel .port2 .postMessage (0);
         });
      };

      const suspended = new Map ();
      let rendered = null;

      window .__audioArm = times =>
      {
         const ctx = make ();

         gated = true;

         times .forEach ((t, i) => ctx .suspend (t) .then (() =>
         {
            const s = suspended .get (i + 1);

            if (s)
               s .resolve (ctx .currentTime);
            else
               suspended .set (i + 1, { done: ctx .currentTime });
         }));

         rendered = ctx .startRendering ();
      };

      window .__audioWait = k => new Promise (resolve =>
      {
         const s = suspended .get (k);

         if (s ?.done !== undefined)
            resolve (s .done);
         else
            suspended .set (k, { resolve });
      });

      window .__audioResume = () => { make (); resume (); };

      window .__audioDone = async () =>
      {
         const buffer = await rendered, out = [ ];

         for (let c = 0; c < buffer .numberOfChannels; ++ c)
         {
            const bytes = new Uint8Array (buffer .getChannelData (c) .buffer);
            let bin = "";

            for (let i = 0; i < bytes .length; i += 0x8000)
               bin += String .fromCharCode .apply (null, bytes .subarray (i, i + 0x8000));

            out .push (btoa (bin));
         }

         return { sampleRate: buffer .sampleRate, length: buffer .length, channels: out };
      };
   }"###;

pub const PIN_FRAME_RATE: &str = r###"rate => Object .defineProperty (document .getElementById ("c") .browser, "currentFrameRate", { get: () => rate, configurable: true })"###;

pub const PHYSICS_SETUP: &str = r###"async () =>
      {
         const browser = document .getElementById ("c") .browser;

         await browser .loadComponents (browser .getComponent ("RigidBodyPhysics"));

         // The probe node only gives access to the prototype; it is live in the default scene, so its own
         // time steps are not logged.
         const probe = browser .currentScene .createNode ("RigidBodyCollection") .getValue ();
         const proto = Object .getPrototypeOf (probe);
         const orig  = proto .getTimeStep;

         window .__phFrame = 0;
         window .__phDt    = [ ];

         proto .getTimeStep = function ()
         {
            const dt = orig .call (this);

            if (this !== probe)
               window .__phDt .push ({ frame: window .__phFrame, dt });
            return dt;
         };

         const plain = v =>
         {
            if (v === null || v === undefined)
               return null;

            if (typeof v !== "object")
               return v;

            if (typeof v .getNodeTypeName === "function")
               return v .getNodeName () || `<${v .getNodeTypeName ()}>`;

            if ("angle" in v && "x" in v)
               return [ v .x, v .y, v .z, v .angle ];

            if ("x" in v)
               return "z" in v ? [ v .x, v .y, v .z ] : [ v .x, v .y ];

            if (typeof v .length === "number")
               return Array .from (v, plain);

            return String (v);
         };

         const resolve = path =>
         {
            let scene = browser .currentScene, node = null;

            for (const part of path .split ("/"))
            {
               try { node = scene ?.getNamedNode (part) ?? null; } catch { node = null; }

               if (!node)
                  return null;

               scene = node .getNodeTypeName () === "Inline" ? node .getValue () .getInternalScene () : null;
            }

            return node;
         };

         window .__physSnap = nodes =>
         {
            const out = { };

            for (const [ path, fields ] of Object .entries (nodes))
            {
               const node = resolve (path);

               if (!node)
               {
                  out [path] = null;
                  continue;
               }

               const o = { type: node .getNodeTypeName () };

               for (const f of fields)
               {
                  if (f === "@bodyActive")
                     o [f] = node .getValue () .getRigidBody () .isActive ();
                  else if (f === "contacts" && o .type === "CollisionSensor")
                     o [f] = Array .from (node .contacts, c => Object .fromEntries ([ "position", "contactNormal", "depth", "geometry1", "geometry2", "body1", "body2" ] .map (n => [ n, plain (c [n]) ])));
                  else
                     o [f] = plain (node [f]);
               }

               out [path] = o;
            }

            return out;
         };
      }"###;

pub const READY: &str = r###"() => window .ready"###;

pub const LOAD: &str = r###"([u, t, d, w]) => window .load (u, t, d, w)"###;

pub const SET_DT: &str = r###"d => window .setDt (d)"###;

pub const GEOMETRY: &str = r###"() => window .geometry ()"###;

pub const FOCUS: &str = r###"() => window .focusCanvas ()"###;

pub const PHYS_SNAP: &str = r###"n => window .__physSnap (n)"###;

pub const AUDIO_ARM: &str = r###"t => window .__audioArm (t)"###;

pub const AUDIO_WAIT: &str = r###"k => window .__audioWait (k)"###;

pub const AUDIO_FLUSH: &str = r###"() => window .__audioFlushDecodes ()"###;

pub const SET_DATE: &str = r###"ms => window .setDate (ms)"###;

pub const SET_VIEWER: &str = r###"n => window .setViewer (n)"###;

pub const RESIZE: &str = r###"([ w, h ]) => { const c = document .getElementById ("c"); c .style .width = w + "px"; c .style .height = h + "px"; }"###;

pub const TWO_FRAMES: &str =
    r###"() => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (r)))"###;

pub const PH_FRAME: &str = r###"k => { window .__phFrame = k; }"###;

pub const STEP: &str = r###"t => window .step (t)"###;

pub const AUDIO_RESUME: &str = r###"() => window .__audioResume ()"###;

pub const VRML: &str = r###"() => window .vrml ()"###;

pub const PIXELS: &str = r###"() => window .pixels ()"###;

pub const PH_DT: &str = r###"() => window .__phDt"###;

pub const GET_LOG: &str = r###"() => window .getLog ()"###;

pub const AUDIO_DONE: &str = r###"() => window .__audioDone ()"###;

pub const INFO: &str = r###"() => window .info ()"###;

pub const YIELD_TASK: &str = r###"() => new Promise (resolve =>
   {
      const channel = new MessageChannel ();
      channel .port1 .onmessage = () => resolve ();
      channel .port2 .postMessage (0);
   })"###;

pub const POSE: &str = r###"() =>
         {
            // fieldOfViewScale / fieldOfViewOffset (PLANE zoom, OrthoViewpoint) are read from the page's browser object;
            // anim.html (inside the read-only pin) is untouched.
            const vp = document .getElementById ("c") .browser .getActiveViewpoint ();

            return Object .assign (window .pose (),
            {
               fieldOfViewScale: vp ._fieldOfViewScale .getValue (),
               fieldOfViewOffset: vp ._fieldOfViewOffset ? Array .from (vp ._fieldOfViewOffset .getValue ()) : undefined,
            });
         }"###;

pub const INLINES: &str = r###"() =>
         {
            const
               browser = document .getElementById ("c") .browser,
               out     = { };

            const walk = (scene, prefix) =>
            {
               for (const node of scene .namedNodes)
               {
                  const n = node .getValue ();

                  if (n ?.getTypeName () !== "Inline")
                     continue;

                  const inner = n .getInternalScene ();

                  if (inner === browser .getDefaultScene ())
                  {
                     out [prefix + n .getName ()] = null;
                     continue;
                  }

                  out [prefix + n .getName ()] = inner .toVRMLString ();
                  walk (inner, prefix + n .getName () + "/");
               }
            };

            walk (browser .currentScene, "");
            return out;
         }"###;

pub const PICK_PROBES: &str = r###"points =>
      {
         const browser = document .getElementById ("c") .browser;

         return points .map (([ x, y ]) =>
         {
            if (!browser .touch (x, y))
               return { id: 0 };

            const h = browser .getHit ();

            return { id: 1, point: [ h .point .x, h .point .y, h .point .z ], normal: [ h .normal .x, h .normal .y, h .normal .z ],
                     texCoord: [ h .texCoord .x, h .texCoord .y, h .texCoord .z, h .texCoord .w ] };
         });
      }"###;

pub const COLLISION_PROBES: &str = r###"directions =>
      {
         const
            browser = document .getElementById ("c") .browser,
            layer   = browser .getActiveLayer (),
            Vector3 = browser .getActiveViewpoint () .getPosition () .constructor;

         return directions .map (([ x, y, z ]) =>
         {
            const c = layer .getClosestObject (new Vector3 (x, y, z));

            return { distance: c .distance, id: c .id, normal: [ c .normal .x, c .normal .y, c .normal .z ] };
         });
      }"###;
