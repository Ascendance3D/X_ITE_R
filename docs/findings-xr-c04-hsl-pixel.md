# Findings Report: HSL and Radial Gradient Pixel Mismatches (XR-C04)

Refs #55. This report records measurements and open questions. It does not change renderer code, oracle baselines or tolerances.

## Observed (reproduced by the script or read from cited files)

### Pinned references

- **X_ITE** (`oracle/x_ite-pin`, commit `34cdf9f97`, 16.5.0) does not convert `hsl()` itself. It sets the CSS `color` property on an element and reads back the computed `rgb()` value.
  - `oracle/x_ite-pin/src/x_ite/Parser/X3DParser.js:82-86`: lines 82-83 set the styles; line 86 reads `window .getComputedStyle (this .div) .color`.
  - `oracle/x_ite-pin/src/x_ite/Parser/SVGParser.js:2461`: SVG elements convert colors with `this.convertColor()`.
  - `oracle/x_ite-pin/src/x_ite/Parser/SVGParser.js:1007`: `this .context .createRadialGradient(...)` builds the radial gradient from the SVG values.
- **Rust port**:
  - `crates/x3d-parse/src/color.rs:235-263`: the port computes each channel in `f64`, casts it to `f32`, multiplies by `255.0f32`, then rounds half up with `floor(x + 0.5)` in `f64` (line 263).
  - `crates/x3d-parse/src/svg_canvas.rs:414-463`: the radial gradient uses a quadratic root in `two_point_conical`.

### Reproducible counts

Run:

```
python3 docs/findings-xr-c04/hsl_count.py oracle/synthetic-rea-svg/css-colors-baseline.json
```

The script compares 16,202 `hsl()` values with the oracle (16,200 unitless hues, one `deg` hue and one `turn` hue). It exits 1 if any count below changes; it compares exact integers and exact rationals only, not rounded display values.

| Formula | Mismatched values |
|---|---|
| `rust_current` | 43 |
| `hypothesis_f32_legacy` | 114 |
| `hypothesis_f32_gfx` | 204 |
| `hypothesis_f64_legacy` | 508 |
| `hypothesis_f64_gfx` | 562 |

The four `hypothesis_*` formulas are experiments written for this report. They are not copies of Chromium or Skia code, and their names do not claim that Chromium uses them:

- `hypothesis_f32_legacy`: hue in sixths (`h / 60`), every step rounded to `f32`, final value rounded half away from zero.
- `hypothesis_f32_gfx`: hue in turns (`h / 360`) with the thresholds `6h < 1`, `2h < 1`, `3h < 2`, every step rounded to `f32`, final value rounded half up.
- `hypothesis_f64_legacy`: the `f32_legacy` steps in `f64`.
- `hypothesis_f64_gfx`: the `f32_gfx` steps in `f64`.

No formula gives 0 mismatches. The cause of the 43 differences is not found.

### Exact half-channel values

The script also computes every channel in exact rational arithmetic (CSS Color 4 `hsl()` to `rgb()`, scaled by 255). 5,946 channels have an exact value of `x.5`; Chrome rounds 5,903 of them up and 43 of them down.

The 43 rounded-down channels are exactly the 43 Rust mismatches. In each, Chrome has `x` and Rust has `x + 1`. Thus a fixed "round x.5 down" rule does not explain Chrome's output.

