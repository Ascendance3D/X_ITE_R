// A W3C WebDriver session with the small part of Playwright's Page API that smoke.mjs uses, so that the same checks run in
// browsers that Playwright cannot drive in their shipped form: Safari (safaridriver) and Firefox (geckodriver).
//
// Differences from Playwright, which smoke.mjs reports with the results:
// - console messages: WebDriver classic has no console log, so after each navigation the page's console.error and
//   console.warn, and its error and unhandledrejection events, are recorded from then on (errors before the
//   page's scripts ran are not seen);
// - responses: from the page's Resource Timing entries (responseStatus where the browser has it).

export async function webDriverPage (endpoint, capabilities)
{
   const call = async (method, path, body) =>
   {
      const response = await fetch (`${endpoint}${path}`, { method, headers: { "Content-Type": "application/json" }, body: body === undefined ? undefined : JSON .stringify (body) });
      const json     = await response .json ();

      if (json .value ?.error)
         throw new Error (`WebDriver ${path}: ${json .value .error}: ${json .value .message}`);

      return json .value;
   };

   const created = await call ("POST", "/session", { capabilities: { alwaysMatch: capabilities } });
   const id      = created .sessionId;
   const s       = path => `/session/${id}${path}`;
   const caps    = created .capabilities;

   const script = (fn, arg) => call ("POST", s ("/execute/async"), {
      script: `const done = arguments [arguments .length - 1];
               Promise .resolve ((${fn}) (arguments [0])) .then (v => done ({ ok: v === undefined ? null : v }), e => done ({ error: String (e && e .stack || e) }));`,
      args: [ arg === undefined ? null : arg ],
   })
   .then (r => { if (r && "error" in r) throw new Error (r .error); return r ?.ok ?? undefined; });

   const collector = () =>
   {
      if (window .__smokeConsole)
         return;

      window .__smokeConsole = [ ];

      for (const level of [ "error", "warn" ])
      {
         const original = console [level];

         console [level] = (... args) => { window .__smokeConsole .push (`[${level === "warn" ? "warning" : "error"}] ${args .map (String) .join (" ")}`); original .apply (console, args); };
      }

      // The pointer events the page gets, for a check that fails (smoke.mjs writes them to pointer.txt).
      window .__smokePointer = [ ];

      for (const type of [ "pointerdown", "pointermove", "pointerup", "pointercancel", "wheel" ])
         addEventListener (type, e => window .__smokePointer .push (`${performance .now () .toFixed (0)} ${type} ${e .clientX},${e .clientY} b=${e .buttons} id=${e .pointerId ?? ""} ${e .isTrusted ? "" : "untrusted"}`), true);

      addEventListener ("error", e => window .__smokeConsole .push (`[pageerror] ${e .message}`));
      addEventListener ("unhandledrejection", e => window .__smokeConsole .push (`[pageerror] ${e .reason}`));
   };

   let pointer = { x: 0, y: 0 };

   const actions = list => call ("POST", s ("/actions"), { actions: list }) .then (() => call ("DELETE", s ("/actions")) .catch (() => { }));
   const pointerAction = steps => actions ([ { type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: steps } ]);
   const element = async selector => (await call ("POST", s ("/element"), { using: "css selector", value: selector })) ["element-6066-11e4-a52f-4a6f7f6f4f6f"] ?? Object .values (await call ("POST", s ("/element"), { using: "css selector", value: selector })) [0];

   const page =
   {
      browserName: caps .browserName,
      browserVersion: caps .browserVersion,
      kind: "webdriver",
      consoleLines: [ ],

      async goto (url) { await page .flushConsole (); await call ("POST", s ("/url"), { url }); await script (collector); },
      async reload () { await page .flushConsole (); await call ("POST", s ("/refresh"), { }); await script (collector); },
      evaluate: (fn, arg) => script (fn, arg),
      waitForTimeout: ms => new Promise (r => setTimeout (r, ms)),

      async waitForFunction (fn, arg, { timeout = 30000 } = { })
      {
         const end = Date .now () + timeout;

         for (;;)
         {
            if (await script (fn, arg) .catch (() => false))
               return;

            if (Date .now () > end)
               throw new Error (`waitForFunction: timeout ${timeout} ms`);

            await new Promise (r => setTimeout (r, 100));
         }
      },

      locator (selector)
      {
         return {
            textContent: () => script (sel => document .querySelector (sel) .textContent, selector),
            allTextContents: () => script (sel => [ ... document .querySelectorAll (sel) ] .map (e => e .textContent), selector),
            // The window can be smaller than the page (a 1000-pixel window on a laptop screen): the element comes into
            // view first, else Safari's element screenshot keeps only the visible part and pointer positions are off-screen.
            boundingBox: () => script (sel => { const e = document .querySelector (sel); e .scrollIntoView ({ block: "nearest", inline: "nearest" }); const r = e .getBoundingClientRect (); return { x: r .x, y: r .y, width: r .width, height: r .height }; }, selector),
            screenshot: async () =>
            {
               await script (sel => document .querySelector (sel) .scrollIntoView ({ block: "nearest", inline: "nearest" }), selector);
               return Buffer .from (await call ("GET", s (`/element/${await element (selector)}/screenshot`)), "base64");
            },
         };
      },

      mouse:
      {
         move: (x, y, { steps = 1 } = { }) =>
         {
            const from = pointer, list = [ ];

            for (let i = 1; i <= steps; ++ i)
               list .push ({ type: "pointerMove", duration: 16, origin: "viewport", x: Math .round (from .x + (x - from .x) * i / steps), y: Math .round (from .y + (y - from .y) * i / steps) });

            pointer = { x, y };
            return pointerAction (list);
         },
         down: () => pointerAction ([ { type: "pointerMove", duration: 0, origin: "viewport", x: Math .round (pointer .x), y: Math .round (pointer .y) }, { type: "pointerDown", button: 0 } ]),
         up:   () => pointerAction ([ { type: "pointerMove", duration: 0, origin: "viewport", x: Math .round (pointer .x), y: Math .round (pointer .y) }, { type: "pointerUp", button: 0 } ]),
         wheel: (dx, dy) => actions ([ { type: "wheel", id: "wheel", actions: [ { type: "scroll", origin: "viewport", x: Math .round (pointer .x), y: Math .round (pointer .y), deltaX: dx, deltaY: dy, duration: 0 } ] } ]),
      },

      // The pointer stays down across mouse.move calls: WebDriver releases nothing between action calls unless told,
      // but "DELETE /actions" releases held buttons, so a drag is sent as one action list.
      async drag (from, to, steps, hold)
      {
         const list = [ { type: "pointerMove", duration: 0, origin: "viewport", x: Math .round (from [0]), y: Math .round (from [1]) }, { type: "pointerDown", button: 0 } ];

         for (let i = 1; i <= steps; ++ i)
            list .push ({ type: "pointerMove", duration: 16, origin: "viewport", x: Math .round (from [0] + (to [0] - from [0]) * i / steps), y: Math .round (from [1] + (to [1] - from [1]) * i / steps) });

         if (hold)
            list .push ({ type: "pause", duration: hold });

         list .push ({ type: "pointerUp", button: 0 });
         pointer = { x: to [0], y: to [1] };
         await call ("POST", s ("/actions"), { actions: [ { type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: list } ] });
      },

      // A press held while the caller reads values: down now, up later.
      async press (x, y) { pointer = { x, y }; await call ("POST", s ("/actions"), { actions: [ { type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [ { type: "pointerMove", duration: 0, origin: "viewport", x: Math .round (x), y: Math .round (y) }, { type: "pointerDown", button: 0 } ] } ] }); },
      async release () { await call ("POST", s ("/actions"), { actions: [ { type: "pointer", id: "mouse", parameters: { pointerType: "mouse" }, actions: [ { type: "pointerUp", button: 0 } ] } ] }); },

      async selectOption (selector, value)
      {
         await script (([ sel, v ]) => { const e = document .querySelector (sel); e .value = v; e .dispatchEvent (new Event ("change", { bubbles: true })); }, [ selector, value ]);
      },

      // Element Click scrolls the element to the bottom edge of the viewport; geckodriver (Firefox 159) then reports a
      // button there as obscured by <html>. The element goes to the center of the viewport first.
      async click (selector)
      {
         await script (sel => document .querySelector (sel) .scrollIntoView ({ block: "center", inline: "nearest" }), selector);
         await call ("POST", s (`/element/${await element (selector)}/click`), { });
      },

      async responses ()
      {
         return script (() => performance .getEntriesByType ("resource") .map (e => ({ url: e .name, status: e .responseStatus || null, type: "" })));
      },

      pointerLog: () => script (() => (window .__smokePointer ?? [ ]) .splice (0)) .catch (() => [ ]),

      async flushConsole ()
      {
         const lines = await script (() => (window .__smokeConsole ?? [ ]) .splice (0)) .catch (() => [ ]);

         page .consoleLines .push (... (lines ?? [ ]));
      },

      // Decodes a PNG in the page: `createImageBitmap` of a Blob, read with a 2D canvas.
      decode: b64 => script (async b64 =>
      {
         const bytes  = Uint8Array .from (atob (b64), c => c .charCodeAt (0));
         const bitmap = await createImageBitmap (new Blob ([ bytes ], { type: "image/png" }));
         const c      = document .createElement ("canvas");

         c .width = bitmap .width; c .height = bitmap .height;

         const g = c .getContext ("2d");

         g .drawImage (bitmap, 0, 0);
         return { w: bitmap .width, h: bitmap .height, px: Array .from (g .getImageData (0, 0, bitmap .width, bitmap .height) .data) };
      },
      b64),

      async setWindowRect (rect) { await call ("POST", s ("/window/rect"), rect); },
      async close () { await page .flushConsole (); await call ("DELETE", s ("")) .catch (() => { }); },
   };

   return page;
}
