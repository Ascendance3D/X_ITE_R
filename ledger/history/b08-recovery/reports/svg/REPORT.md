# Batch 08, lane svg: report

Worktree: `/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup` (called WT below).
Pinned X_ITE: `oracle/x_ite-pin` (16.5.0, read only). Builds: `CARGO_TARGET_DIR=/tmp/b08/svg/target`, `-j 8 --release`.

## Recommended status

| entry | recommendation | why not more |
| --- | --- | --- |
| `sub:parse.svg` | PARTIAL, close to COMPLETE | three things are open: colors that Chrome parses and the port does not (hwb, lab, lch, oklab, oklch, `color()`); 43 `hsl()` values one level off Chrome; one gradient pixel one level off (see Known limits). No field-change item applies (a parser has no runtime fields). |
| `sub:parse.html-support` | COMPLETE for the contract as it applies to a parser, subject to owner decision 3 | HTMLSupport only acts on a DOM element, and the port has no DOM. The port gets the same input through an HTML-lite fragment parser, written for this lane. Items 3, 5, 6, 7 of the contract do not apply to this entry. |

I did not touch the ledger.

## What was done

### sub:parse.svg
- `crates/x3d-parse/src/svg.rs` ports `SVGParser.js` (2,731 lines): all elements, `d` path commands, transforms, `style` and presentation attributes, units, root `viewBox`, `use`, `switch`, `a`, `image`, fill and stroke, libtess tessellation with the combine callback, curve subdivision, linear and radial gradients, the optimizer step.
- `crates/x3d-parse/src/svg_canvas.rs` draws the gradient canvas texture: Skia stop evaluation in 32-bit, Skia matrix chain (`SkMatrix` concat and invert with double intermediates, fused multiply-add in the matrix stage), dither with fused multiply-add, premultiply on store, unpremultiply on read, PNG encode and data URL. Chrome's canvas error texts are reproduced (non-finite value, negative radius).
- `crates/x3d-text/src/libtess.rs`: new `triangulate_svg` entry that returns triangles and the combine vertices, with the SVG callback.
- `crates/x3d-parse/src/color.rs`: new `try_convert_color`, hsl fixed to Blink's 32-bit rounding, percent channels, hue units (`deg`, `grad`, `rad`, `turn`).
- Format dispatch: SVG is last in GoldenGate order and accepts only an XML document whose root is `svg` (`lib.rs`).
- Quality options: `PrimitiveQuality` and `TextureQuality` reach the parser through `Runtime::sync_svg_quality` (new, `inline.rs`), called before an Inline scene and an EXTERNPROTO scene are parsed.

Fixes found by running against X_ITE (each one is an X_ITE quirk or a port bug that the tests caught):
- `OrthoViewpoint.fieldOfView` is an MFFloat of four values that prints as an SFVec4f. The port had set an SFVec4f, so the renderer saw the default extent. Text was equal both ways. The render comparison found it.
- Nodes made by the SVG parser were not in `scene.setup_order`, so a gradient `ImageTexture` never asked for its url in the runtime. They are now added in creation order (X_ITE sets nodes up as `createNode` makes them).
- `LocalFetcher` could not fetch a `data:` URL; it now decodes it like a browser's `fetch`.
- `convertColor` keeps one shared `div`: an invalid color keeps the previous inline color, `currentcolor` resolves to the wrapper color. Reproduced with `div_color`.
- Degenerate linear gradient paints nothing in Chrome (the port painted the last stop color).

### sub:parse.html-support
- `crates/x3d-parse/src/html_support.rs` ports `HTMLSupport.js`: the node type map (`TypeName` and `TYPENAME` to `TypeName`, `ProtoInstance` first) and the field map (every `initializeOnly` or `inputOutput` field and its aliases, lower-case and exact case). The maps are global, as in X_ITE; a node that does not own the field drops it later.
- `crates/x3d-parse/src/html.rs`: an HTML-lite fragment parser (upper-case element names, lower-case attribute names, first duplicate attribute wins, character references, raw text elements, void elements, `html`/`head`/`body` ignored, comments, bogus comments). Public entry `parse_html_x3d(html, world_url)`: finds the first `<x3d>` as `div.innerHTML = text; browser.importDocument(div.querySelector("x3d"))` does.
- `xml.rs` / `dom.rs`: the XML parser has an HTML mode (`parse_x3d_html_element`): mapped element and attribute names, PROTO and EXTERNPROTO names added to the maps, `head` handled on the x3d element itself, `USE` mapped, SCRIPT re-parsed as XML (`scriptElement`).
- Quirk reproduced: `scriptElement` walks a live `childNodes` list while `append` moves each element or CDATA out of it, so every second child is skipped. A two-child script loses its CDATA source. Cases `script-cdata`, `script-three-children`, `script-spaced-children`.
- HTMLSupport does nothing for an XML document: 12 twin files (same names in `.x3d`) confirm that upper-case and lower-case names are rejected there with X_ITE's console text.

