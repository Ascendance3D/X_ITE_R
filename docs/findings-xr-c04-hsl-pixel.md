# Findings Report: HSL and Radial Gradient Pixel Mismatches (XR-C04)

## Observed (reproduced by the script or read from cited files)

### Pinned References
- **X_ITE**: The reference codebase (`oracle/x_ite-pin`, commit `34cdf9f97`) does not parse `hsl()` internally through canvas fillStyle. It sets a CSS property on an element and reads back the computed `rgb()`.
  - `oracle/x_ite-pin/src/x_ite/Parser/X3DParser.js`:82-86: The styles are set at lines 82-83 and read using `window .getComputedStyle (this .div) .color` at line 86.
  - `oracle/x_ite-pin/src/x_ite/Parser/SVGParser.js`:2461: SVG elements convert colors using `this.convertColor()`.
  - `oracle/x_ite-pin/src/x_ite/Parser/SVGParser.js`:1007: `this .context .createRadialGradient(...)` builds the radial gradient from SVG values.
- **Rust Port**:
  - `crates/x3d-parse/src/color.rs`:235-263: The port implements HSL conversion using `f64` arithmetic and scales to `f32` inside a custom rounding step.
  - `crates/x3d-parse/src/svg_canvas.rs`:414-463: The radial gradient evaluation uses a quadratic intersection root via `two_point_conical`.

### Reproducible Counts
A reproduction script is provided at `docs/findings-xr-c04/hsl_count.py` (which takes the oracle JSON path `oracle/synthetic-rea-svg/css-colors-baseline.json` as an argument). It asserts the following counts out of 16,200 grid values compared to the Oracle:
- `rust_current`: 43 mismatches.
- `hypothesis_f32_legacy`: 114 mismatches.
- `hypothesis_f32_gfx`: 204 mismatches.
- `hypothesis_f64_legacy`: 508 mismatches.
- `hypothesis_f64_gfx`: 562 mismatches.

### The 43 Verified Half-Channel Cases
The reproduction script verified that every one of the 43 mismatches in the current Rust port is an interpolated channel whose exact real value is `x.5`, and Chrome (oracle) rounded it DOWN.

The full list outputted by the script:
| Input | Channel | Exact Scaled Value | Chrome (Oracle) Value | Rust Value |
|---|---|---|---|---|
| `hsl(10,80%,50%)` | G | 59.5 | 59 | 60 |
| `hsl(10,100%,50%)` | G | 42.5 | 42 | 43 |
| `hsl(14,50%,50%)` | G | 93.5 | 93 | 94 |
| `hsl(14,100%,50%)` | G | 59.5 | 59 | 60 |
| `hsl(25,80%,50%)` | G | 110.5 | 110 | 111 |
| `hsl(42,100%,50%)` | G | 178.5 | 178 | 179 |
| `hsl(46,100%,50%)` | G | 195.5 | 195 | 196 |
| `hsl(125,50%,40%)` | B | 59.5 | 59 | 60 |
| `hsl(125,50%,60%)` | B | 110.5 | 110 | 111 |
| `hsl(125,80%,50%)` | B | 42.5 | 42 | 43 |
| `hsl(140,80%,50%)` | B | 93.5 | 93 | 94 |
| `hsl(140,100%,25%)` | B | 42.5 | 42 | 43 |
| `hsl(155,50%,40%)` | B | 110.5 | 110 | 111 |
| `hsl(155,80%,50%)` | B | 144.5 | 144 | 145 |
| `hsl(162,100%,50%)` | B | 178.5 | 178 | 179 |
| `hsl(166,100%,50%)` | B | 195.5 | 195 | 196 |
| `hsl(170,80%,50%)` | B | 195.5 | 195 | 196 |
| `hsl(170,100%,50%)` | B | 212.5 | 212 | 213 |
| `hsl(190,80%,50%)` | G | 195.5 | 195 | 196 |
| `hsl(190,100%,50%)` | G | 212.5 | 212 | 213 |
| `hsl(194,100%,50%)` | G | 195.5 | 195 | 196 |
| `hsl(198,100%,50%)` | G | 178.5 | 178 | 179 |
| `hsl(205,50%,40%)` | G | 110.5 | 110 | 111 |
| `hsl(205,80%,50%)` | G | 144.5 | 144 | 145 |
| `hsl(220,80%,50%)` | G | 93.5 | 93 | 94 |
| `hsl(220,100%,25%)` | G | 42.5 | 42 | 43 |
| `hsl(235,50%,40%)` | G | 59.5 | 59 | 60 |
| `hsl(235,50%,60%)` | G | 110.5 | 110 | 111 |
| `hsl(235,80%,50%)` | G | 42.5 | 42 | 43 |
| `hsl(250,80%,50%)` | R | 59.5 | 59 | 60 |
| `hsl(250,100%,50%)` | R | 42.5 | 42 | 43 |
| `hsl(254,50%,50%)` | R | 93.5 | 93 | 94 |
| `hsl(254,100%,50%)` | R | 59.5 | 59 | 60 |
| `hsl(265,80%,50%)` | R | 110.5 | 110 | 111 |
| `hsl(282,100%,50%)` | R | 178.5 | 178 | 179 |
| `hsl(284,100%,25%)` | R | 93.5 | 93 | 94 |
| `hsl(316,100%,25%)` | B | 93.5 | 93 | 94 |
| `hsl(318,100%,50%)` | B | 178.5 | 178 | 179 |
| `hsl(335,80%,50%)` | B | 110.5 | 110 | 111 |
| `hsl(346,50%,50%)` | B | 93.5 | 93 | 94 |
| `hsl(346,100%,50%)` | B | 59.5 | 59 | 60 |
| `hsl(350,80%,50%)` | B | 59.5 | 59 | 60 |
| `hsl(350,100%,50%)` | B | 42.5 | 42 | 43 |

