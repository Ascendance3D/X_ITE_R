// The start page (index.html): the build facts from build-info.json, and one status line from the real state of the
// <x3d-canvas>: the graphics backend the engine uses, the world, the load status. It only reads; the page has no
// controls of the engine (they are on preview/).

import "../x_ite_r.mjs";

const $      = id => document .getElementById (id);
const canvas = $("canvas");
const world  = canvas .getAttribute ("src");

function status (text, kind = "")
{
   $("status") .textContent = `Status: ${text}`;
   $("status") .className   = `mono ${kind}`;
}

async function facts ()
{
   try
   {
      const response = await fetch ("build-info.json", { cache: "no-store" });

      if (!response .ok)
         throw new Error (`HTTP ${response .status}`);

      const info = await response .json ();

      $("f-version")   .textContent = `${info .version} (${info .kind})`;
      $("f-commit")    .textContent = info .commit;
      $("f-wasm")      .textContent = `pkg/x3d_web_bg.wasm, ${Number (info .wasm_bytes) .toLocaleString ("en-US")} bytes, SHA-256 ${info .wasm_sha256}`;
      $("f-rustc")     .textContent = `rustc ${info .rustc}, wasm-bindgen ${info .wasm_bindgen}, ${info .target}`;
      $("f-reference") .textContent = info .reference;
   }
   catch (error)
   {
      // A development tree has no build-info.json.
      $("f-commit") .textContent = `no build-info.json (${error .message})`;
   }
}

async function graphics ()
{
   try
   {
      await canvas .browser .ready ();

      const engine  = canvas .browser [Symbol .for ("X_ITE_R.engine")];
      const backend = engine ?.backend === "webgl2" ? "WebGL2" : "WebGPU";

      $("graphics") .textContent = `Graphics: ${backend}${engine ?.fallbackReason ? " (WebGPU did not start)" : ""}`;
      $("graphics") .className   = "mono ok";
   }
   catch (error)
   {
      $("graphics") .textContent = "Graphics: none (neither WebGPU nor WebGL2 started)";
      $("graphics") .className   = "mono error";
      status ("not started", "error");
   }
}

function loaded ()
{
   const url = canvas .browser ?.currentScene ?.worldURL ?? "";

   // "load" also comes for the empty world the browser starts with.
   if (url .endsWith (world))
      status ("loaded", "ok");
}

canvas .addEventListener ("load", loaded);
// The world can finish loading before this module runs (x_ite_r.mjs does not wait for it).
loaded ();

canvas .addEventListener ("error", () => status (`${world} did not load`, "error"));

facts ();
graphics ();
