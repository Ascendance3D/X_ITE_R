// The controlled local server of the browser tests (api.test.mjs, runtime02*.test.mjs) and of the pinned X_ITE
// recorder (oracle/x_ite-web-runtime-02.mjs): one implementation, so both engines see the same responses.
//
// Files come from `root` (the crate directory), with `Cache-Control: no-store`. Path prefixes change a response:
//
//   /hold/<key>/<path>     wait until hold (key) is released (server .release (key)), then serve <path>
//   /delay/<ms>/<path>     wait <ms> milliseconds, then serve <path>
//   /status/<code>/<path>  answer with HTTP status <code> and no body
//   /garbage/<path>        answer 200 with bytes that are no file format
//
// `mounts` maps other path prefixes to other directories (the recorder serves the X_ITE pin at /x_ite/). Every
// request is logged in `server .requests` (its path, the time and the Host header).

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const TYPES = {
   ".html": "text/html", ".mjs": "text/javascript", ".js": "text/javascript", ".wasm": "application/wasm",
   ".css": "text/css", ".json": "application/json", ".png": "image/png", ".wrl": "model/vrml", ".x3dv": "model/x3d+vrml",
};

// Clean fixtures of other oracle sets that both engines load: /fonts/ (PT Sans, OFL; notfont.ttf, see PROVENANCE.txt),
// /media/ (synthetic tones) and /synthetic-x6/, /synthetic-inline-scripts/, /synthetic-externproto/ and /synthetic-followers/ (the synthetic Script
// worlds, script01.test.mjs).
const REPO = normalize (join (new URL (".", import.meta.url) .pathname, "../../../.."));

const FIXTURES = {
   "/fonts/": join (REPO, "oracle/synthetic-rea-b08-net/fonts"),
   "/media/": join (REPO, "oracle/synthetic-rea-b05/media"),
   "/synthetic-x6/": join (REPO, "oracle/synthetic-x6"),
   "/synthetic-inline-scripts/": join (REPO, "oracle/synthetic-inline-scripts"),
   "/synthetic-externproto/": join (REPO, "oracle/synthetic-externproto"),
   "/synthetic-followers/": join (REPO, "oracle/synthetic-followers"),
};

export async function startServer (root, { mounts = { }, pages = { } } = { })
{
   mounts = { ... FIXTURES, ... mounts };

   const holds    = new Map ();
   const requests = [ ];

   // A hold: a Promise and its release.
   const hold = key =>
   {
      if (!holds .has (key))
      {
         let release;
         const promise = new Promise (resolve => release = resolve);

         holds .set (key, { promise, release });
      }

      return holds .get (key);
   };

   const handler = async (req, res) =>
   {
      let pathname = decodeURIComponent (new URL (req .url, "http://x") .pathname);

      requests .push ({ path: pathname, host: req .headers .host, time: Date .now () });

      try
      {
         if (pathname in pages)
         {
            res .writeHead (200, { "Content-Type": "text/html", "Cache-Control": "no-store" });
            return res .end (pages [pathname]);
         }

         let match;

         if ((match = pathname .match (/^\/hold\/([^/]+)(\/.*)$/)))
         {
            await hold (match [1]) .promise;
            pathname = match [2];
         }
         else if ((match = pathname .match (/^\/delay\/(\d+)(\/.*)$/)))
         {
            await new Promise (resolve => setTimeout (resolve, Number (match [1])));
            pathname = match [2];
         }

         if ((match = pathname .match (/^\/status\/(\d+)\//)))
         {
            res .writeHead (Number (match [1]), { "Cache-Control": "no-store" });
            return res .end ();
         }

         if (pathname .startsWith ("/garbage/"))
         {
            res .writeHead (200, { "Content-Type": TYPES [extname (pathname)] ?? "application/octet-stream", "Cache-Control": "no-store" });
            return res .end (Buffer .from ("\x00\x01\x02 this is no file format \xff\xfe", "binary"));
         }

         let base = root;

         for (const [ prefix, dir ] of Object .entries (mounts))
         {
            if (pathname .startsWith (prefix))
            {
               base     = dir;
               pathname = pathname .slice (prefix .length - 1);
               break;
            }
         }

         const path = normalize (join (base, pathname));

         if (!path .startsWith (base))
         {
            res .writeHead (403, { "Cache-Control": "no-store" });
            return res .end ();
         }

         const body = await readFile (path);

         res .writeHead (200, { "Content-Type": TYPES [extname (path) .toLowerCase ()] ?? "application/octet-stream", "Cache-Control": "no-store" });
         res .end (body);
      }
      catch
      {
         if (!res .headersSent)
            res .writeHead (404, { "Cache-Control": "no-store" });

         res .end ();
      }
   };

   // The second server is another origin (another port) with the same files: a fetch from the page to it is
   // cross-origin, and no response has a CORS header.
   const servers = [ createServer (handler), createServer (handler) ];

   await Promise .all (servers .map (server => new Promise (resolve => server .listen (0, "127.0.0.1", resolve))));

   const [ port, foreignPort ] = servers .map (server => server .address () .port);

   return {
      base: `http://127.0.0.1:${port}`,
      foreign: `http://127.0.0.1:${foreignPort}`,
      requests,
      release: key => hold (key) .release (),
      // A new hold for `key` (the next request waits again).
      arm: key => holds .delete (key),
      close: () => Promise .all (servers .map (server => new Promise (resolve => server .close (resolve)))),
   };
}
