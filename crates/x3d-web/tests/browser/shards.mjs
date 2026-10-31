// The CI shards of the browser tests (.github/workflows/web-tests.yml). Each shard runs on its own runner. A shard
// entry is a whole test file, or { file, cases } for a file whose reference cases are split over shards: the shard
// then registers only those cases (XR_CASES, runtime02_harness.mjs), so no test is filtered or skipped. The shards are
// balanced by measured CI durations (issue #187): the phone test alone takes about 4 minutes on lavapipe, and one
// viewpoints 01 case up to about 5 minutes on SwiftShader.
//
//   node tests/browser/shards.mjs matrix        the shard names as a JSON array, after the check below
//   node tests/browser/shards.mjs files NAME    the files of one shard, one path per line
//   node tests/browser/shards.mjs cases NAME    the case keys of one shard, comma-separated (empty: all cases)
//
// The check: every *.test.mjs file in this directory is in exactly one shard, or split over shards by case; every
// listed file exists. The case lists of a split file name each case of its *_case.mjs exactly once, and a shard with
// a case list has no other entry. A new test file or case that is not listed here fails the CI "Changed paths" job;
// it is never left out in silence.

import { readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const DIR = dirname (fileURLToPath (import.meta.url));

// The cases-* shards: about 439 s of tests each, by the mean file times of 3 runs of the old cases shard (runs
// 40993357287, 40996523915, 40998734092: 860 s to 891 s in one shard). The viewpoints-* shards: by the case times of
// the slowest of the same runs, 309 s, 452 s and 397 s.
export const SHARDS = {
   phone:  [ "runtime02_phone.test.mjs" ],
   tablet: [ "runtime02_tablet.test.mjs", "runtime02_lifecycle.test.mjs" ],
   input:  [ "runtime02_input.test.mjs", "loadurl01.test.mjs" ],
   "cases-1": [ "scripturl01.test.mjs", "script01.test.mjs", "attributes01.test.mjs", "gltfworld01.test.mjs", "options01.test.mjs", "fallback01.test.mjs", "routes01.test.mjs", "scenecreate01.test.mjs", "gltfworld02.test.mjs", "fields04.test.mjs", "sound01.test.mjs" ],
   "cases-2": [ "runtime02.test.mjs", "dataurl01.test.mjs", "scenetext01.test.mjs", "degenerate01.test.mjs", "api.test.mjs", "entry01.test.mjs", "fields01.test.mjs", "fields02.test.mjs", "fields03.test.mjs", "rootnodes01.test.mjs", "scriptlife01.test.mjs" ],
   "viewpoints-1": [ { file: "viewpoints01.test.mjs", cases: [ "viewpointsApi.steps", "viewpointsApi.api" ] } ],
   "viewpoints-2": [ { file: "viewpoints01.test.mjs", cases: [ "viewpointsApi.update", "viewpointsApi.errors", "viewpointsApi.world" ] } ],
   "viewpoints-3": [ { file: "viewpoints01.test.mjs", cases: [ "viewpointsApi.bind", "viewpointsApi.change", "viewpointsApi.viewAll" ] } ],
};

const fileOf = entry => typeof entry === "string" ? entry : entry .file;

// Every `group.case` key of the *_case.mjs module of a test file.
export async function caseKeys (file)
{
   const { CASE_GROUPS } = await import (pathToFileURL (join (DIR, file .replace (/\.test\.mjs$/, "_case.mjs"))) .href);

   return Object .entries (CASE_GROUPS) .flatMap (([ group, { cases } ]) => Object .keys (cases) .map (name => `${group}.${name}`));
}

export async function check (shards = SHARDS, files = readdirSync (DIR) .filter (f => f .endsWith (".test.mjs")), keysOf = caseKeys)
{
   const errors = [ ];
   const seen   = new Map ();
   const split  = new Map ();
   const whole  = new Set ();

   for (const [ name, list ] of Object .entries (shards))
   {
      if (!/^[a-z0-9-]+$/ .test (name))
         errors .push (`shard name ${JSON .stringify (name)} is not [a-z0-9-]+`);

      if (!list .length)
         errors .push (`shard ${name} has no files`);

      for (const entry of list)
      {
         const file = fileOf (entry ?? { });

         if (typeof file !== "string" || !file)
         {
            errors .push (`shard ${name} has an entry that is not a file name or { file, cases }: ${JSON .stringify (entry)}`);
            continue;
         }

         if (typeof entry !== "string")
         {
            const cases = Array .isArray (entry .cases) ? entry .cases : [ ];

            if (list .length !== 1)
               errors .push (`shard ${name} has a case list for ${file} and other entries`);

            if (!cases .length)
               errors .push (`shard ${name} has no case list (an array of group.case keys) for ${file}`);

            if (!split .has (file))
               split .set (file, [ ]);

            split .get (file) .push ([ name, cases ]);
         }

         const kind = typeof entry === "string" ? "file" : "cases";

         if (kind === "file")
            whole .add (file);

         if (seen .has (file) && (kind === "file" || seen .get (file) .kind === "file"))
            errors .push (`${file} is in shards ${seen .get (file) .name} and ${name}`);
         else if (!seen .has (file))
            seen .set (file, { name, kind });

         if (!files .includes (file))
            errors .push (`${file} (shard ${name}) does not exist`);
      }
   }

   for (const file of files)
   {
      if (!seen .has (file))
         errors .push (`${file} is in no shard`);
   }

   // A file that is also listed whole has its error above; the case lists of a missing file cannot be read.
   for (const [ file, lists ] of split)
   {
      if (!files .includes (file) || whole .has (file))
         continue;

      const all   = await keysOf (file);
      const count = new Map (all .map (key => [ key, [ ] ]));

      for (const [ name, cases ] of lists)
      {
         for (const key of cases)
         {
            if (count .has (key))
               count .get (key) .push (name);
            else
               errors .push (`${JSON .stringify (key)} (shard ${name}) is not a case of ${file}; its cases are ${all .join (", ")}`);
         }
      }

      for (const [ key, names ] of count)
      {
         if (!names .length)
            errors .push (`${key} of ${file} is in no shard`);
         else if (names .length > 1)
            errors .push (`${key} of ${file} is in shards ${names .join (" and ")}`);
      }
   }

   return errors;
}

if (process .argv [1] === fileURLToPath (import.meta.url))
{
   const [ command, name ] = process .argv .slice (2);
   const errors            = await check ();

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
      for (const entry of SHARDS [name])
         console .log (relative (process .cwd (), join (DIR, fileOf (entry))));
   }
   else if (command === "cases" && Object .hasOwn (SHARDS, name ?? ""))
   {
      console .log (SHARDS [name] .flatMap (entry => entry .cases ?? [ ]) .join (","));
   }
   else
   {
      console .error ("usage: node tests/browser/shards.mjs matrix | files NAME | cases NAME");
      process .exit (1);
   }
}
