# Open comparisons (not in cases.txt, so no test runs them)

Each directory holds one pinned X_ITE 16.5.0 recording (`js.json`, three identical runs) of a world in `oracle/synthetic-rea-b09-sai`
where the port differs and the difference is not yet pinned by a test. `rea_b09_sai.py case` adds a case to `cases.txt` only when the
port equals the page, so these are kept here until the port is fixed or the difference is approved.

- `externproto-after-load` (world `externproto-after-load.wrl`, script `sai.json`, entry `sub:proto.externproto`):
  `Browser.createX3DFromString` after load, with an EXTERNPROTO. Final values equal at frame 60. Open: a scene-cache hit on a loaded
  `data:` url (cases dataX3d, twoUrls, hash) resolves the promise one frame earlier in the port (frames 11, 17, 20 against 12, 18, 21).
  The log differs in one line: the pinned page reports `TypeError: Cannot read properties of null (reading 'setWorldURL')` where a
  cached failed url is asked again (a documented difference: the port does not cache a failure, extern_proto.rs).
  Compare: `python3 -I oracle/tools/x10_compare.py WORLD oracle/synthetic-rea-b09-sai/sai.json /tmp/out --no-images --target <release dir>`.