### Radial Gradient Pixel
For `grad-radial-focal.svg` at pixel coordinates (40, 72):
- Oracle (X_ITE) RGBA: `[52, 52, 154, 255]`
- Rust RGBA: `[53, 53, 154, 255]`
- Mismatch Count: The REA Batch 08 SVG report limits table lists "7 of 65,536 pixels" for this file. Issue #55 explicitly cites "one radial-focal gradient pixel".

## Hypotheses (UNVERIFIED)

Because the exact upstream source code mapping to the Oracle environment cannot be verified in this repository, these represent purely structural mathematical hypotheses intended to align Chrome's output to standard C++ implementations:
- It is hypothesized that Chromium evaluates CSS `hsl()` via CSS parsing (`css_color_value.cc`) and color conversions (`Color::FromHSLA`, `CalcHue`).
- It is hypothesized that Chromium's HSL-to-RGB conversion could utilize `gfx::HSLToSRGB` (`ui/gfx/color_conversions.cc`) or a legacy path using float domains.
- It is hypothesized that the 8-bit channel serialization likely executes via `std::round` combined with `Sk4f_toL32` (or `SkColor4f::toSkColor`).
- It is hypothesized that Chromium's float precision execution path dynamically rounds channels exactly evaluated at `x.5` *down* to `x` instead of tracking away from zero.
- It is hypothesized that the Skia pixel gradient logic heavily relies on `SkTwoPointConicalGradient` (utilizing `FocalData` and `appendGradientStages`) and specific hardware optimization tracks scaling over stages like `xy_to_2pt_conical_focal_on_circle`.

## Unresolved
1. **Mathematical discrepancies in exact replication limits:** The python script reproduces varying counts depending on arithmetic mapping (producing 114, 204, 508, 562 for different strict float evaluation permutations). The owner reported alternate numbers originally for differing variations (`legacy_f32`: 117, `gfx_f32`: 473, `legacy_f64`: 915, `gfx_f64`: 1263). The true single mathematical root matching Chromium across the board remains unresolved.
2. **Gradient Pixel Count limit reporting:** Issue #55 cites "one radial-focal gradient pixel" differing, while the formal Batch 08 Report limits table recorded "7 of 65,536 pixels" differing for `grad-radial-focal.svg`. This discrepancy in reporting numbers remains unresolved.