## Evidence against pinned X_ITE

Baselines are written only from X_ITE runs (never from Rust output):
- `oracle/synthetic-rea-svg/svg-baseline.json` (`node oracle/x_ite-svg.mjs synthetic-rea-svg synthetic-rea-svg/svg-baseline.json`), 269 inputs: 257 load, 12 are rejected with their console text.
- `oracle/synthetic-rea-svg/css-colors-baseline.json` (`oracle/x_ite-css-colors.mjs`): 16,419 color strings, Chrome's computed values.
- `oracle/synthetic-rea-svg-quality/baseline-{low-low,high-high,low-high}.json` (`SVG_QUALITY=LOW,HIGH node oracle/x_ite-svg.mjs ...`): 16 inputs at three option pairs.
- `oracle/render-rea-b08-svg-baseline`, `oracle/render-msaa4-rea-b08-svg-baseline` (`oracle/x_ite-svg-shot.mjs`, `x_ite-svg-shot-msaa.mjs`): 25 renders each.
- `oracle/synthetic-rea-html/html-baseline.json` (`x_ite-html.mjs`, 53 `.html` inputs) and `oracle/synthetic-rea-html/xml-baseline.json` (`x_ite-formats.mjs` on `synthetic-rea-html/xml`, 12 inputs).
- Generators: `oracle/tools/gen_rea_b08_svg.py`, `gen_rea_b08_html.py` (synthetic only, seeded random for the tessellation stress set). No archive was read.

Results (all on the final code):

| test | result |
| --- | --- |
| `svg_matches_x_ite` | 1,095 checks, 0 failed: scene text in VRML, XML and JSON equal for all 257 loaded inputs, encoding equal, 12 rejections with the same console text. 55 gradient textures compared by decoded pixels: max channel difference 1, worst file 7 of 65,536 pixels (`grad-radial-focal.svg`). |
| `svg_quality_options_match_x_ite` | 3 option pairs x 69 checks, 0 failed. Worst texture: 6 of 262,144 pixels one level off. |
| `render_rea_b08_svg_renders_match_x_ite` | 50 renders (25 at MSAA 0, 25 at MSAA 4, `--gl-samples` orientation), all within 0.5% over 8 levels. Worst: `shape-rect-rounded.svg` at MSAA 0, 488 pixels = 0.186%. At MSAA 4 the worst has 9 pixels (0.003%), max level 64. |
| `html_support_matches_x_ite` | 325 checks, 0 failed: 53 HTML inputs and 12 XML twins, text in three formats and the console log. |
| `css_colors_match_chrome` | 16,419 values: 43 `hsl()` values one level off (known, see below), 0 other failures; 6 forms not compared (see below). |
| `svg_b08_lifecycle` (2 tests) | see lifecycle. |

PNG normalization in the SVG test: the data URL of a gradient is decoded on both sides and compared by pixel (limit: max channel difference 1 and at most 0.1% of the pixels different), then replaced by `PNG <w>x<h>` for the text comparison. Nothing else is normalized. I did not widen any tolerance.

Console log comparison for HTML: X_ITE's lines carry `warn: ` or `error: ` and the catch blocks add `Error: `; `Scene::warnings` holds the message only and once per distinct message. The test strips the level and the prefixes and compares the sorted distinct messages. The levels and X_ITE's duplicate logging of one failure (for example `Bad ROUTE specification` is logged once as error and once as warning) are not compared. For the world URL of an HTML element the test passes the page URL, as X_ITE does.

## Lifecycle and memory

