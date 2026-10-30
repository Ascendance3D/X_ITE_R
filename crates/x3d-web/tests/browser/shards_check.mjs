// Tests of the shard check in shards.mjs (issue #187): the CI layout passes, and a viewpoints case that is in no shard,
// in two shards or not a case at all fails it, as do the other malformed layouts. It needs no browser and no build;
// the CI "Changed paths" job runs it before it reads the shard matrix.
//
//   node --test tests/browser/shards_check.mjs

import { test } from "node:test";
import assert from "node:assert/strict";
import { SHARDS, caseKeys, check } from "./shards.mjs";

const FILE = "viewpoints01.test.mjs";

// The CI shards without the viewpoints shards, and viewpoints shards made from case lists.
const WHOLE = Object .fromEntries (Object .entries (SHARDS) .filter (([ name ]) => !name .startsWith ("viewpoints")));

const layout = (... lists) => ({
   ... WHOLE,
   ... Object .fromEntries (lists .map ((cases, i) => [ `viewpoints-${i + 1}`, [ { file: FILE, cases } ] ])),
});

const v = (... names) => names .map (name => `viewpointsApi.${name}`);

test ("the CI layout passes the check", async () =>
{
   assert .deepEqual (await check (), [ ]);
});

test ("the viewpoints shards name each of the 8 cases once", async () =>
{
   const keys     = await caseKeys (FILE);
   const assigned = Object .values (SHARDS) .flat () .filter (entry => entry .file === FILE) .flatMap (entry => entry .cases);

   assert .equal (keys .length, 8);
   assert .deepEqual ([ ... assigned ] .sort (), [ ... keys ] .sort ());
});

test ("a viewpoints case in no shard fails", async () =>
{
   const errors = await check (layout (v ("steps", "api"), v ("update", "errors", "world"), v ("bind", "change")));

   assert .deepEqual (errors, [ `viewpointsApi.viewAll of ${FILE} is in no shard` ]);
});

test ("a viewpoints case in two shards fails", async () =>
{
   const errors = await check (layout (v ("steps", "api", "bind"), v ("update", "errors", "world"), v ("bind", "change", "viewAll")));

   assert .deepEqual (errors, [ `viewpointsApi.bind of ${FILE} is in shards viewpoints-1 and viewpoints-3` ]);
});

test ("an unknown viewpoints case fails", async () =>
{
   const errors = await check (layout (v ("steps", "api", "nope"), v ("update", "errors", "world"), v ("bind", "change", "viewAll")));

   assert .equal (errors .length, 1);
   assert .match (errors [0], /^"viewpointsApi\.nope" \(shard viewpoints-1\) is not a case of viewpoints01\.test\.mjs; its cases are /);
});

test ("a case-list shard with another entry fails", async () =>
{
   const shards = layout (v ("steps", "api"), v ("update", "errors", "world"), v ("bind", "change", "viewAll"));

   shards ["viewpoints-1"] .push ("fields03.test.mjs");

   const errors = await check (shards);

   assert .ok (errors .includes (`shard viewpoints-1 has a case list for ${FILE} and other entries`), errors .join ("\n"));
   assert .ok (errors .includes ("fields03.test.mjs is in shards cases-2 and viewpoints-1"), errors .join ("\n"));
});

test ("a file listed whole and split fails", async () =>
{
   const errors = await check ({ ... layout (v ("steps", "api", "update", "errors", "world", "bind", "change", "viewAll")), viewpoints: [ FILE ] });

   assert .deepEqual (errors, [ `${FILE} is in shards viewpoints-1 and viewpoints` ]);
});

test ("a split file with no listed cases fails", async () =>
{
   const errors = await check ({ ... WHOLE, "viewpoints-1": [ { file: FILE } ] });

   assert .ok (errors .includes (`shard viewpoints-1 has no case list (an array of group.case keys) for ${FILE}`), errors .join ("\n"));
   assert .ok (errors .includes (`viewpointsApi.steps of ${FILE} is in no shard`), errors .join ("\n"));
});

test ("malformed entries fail without an exception", async () =>
{
   const errors = await check ({ ... WHOLE, "viewpoints-1": [ { file: FILE, cases: "viewpointsApi.steps" } ], "viewpoints-2": [ { cases: [ ] } ], "viewpoints-3": [ null ] });

   assert .ok (errors .includes (`shard viewpoints-1 has no case list (an array of group.case keys) for ${FILE}`), errors .join ("\n"));
   assert .ok (errors .some (e => e .startsWith ("shard viewpoints-2 has an entry that is not a file name")), errors .join ("\n"));
   assert .ok (errors .some (e => e .startsWith ("shard viewpoints-3 has an entry that is not a file name")), errors .join ("\n"));
});
