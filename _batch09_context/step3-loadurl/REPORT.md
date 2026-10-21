# Step 3: Browser.loadURL / replaceWorld / Anchor click

Worktree integration-b06-b08, branch integration/xite-rust-b06-b08-final, local commit only (nothing pushed).
Gate: `cargo test --workspace --release -j 12 --no-fail-fast` = 994 passed / 0 failed / 1 ignored (baseline 994/0/1).
script_shutdown_kept_difference.rs still passes (dispose_scene still calls Script shutdown; owner-approved).

## Implemented
- In-place world switching following pinned X_ITE (X3DBrowser.replaceWorld/loadURL timeline): `Scene.world` names the live world
  context (ROOT stays the shell); a new world is a merged context promoted to the world. All "ROOT is the world" sites were
  switched to `scene.world`.
- `Browser.replaceWorld(scene | null | undefined)`, `Browser.loadURL(urls, parameter)` with promise resolution, fallback urls,
  X_ITE error texts ("Couldn't load X3D file.", "Loading of X3D file aborted.", "Replacing world aborted."), superseded loads,
  and frame-exact ordering: SHUTDOWN callback, dispose of the old world, new world live, loadCount event, INITIALIZED callback.
- Browser callbacks: addBrowserCallback/removeBrowserCallback/getBrowserCallbacks with INITIALIZED_EVENT (1003), SHUTDOWN_EVENT
  (1004), INITIALIZED_ERROR (1005). Browser.initialized()/shutdown(), changeViewpoint(name), baseURL/getBaseURL.
- Hash-only url (and `file#Name` start viewpoint) binds a viewpoint; bound stacks, navigation info, description and the viewer pose
  reset after a switch, with the X_ITE one-phase lag (HeldPose).
- A browser scene's loadCount now counts pending Script/texture url objects (X3DUrlObject.setLoadState), which fixed createX3DFromString
  of scenes containing a Script resolving a frame early.
- Anchor click (Anchor.js requestImmediateLoad): the viewer now calls `Runtime::anchor_load` (own FileLoader with url + parameter):
  scene -> replaceWorld (no loadURL promise, no INITIALIZED_ERROR), failure -> nothing else, hash -> changeViewpoint. Hover also
  sets Browser.description (TouchSensor set_over__).

## Tests and baselines
- crates/x3d-viewer/tests/rea_batch09_sai.rs: 21 new cases in oracle/rea-batch09-sai-baseline (+cases.txt), generator
  oracle/tools/gen_rea_b09_loadurl.py: loadurl-{ok,fail,fail2,fallback,param,empty,badargs,badscene,chain,twice,then-replace,
  hash-start,inline,viewpoint}, replaceworld-{null,none,number,string,nodes,scene,twice}. Each Script probe records into its own
  field; scene text, sorted console (with level) and poses compared exactly. X_ITE side repeated 3 times: deterministic.
- crates/x3d-viewer/tests/rea_batch09_inputs.rs: 5 stepped cases (msaa 0) in oracle/rea-batch09-inputs-baseline:
  anchor-load-{ok,fail,param,hash,disabled} (generator oracle/tools/gen_rea_b09_anchor_load.py, settle true).
- Existing viewer test anchor_activation_is_reported_not_opened still passes (status line keeps the Anchor message).

## Owner-scope boundary (not DIFFERENT)
Browser DOM importDocument stays rejecting with an error. A foreign-target loadURL (_blank, other window) or an .html/.xhtml url
is out of native scope: the runtime only records a "foreign:<target>:<href>" request and resolves the promise; the viewer opens nothing.

## Remaining differences
1. Anchor with a missing first url then a good one (A2 fallback): pinned X_ITE shows the new world one frame EARLIER (7) than
   after a good first url (8), 3 of 3 runs; Rust shows it at 8. Depends on real fetch timing, not modeled; no case added (not fudged).
2. Browser.baseURL is the embedder page URL in X_ITE; Rust uses a page URL configured by the embedder (oracle harness sets the
   oracle page), else falls back to the calling world's URL. Embedder-dependent.
3. `Browser.worldURL` is undefined in both. `sceneGraph_changed` carries a real-clock value in X_ITE and is not implemented.
4. Dispose-time Script shutdown (kept, owner-approved).
5. Falsy-thrown-value diagnostics, etc. are unrelated to this lane and untouched.

## Ledger recommendations (recommendation only)
- sub:browser.sai-api: evidence = the 21 sai cases above (loadURL/replaceWorld success, failure, fallback, abort, callbacks,
  bound-stack reset). Recommend COMPLETE for loadURL/replaceWorld; keep PARTIAL overall only for sceneGraph_changed and
  embedder-dependent baseURL, if the entry still lists them; importDocument/window.open are native-scope exclusions.
- node:Anchor: evidence = anchor-load-{ok,fail,param,hash,disabled} plus existing anchor-click/anchor-drag-off. Recommend COMPLETE
  with the documented fallback-timing difference (item 1) and the foreign-target exclusion.
- node:Inline: evidence = loadurl-inline (Inline inside a loaded world, loadCount/initialization ordering) plus prior inline cases.
  Recommend leaving the existing status unless it was PARTIAL only for world switching, in which case COMPLETE.