`crates/x3d-viewer/tests/svg_b08_lifecycle.rs`:
- An Inline holding an SVG (two gradients, curves, evenodd polygon, `use`, transforms) is unloaded (`load FALSE`) and loaded again 100 times, alternating a large and a small SVG document. Tables after cycle 10, 50, 100 are the same: live nodes 49, arena 50, node runtime states 49, field runtime states 521, contexts 2. Heap bytes from a counting allocator: 3,174,377 / 3,174,409 / 3,174,409 (run alone). RSS 18,192 KiB at all three.
- 100 parses of the two documents (no runtime): 2,837,210 / 2,837,242 / 2,837,242 bytes.
- Under the default parallel harness the heap differs by a few hundred bytes (captured output, other test thread), so the test bound is 2,048 bytes from cycle 50 to 100, 64 KiB from 10 to 50 (inline test), 4,096 bytes (parse test).

## Known limits (with numbers and reasons)

1. `hsl()`: 43 of the 16,200 `hsl` values in the grid differ from Chrome by one level in one channel. They are the values where the channel is exactly x.5 in real arithmetic and Blink's 32-bit evaluation lands just below it. The port scales in 32-bit after a 64-bit evaluation. I searched 560 combinations of arithmetic order and precision and none reaches 0; 43 is the minimum found. The count is asserted in `css_colors_b08.rs`, so it cannot change unnoticed.
2. Colors the port does not parse and Chrome does: `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()`, `color()`. `try_convert_color` returns `None`, so an SVG paint using them takes the previous or wrapper color. The shared `convert_color` used by the VRML and XML parsers has the same gap. Not compared in the css test: `currentcolor`, `inherit` (resolved by context), and those forms.
3. One gradient pixel: `grad-radial-focal.svg` pixel (40, 72), X_ITE 52,52,154 and Rust 53,53,154 (the two-point-conical solve rounds the other way at one point).
4. Text and Font are not involved; `text` elements are ignored by SVGParser as in X_ITE.
5. Missing image files of an SVG `image` element (four fixtures) log a runtime error in X_ITE; the SVG parse tests do not compare that runtime log (it is not produced by the parser). The render of `struct-use-image.svg` is equal.
6. HTML-lite parser: not the full HTML5 tree construction (no implied end tags, tables, `<p>`, adoption agency). X3D elements are unknown elements, so plain container handling is what X3D needs. The 53 fixtures cover names, fields, DEF/USE, ROUTE, PROTO, EXTERNPROTO, Script, head, comments, entities and raw text.

## X_ITE quirks reproduced
- `div` color persistence in `convertColor`.
- Live `childNodes` skip in `scriptElement`.
- `OrthoViewpoint.fieldOfView` as MFFloat printed as SFVec4f.
- Root `viewBox` and unit rules, `use` cache keys, `switch` handling, `image` url `null`, optimizer merging the ViewBox transform of small documents (the name `ViewBox` is gone then; the big document keeps it).
- Canvas gradient errors: `createLinearGradient` non-finite value; `createRadialGradient` radius below 0 (text as Chrome prints it, without the value).
- A degenerate linear gradient paints nothing.
- HTMLSupport field map includes `inputOutput` fields; the name maps are global per type name.

## Files

New (WT):
- `crates/x3d-parse/src/svg.rs`, `svg_canvas.rs`, `html.rs`, `html_support.rs`
- `crates/x3d-parse/tests/svg_rea.rs`, `html_support_rea.rs`, `css_colors_b08.rs`
- `crates/x3d-render/tests/render_rea_b08_svg.rs`
- `crates/x3d-viewer/tests/svg_b08_lifecycle.rs`
- `oracle/x_ite-svg.mjs`, `x_ite-html.mjs`, `x_ite-css-colors.mjs`, `x_ite-svg-shot.mjs`, `x_ite-svg-shot-msaa.mjs`
- `oracle/tools/gen_rea_b08_svg.py`, `gen_rea_b08_html.py`
- data dirs: `oracle/synthetic-rea-svg`, `synthetic-rea-svg-quality`, `synthetic-rea-svg-render`, `synthetic-rea-html`, `render-rea-b08-svg-baseline`, `render-msaa4-rea-b08-svg-baseline`

