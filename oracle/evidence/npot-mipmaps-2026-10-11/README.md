# NPOT mipmaps: the two known picture differences (2026-10-11)

Result: **not fixed, by decision.** Pictures stay at 294/296 on corpus-items2 (MSAA off and 4x
`--gl-samples`). The mipmap algorithm in `crates/x3d-render/src/texture.rs` is unchanged from
`e0f613e`. The comparison threshold is unchanged.

## Environment

| | |
| --- | --- |
| GPU | NVIDIA GeForce RTX 3060 Ti |
| driver | 595.91.07 |
| Chrome | 155.0.8059.39, `--use-gl=angle --use-angle=gl` |
| WebGL renderer | `ANGLE (NVIDIA Corporation, NVIDIA GeForce RTX 3060 Ti/PCIe/SSE2, OpenGL 4.5.0)` |
| X_ITE pin | `oracle/x_ite-pin`, commit `34cdf9f97` |
| Rust | `e0f613e` |
| threshold | `x3d-shot compare` default: a pixel is over when an RGB delta > 8; an image passes when ≤ 0.5% of pixels are over |

## Affected items

| item | image | mixed-parity levels the driver shifts |
| --- | --- | --- |
| `art/rainbow-room` | `art.jpg` 800×450 | 400×225, 25×14, 12×7 |
| `art/djascendance-live` | `djascendance-live.jpg` 509×339 | 254×169, 127×84, 63×42, 15×10 |

Picture result at `e0f613e` (`shot-compare-e0f613e.txt`):

| item | MSAA | max | over |
| --- | --- | --- | --- |
| rainbow-room | off | 82 | 2.539% |
| rainbow-room | 4x | 82 | 2.545% |
| djascendance-live | off | 94 | 1.847% |
| djascendance-live | 4x | 94 | 1.842% |

## What was measured

X_ITE uploads NPOT images as they are and calls `gl.generateMipmap` (X3DSingleTextureNode.js).
The pixels are the driver's, not X_ITE logic. `scripts/probe.mjs` reads every mip level back from
headless GPU Chrome (`gpu-levels/levels.tar.gz`: `rr.*`/`dj.*` are the GPU, `rs_*` are Rust).

Per level from the previous GPU level (`scripts/rule.py`, `grid-even-odd.txt`, `level1-flags.tsv`):

* Both sides even: 2-tap box `(2x, 2x+1)`. Rust matches (±1).
* Both sides odd: 3-tap polyphase filter. Rust matches (±1).
* One side even, one side odd: the odd side uses the 3-tap filter. The even side is **either** the
  normal box `(2x, 2x+1)` **or** a box shifted by one texel, `(max(2x−1, 0), 2x)`. The choice is the
  same for every texel of a level and depends only on the level size: 2,048 even/odd pairs
  (even 2…64 × odd 3…65, both orientations) gave 1,046 normal and 1,002 shifted, 0 other.
* The choice depends on the odd part of the even side (6, 12, 24, 48 share one pattern) and on
  the odd side. No float32 expression tried predicts it (`scripts/f32.py`). It is closed-driver
  behavior.

Level 0 also differs by up to 4 (JPEG decode: the `image` crate vs Chrome). That is within
tolerance on its own.

## Experiment (rejected)

A temporary switch forced the shifted box for exactly the 7 sizes above. Both items then passed
at MSAA off and 4x (max 3). The size list came from GPU output, so it is a fudge, fits one
driver only, and cannot cover all sizes. It was removed; nothing of it is committed.

## Re-run

The scripts were written in `/tmp/npot`; run them from a scratch copy, not from this directory.
`probe.mjs <image> <prefix>` and `probe2.mjs "$(cat sizes2.json)"` need the playwright in
`../../x_ite-tests` (path in the scripts) and write raw RGBA / `synth.json` to the current
directory. `npotdump-main.rs` + `npotdump-Cargo.toml` build the Rust side of `cmp.py`.