| Input | Channel | Exact scaled value | Chrome (oracle) | Rust |
|---|---|---|---|---|
| `hsl(10,80%,50%)` | G | 119/2 | 59 | 60 |
| `hsl(10,100%,50%)` | G | 85/2 | 42 | 43 |
| `hsl(14,50%,50%)` | G | 187/2 | 93 | 94 |
| `hsl(14,100%,50%)` | G | 119/2 | 59 | 60 |
| `hsl(25,80%,50%)` | G | 221/2 | 110 | 111 |
| `hsl(42,100%,50%)` | G | 357/2 | 178 | 179 |
| `hsl(46,100%,50%)` | G | 391/2 | 195 | 196 |
| `hsl(125,50%,40%)` | B | 119/2 | 59 | 60 |
| `hsl(125,50%,60%)` | B | 221/2 | 110 | 111 |
| `hsl(125,80%,50%)` | B | 85/2 | 42 | 43 |
| `hsl(140,80%,50%)` | B | 187/2 | 93 | 94 |
| `hsl(140,100%,25%)` | B | 85/2 | 42 | 43 |
| `hsl(155,50%,40%)` | B | 221/2 | 110 | 111 |
| `hsl(155,80%,50%)` | B | 289/2 | 144 | 145 |
| `hsl(162,100%,50%)` | B | 357/2 | 178 | 179 |
| `hsl(166,100%,50%)` | B | 391/2 | 195 | 196 |
| `hsl(170,80%,50%)` | B | 391/2 | 195 | 196 |
| `hsl(170,100%,50%)` | B | 425/2 | 212 | 213 |
| `hsl(190,80%,50%)` | G | 391/2 | 195 | 196 |
| `hsl(190,100%,50%)` | G | 425/2 | 212 | 213 |
| `hsl(194,100%,50%)` | G | 391/2 | 195 | 196 |
| `hsl(198,100%,50%)` | G | 357/2 | 178 | 179 |
| `hsl(205,50%,40%)` | G | 221/2 | 110 | 111 |
| `hsl(205,80%,50%)` | G | 289/2 | 144 | 145 |
| `hsl(220,80%,50%)` | G | 187/2 | 93 | 94 |
| `hsl(220,100%,25%)` | G | 85/2 | 42 | 43 |
| `hsl(235,50%,40%)` | G | 119/2 | 59 | 60 |
| `hsl(235,50%,60%)` | G | 221/2 | 110 | 111 |
| `hsl(235,80%,50%)` | G | 85/2 | 42 | 43 |
| `hsl(250,80%,50%)` | R | 119/2 | 59 | 60 |
| `hsl(250,100%,50%)` | R | 85/2 | 42 | 43 |
| `hsl(254,50%,50%)` | R | 187/2 | 93 | 94 |
| `hsl(254,100%,50%)` | R | 119/2 | 59 | 60 |
| `hsl(265,80%,50%)` | R | 221/2 | 110 | 111 |
| `hsl(282,100%,50%)` | R | 357/2 | 178 | 179 |
| `hsl(284,100%,25%)` | R | 187/2 | 93 | 94 |
| `hsl(316,100%,25%)` | B | 187/2 | 93 | 94 |
| `hsl(318,100%,50%)` | B | 357/2 | 178 | 179 |
| `hsl(335,80%,50%)` | B | 221/2 | 110 | 111 |
| `hsl(346,50%,50%)` | B | 187/2 | 93 | 94 |
| `hsl(346,100%,50%)` | B | 119/2 | 59 | 60 |
| `hsl(350,80%,50%)` | B | 119/2 | 59 | 60 |
| `hsl(350,100%,50%)` | B | 85/2 | 42 | 43 |

### Radial gradient pixel

- `grad-radial-focal.svg`, pixel (40, 72): X_ITE `52, 52, 154`, Rust `53, 53, 154` (`_batch08_context/reports/svg/REPORT.md:75`).
- The same Batch 08 report gives "max channel difference 1, worst file 7 of 65,536 pixels (`grad-radial-focal.svg`)" (`_batch08_context/reports/svg/REPORT.md:53`). Its owner question calls this "the one-pixel radial-focal difference" and gives "7 of 65,536 pixels" (`:137`).
- Issue #55 says "one radial-focal gradient pixel is one level off".

This lane did not measure the pixel again.

## Hypotheses (UNVERIFIED)

Chromium and Skia source code is not in this repository. The names below are not verified, and no measurement here proves that Chromium uses them.

- Chromium can parse CSS `hsl()` in `css_color_value.cc` and convert it with `Color::FromHSLA` or `CalcHue`.
- Chromium can use `gfx::HSLToSRGB` (`ui/gfx/color_conversions.cc`) or an older float path.
- The 8-bit result can come from `std::round` with `Sk4f_toL32` or `SkColor4f::toSkColor`.
- The 43 rounded-down channels can come from a float value just below `x.5` before rounding. This is not tested.
- The radial gradient can use Skia's `SkTwoPointConicalGradient` (`FocalData`, `appendGradientStages`, `xy_to_2pt_conical_focal_on_circle`).

## Unresolved

1. **HSL formula:** no tested formula matches all 16,202 oracle values. The arithmetic that gives Chrome's 43 rounded-down channels is unknown.
2. **Radial pixel count:** the Batch 08 report gives both "one-pixel" (`:137`, `:75`) and "7 of 65,536 pixels" (`:53`, `:137`) for `grad-radial-focal.svg`. The source evidence does not explain the difference.