Shared files (small hunks, files were already dirty from other work; re-read before each edit):
- `crates/x3d-parse/src/lib.rs`: `mod html; mod html_support; mod svg; mod svg_canvas;`, two `pub use`, the SVG dispatch step (was "SVGParser: not ported"), keep the root element for it.
- `crates/x3d-parse/src/dom.rs`: `Element.html`, `Element.script_error`, case-insensitive `attr` when `html`.
- `crates/x3d-parse/src/xml.rs`: HTML mode (`parse_x3d_html_element`, `parse_x3d(root, url, html)`, name mapping, SCRIPT handling).
- `crates/x3d-parse/src/color.rs`: `try_convert_color`, hsl, percent and hue-unit fixes.
- `crates/x3d-parse/Cargo.toml`: internal dependency `x3d-text` (no new external crate; `Cargo.lock` updated by cargo).
- `crates/x3d-text/src/libtess.rs`: `svg` fields in `Tess`, combine branch, `triangulate_svg`.
- `crates/x3d-runtime/src/inline.rs`: `sync_svg_quality` and its call; `extern_proto.rs`: one call; `loader.rs`: `LocalFetcher` decodes `data:` URLs.

No `pub mod` added to `x3d-fields` or `x3d-scene`. No baseline, existing test or other lane's file edited. No NURBS code touched. Nothing committed, staged or stashed.

## Commands

```
cd WT; export CARGO_TARGET_DIR=/tmp/b08/svg/target CARGO_BUILD_BUILD_DIR=/tmp/b08/svg/build
cargo test --release -j 8 -p x3d-parse --test svg_rea --test html_support_rea --test css_colors_b08
cargo test --release -j 8 -p x3d-render --test render_rea_b08_svg
cargo test --release -j 8 -p x3d-viewer --test svg_b08_lifecycle -- --nocapture --test-threads=1   # exact MEM lines
SVG_ONLY=grad- SVG_PIXELS=1 cargo test --release -p x3d-parse --test svg_rea -- --nocapture         # pixel differences
SVG_OUT=/tmp/svgout cargo test --release -p x3d-parse --test svg_rea                                 # .rust/.xite files per difference
# baselines (X_ITE only; each needs a few minutes, use a free machine)
node oracle/x_ite-svg.mjs synthetic-rea-svg synthetic-rea-svg/svg-baseline.json
node oracle/x_ite-css-colors.mjs <out.json>
SVG_QUALITY=LOW,HIGH node oracle/x_ite-svg.mjs synthetic-rea-svg-quality synthetic-rea-svg-quality/baseline-low-high.json
node oracle/x_ite-svg-shot.mjs oracle/synthetic-rea-svg-render oracle/render-rea-b08-svg-baseline
node oracle/x_ite-svg-shot-msaa.mjs oracle/synthetic-rea-svg-render oracle/render-msaa4-rea-b08-svg-baseline
node oracle/x_ite-html.mjs synthetic-rea-html synthetic-rea-html/html-baseline.json
node oracle/x_ite-formats.mjs synthetic-rea-html/xml synthetic-rea-html/xml-baseline.json
```

Workspace run (`cargo test --release -j 8 --workspace --no-fail-fast`, 157 test binaries ok): the only failing binaries are other lanes' work in progress (`render_rea_b08_shaders`, `multi_viewer_b08`, `rea_b08_net_reload`, `rea_b08_shaders_reload`, `rea_b08_vs_reload`, `rea_batch08_net`, `rea_batch08_shaders`) plus my lifecycle test, which failed once under the parallel harness before I set the bounds above and now passes in repeated runs. None of those failures touches SVG, HTML, color, `LocalFetcher` data URLs or the quality hook. I did not fix them.

## Owner decisions needed
1. Should the shared `convert_color` get the CSS Color 4 functions (hwb, lab, lch, oklab, oklch, `color()`)? It is a general gap that SVG exposes. Needed for COMPLETE of `sub:parse.svg` if the owner wants every Chrome color accepted.
2. Accept 43 `hsl()` one-level ties as a known limit, or ask for a bit-exact Blink color port (float color types)?
3. HTMLSupport has no runtime of its own here: should the entry be judged on the parser contract (items 2, 4 applied as parse and lifecycle) or does a DOM-integration layer (live attribute changes, `browser.importDocument` on a live document) have to exist before COMPLETE?
4. `LocalFetcher` now decodes `data:` URLs (a browser does, and X_ITE's SVG gradients depend on it). Confirm that a change to this shared file by this lane is acceptable.
5. The one-pixel radial-focal difference: accept at the measured limit (max 1 level, 7 of 65,536 pixels), or investigate the conical solve further?
