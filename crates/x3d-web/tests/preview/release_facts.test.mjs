// Tests of release_facts.mjs (no browser): node --test tests/preview/release_facts.test.mjs
import test from "node:test";
import assert from "node:assert/strict";
import { versionFacts } from "./release_facts.mjs";

const commit = "6e41f12a71ecf769133b4a41688f9c1b2cc16497";
const info   = { release: "x_ite_r-canary-0.2.0", version: "0.2.0", commit };
const facts  = { version: "X_ITE_R 0.2.0", build: `${commit} (x_ite_r-canary-0.2.0, rustc 1.95.0, wasm-bindgen 0.2.129)` };

test ("the shown version and commit of the package under test pass", () =>
{
   assert .equal (versionFacts (facts, info) .pass, true);
});

test ("a page of another version fails (PR #521: the check accepted only 0.1.0)", () =>
{
   assert .equal (versionFacts ({ ...facts, version: "X_ITE_R 0.1.0" }, info) .pass, false);
   assert .equal (versionFacts (facts, { ...info, version: "0.1.0" }) .pass, false);
});

test ("Browser Preview 0.1.0 still passes against its own build-info.json", () =>
{
   const old = "11e148db0b82cb9af25b80acb44a4303227313b6";
   assert .equal (versionFacts ({ version: "X_ITE_R 0.1.0", build: `${old} (x_ite_r-browser-preview-0.1.0, rustc 1.95.0, wasm-bindgen 0.2.129)` },
      { release: "x_ite_r-browser-preview-0.1.0", version: "0.1.0", commit: old }) .pass, true);
});

test ("another commit or release in #build fails", () =>
{
   assert .equal (versionFacts ({ ...facts, build: `${"0" .repeat (40)} (x_ite_r-canary-0.2.0, rustc 1.95.0)` }, info) .pass, false);
   assert .equal (versionFacts ({ ...facts, build: `${commit} (x_ite_r-browser-preview-0.2.0, rustc 1.95.0)` }, info) .pass, false);
});

test ("a development tree (no build-info.json) or a bad build-info.json fails", () =>
{
   assert .equal (versionFacts ({ ...facts, build: "no build-info.json (development tree)" }, info) .pass, false);
   assert .equal (versionFacts (facts, null) .pass, false);
   assert .equal (versionFacts (facts, { ...info, version: "0.2" }) .pass, false);
   assert .equal (versionFacts (facts, { ...info, commit: "6e41f12a" }) .pass, false);
   assert .equal (versionFacts (facts, { version: "0.2.0", commit }) .pass, false);
});
