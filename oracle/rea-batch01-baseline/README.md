# REA batch 01 oracle baselines

X_ITE (pinned `x_ite-pin`, headless Chrome, ANGLE on OpenGL 4.5, RTX 3060 Ti) results for the
REA batch 01 worlds, written by `oracle/tools/x10_compare.py` (the oracle half:
`x_ite-anim.mjs` / `x_ite-anim-msaa.mjs`). `cases.txt` lists `name world script msaa`; each
`<name>-msaa<N>/js.json` is the oracle result, `js/*.png` its captured frames.

`crates/x3d-viewer/tests/rea_batch01.rs` runs `x3d-drive` for each case and compares with the
x10_compare rules: scene text exact, poses within 1e-4, the sorted `log` exact, images with
threshold 8 and at most 0.5% of the pixels over it. The oracle's `console` list starts after
the world is parsed, so parse-time warnings are only in `log`.

All worlds are authored for this batch; `checker64.png` / `checker8.png` are self-made.
