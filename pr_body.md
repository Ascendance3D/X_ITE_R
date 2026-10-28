Refs #129

### Mandatory Reverse Engineer Anything (REA) Evidence
The main agent supplied it; `analyze_javascript_application` on `oracle/x_ite-pin/src` (sha256 15630b97...afb9), source_paths `x_ite/InputOutput/FileLoader.js` and `x_ite/Parser/GLTF2Parser.js`, evidence ev_759732bd967ba7aa04edd1eec626f67bfae70908012f999fb6577a289d74c0a7: 2 files parsed, 0 parse failures, coverage partial. Trace of api "fetch" (ev_274db38a...5a41): no match; a no-match is not evidence of absence. The upstream behavior rests on direct reading of `oracle/x_ite-pin/src/x_ite/Parser/GLTF2Parser.js:600-614`.

### Ledger effect
**sub:parse.gltf2**: Optimizes GLTF buffer failure checks by preventing the cloning and immediate dropping of successful body bytes in `follow_buffers`, improving memory efficiency.

### Skipped tests
- GPU tests
- Gitignored answer keys
