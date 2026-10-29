// The CI shards of the browser tests (.github/workflows/web-tests.yml). Each shard runs whole test files on its own
// runner, so no test is filtered or skipped. The shards are balanced by the CI durations on lavapipe: the phone test
// alone takes about 4 minutes there, and the viewpoints 01 cases about 20 minutes on SwiftShader.
//
//   node tests/browser/shards.mjs matrix        the shard names as a JSON array, after the check below
//   node tests/browser/shards.mjs files NAME    the files of one shard, one path per line
//
// The check: every *.test.mjs file in this directory is in exactly one shard, and every listed file exists. A new
// test file that is not listed here fails the CI "Changed paths" job; it is never left out in silence.

import { readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const DIR = dirname (fileURLToPath (import.meta.url));

export const SHARDS = {
   phone:  [ "runtime02_phone.test.mjs" ],
   tablet: [ "runtime02_tablet.test.mjs", "runtime02_lifecycle.test.mjs" ],
   input:  [ "runtime02_input.test.mjs", "loadurl01.test.mjs" ],
   cases:  [ "api.test.mjs", "runtime02.test.mjs", "fallback01.test.mjs", "dataurl01.test.mjs", "degenerate01.test.mjs", "gltfworld01.test.mjs", "gltfworld02.test.mjs", "fields01.test.mjs", "fields02.test.mjs", "fields03.test.mjs", "fields04.test.mjs", "routes01.test.mjs", "options01.test.mjs", "attributes01.test.mjs", "scenetext01.test.mjs", "entry01.test.mjs", "scripturl01.test.mjs" ],
   viewpoints: [ "viewpoints01.test.mjs" ],
};

export function check (shards = SHARDS, files = readdirSync (DIR) .filter (f => f .endsWith (".test.mjs")))
{
   const errors = [ ];
   const seen   = new Map ();

   for (const [ name, list ] of Object .entries (shards))
   {
      if (!/^[a-z0-9-]+$/ .test (name))
         errors .push (`shard name ${JSON .stringify (name)} is not [a-z0-9-]+`);

      if (!list .length)
         errors .push (`shard ${name} has no files`);

      for (const file of list)
      {
         if (seen .has (file))
            errors .push (`${file} is in shards ${seen .get (file)} and ${name}`);
         else
            seen .set (file, name);

         if (!files .includes (file))
            errors .push (`${file} (shard ${name}) does not exist`);
      }
   }

   for (const file of files)
   {
      if (!seen .has (file))
         errors .push (`${file} is in no shard`);
   }

   return errors;
}

if (process .argv [1] === fileURLToPath (import.meta.url))
{
   const [ command, name ] = process .argv .slice (2);
   const errors            = check ();

   if (errors .length)
   {
      console .error (errors .map (e => `shards.mjs: ${e}`) .join ("\n"));
      process .exit (1);
   }

   if (command === "matrix" && !name)
   {
      console .log (JSON .stringify (Object .keys (SHARDS)));
   }
   else if (command === "files" && Object .hasOwn (SHARDS, name ?? ""))
   {
      for (const file of SHARDS [name])
         console .log (relative (process .cwd (), join (DIR, file)));
   }
   else
   {
      console .error ("usage: node tests/browser/shards.mjs matrix | files NAME");
      process .exit (1);
   }
}
