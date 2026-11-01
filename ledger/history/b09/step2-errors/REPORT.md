# Step 2: scene error channel, XML unknown-node baseline, Script shutdown tests

Worktree integration-b06-b08, local commit only (nothing pushed). Final gate: `cargo test --workspace --release -j 12 --no-fail-fast` = 994 passed, 0 failed, 1 ignored (gen_oracle_worlds).

## Design of the error channel (Task A)
Same `Scene::warnings` Vec, so print order is preserved. An error entry carries the prefix `ERROR_LEVEL_MARK` ("\u{1}error\u{1}"), added by new `Scene::error(String)`. `console_level(&str) -> (is_error, &str)` reads an entry. Both live in scene.rs (no new pub module; the constant and fn are re-exported by the existing `pub use scene::*`).
Routing: `Runtime::log_warn` decodes the mark and forwards to `log_error`, so every flush site (lib.rs x4, inline.rs, browser_load.rs, extern_proto.rs, ctx_api.rs flush_warnings, take_error_warnings paths) routes errors with no per-site edit. CLI printers (x3d-parse dump, vrml check) strip/print the level.
Conversions: XML unknown node type is now an error with X_ITE's actual text `Error: Unknown node type 'X'. You probably have ...` (the pinned DEVELOPMENT build logs the Error object; the old text "XML Parser: Unknown node..." was wrong as well as the wrong level). Also XML `Profile 'X' is not supported.` (XMLParser.x3dElement catch -> console.error), found cheaply.
Not converted (not cheap/outside the quick look): `Invalid XML.` is console.error in X_ITE but warn in Rust; X_ITE's DEVELOPMENT double logging (warn "XML Parser: ..." plus error "Error: ..." for ROUTE and similar failures); XMLParser lines 519/622/707/763 and JSONParser console.error sites were not individually checked/converted.

## Files changed
crates/x3d-scene/src/scene.rs; crates/x3d-runtime/src/lib.rs (log_warn); crates/x3d-parse/src/xml.rs; crates/x3d-parse/src/bin/x3d-parse.rs; crates/vrml/src/bin/vrml.rs; crates/x3d-parse/tests/html_support_rea.rs (strip the mark; still level-blind, comment explains); oracle/tools/gen_rea_b09_sai_parse.py; oracle/synthetic-rea-b09-sai/parse-xml-unknown.wrl (generated; other generated worlds byte-identical); oracle/rea-batch09-sai-baseline/{cases.txt,parse-xml-unknown/js.json}; crates/x3d-script/tests/script_shutdown_kept_difference.rs (new).

## Baseline case added (Task B)
`parse-xml-unknown` (5 snippets via createX3DFromString: top-level unknown + unknown nested in Transform children + known nodes after; unknown in Shape.geometry; unknown with children; two unknowns in a Group; USE of missing + unknown). Records scene text/REJ text per snippet in the scene, and the sorted log with level. Recorded from pinned X_ITE by oracle/tools/rea_b09_sai.py (script sai-parse.json). Not touched: parse-formats stays unbaselined.

## Level-comparison proof
rea_batch09_sai.rs compares `js["log"] == rs["log"]` exactly; entries are `error: ...` / `warn: ...` strings, so the level IS compared. (x10_compare.py does not compare `log` at all, only the opt-in `console` key, which stayed empty from X_ITE in this harness for this case; the Rust test is the gate.) Proof: with the converted message but emitted via `warn` (level only differs), `REA_ONLY=parse-xml-unknown cargo test -p x3d-viewer --test rea_batch09_sai` FAILED ("log differs": error: vs warn: lines); with `Scene::error` it passes (1 passed).

## Shutdown test (Task C)
File crates/x3d-script/tests/script_shutdown_kept_difference.rs, header comment states X_ITE 16.5.0 does not call shutdown on replaceWorld and Rust's behavior is an owner-approved kept difference. Tests:
- unloading_the_world_runs_shutdown_once_per_script (second unload adds nothing)
- replacing_an_inline_scene_runs_shutdown_once_each_time
- removal_from_a_live_group_alone_does_not_run_shutdown
- twenty_load_unload_cycles_release_everything (one shutdown per cycle; live nodes, arena size constant; released_total, release_pending and dynamic_nodes bounded; Script-created nodes released)
Finding locked by the third test: removing a Script from a live Group's children alone does NOT call shutdown in Rust (no per-node hook); it runs once at world unload. So "shutdown on removal from the scene graph" is not behavior of this port; if the owner wants it, that is a separate change. No ledger status changed.

## Remaining differences not closed
- `Invalid XML.` level, DEVELOPMENT double-logging (warn + error) of failures, other XMLParser/JSONParser console.error sites not yet reviewed individually.
- html_support_rea.rs still ignores levels for those reasons (enforcing them showed 3 further mismatches: profile-bad (now fixed), script-invalid-xml, upper-route).
- parse-formats still differs (SVG rounding); Script removal shutdown as above.
