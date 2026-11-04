//! X3DParser.convertColor: X_ITE lets the browser's CSS engine parse a color name and reads
//! back the computed `rgb()`/`rgba()`. An invalid color inherits the wrapper's `white`.
//! This covers CSS named colors, hex notation, `rgb()/rgba()/hsl()/hsla()` and the CSS Color 4
//! functions `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()` and `color()`, and their CSS Color 5
//! relative forms (`lab(from red l a b)`, see `relative`).

mod relative;

const NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

const WHITE: [f64; 4] = [1.0, 1.0, 1.0, 1.0];

fn rgb(r: f64, g: f64, b: f64, a: f64) -> [f64; 4] {
    [r / 255.0, g / 255.0, b / 255.0, a]
}

fn hex(s: &str) -> Option<[f64; 4]> {
    let digits: Vec<u32> = s.chars().map(|c| c.to_digit(16)).collect::<Option<_>>()?;
    let d = |i: usize| digits[i] as f64;

    Some(match digits.len() {
        3 => rgb(d(0) * 17.0, d(1) * 17.0, d(2) * 17.0, 1.0),
        4 => rgb(d(0) * 17.0, d(1) * 17.0, d(2) * 17.0, (d(3) * 17.0) / 255.0),
        6 => rgb(
            d(0) * 16.0 + d(1),
            d(2) * 16.0 + d(3),
            d(4) * 16.0 + d(5),
            1.0,
        ),
        8 => rgb(
            d(0) * 16.0 + d(1),
            d(2) * 16.0 + d(3),
            d(4) * 16.0 + d(5),
            (d(6) * 16.0 + d(7)) / 255.0,
        ),
        _ => return None,
    })
}

/// One component of a CSS Color 4 function: a number, a percentage, an angle (in degrees) or `none`.
#[derive(Clone, Copy)]
enum Comp {
    None,
    Num(f64),
    Pct(f64),
    Angle(f64),
}

fn comp(token: &str) -> Option<Comp> {
    let t = token.to_ascii_lowercase();

    if t == "none" {
        return Some(Comp::None);
    }

    let number = |v: &str| -> Option<f64> {
        // CSS numbers only: no `inf`, `nan` or Rust-only spellings.
        let ok = !v.is_empty()
            && v.bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-' | b'e'))
            && v.bytes().any(|b| b.is_ascii_digit());
        if ok { v.parse::<f64>().ok() } else { None }
    };

    if let Some(v) = t.strip_suffix('%') {
        return number(v).map(Comp::Pct);
    }

    for (unit, scale) in [
        ("deg", 1.0),
        ("grad", 0.9),
        ("rad", 180.0 / std::f64::consts::PI),
        ("turn", 360.0),
    ] {
        if let Some(v) = t.strip_suffix(unit) {
            return number(v).map(|v| Comp::Angle(v * scale));
        }
    }

    number(&t).map(Comp::Num)
}

/// The arguments of a CSS Color 4 function in its space-separated modern syntax (a comma is
/// invalid there): the components before the optional `/ alpha`, and the alpha.
fn modern_args(inner: &str) -> Option<(Vec<&str>, Option<Comp>)> {
    let mut parts = inner.split('/');
    let main = parts.next()?;
    let alpha = parts.next();

    if parts.next().is_some() {
        return None;
    }

    let words: Vec<&str> = main.split_ascii_whitespace().collect();

    if words.iter().any(|w| w.contains(',')) {
        return None;
    }

    let alpha = match alpha {
        None => None,
        Some(a) => {
            let mut w = a.split_ascii_whitespace();
            let (one, rest) = (w.next()?, w.next());
            if rest.is_some() || one.contains(',') {
                return None;
            }
            Some(comp(one)?)
        }
    };

    Some((words, alpha))
}

/// The alpha Chrome keeps: a number or percentage clamped to [0, 1], `none` for a missing one.
fn alpha_value(a: Comp) -> Option<f64> {
    match a {
        Comp::None => None,
        Comp::Num(v) => Some(v.clamp(0.0, 1.0)),
        Comp::Pct(v) => Some((v / 100.0).clamp(0.0, 1.0)),
        Comp::Angle(_) => None,
    }
}

/// The alpha as a computed color serializes it: omitted (`None`) when absent or exactly 1,
/// `Some(None)` for `none`.
fn serialized_alpha(alpha: Option<Comp>) -> Option<Option<f64>> {
    match alpha.map(alpha_value) {
        Some(Some(1.0)) => None,
        a => a,
    }
}

/// A number rounded to the 6 significant digits Chrome serializes a computed color with.
fn sig6(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        v
    } else {
        format!("{v:.5e}").parse().unwrap_or(v)
    }
}

/// What `X3DParser.convertColor` makes of a color Chrome does not compute to `rgb()`. The computed
/// value is a string such as `lab(50 40 20 / 0.5)` or `color(srgb 1 0.5 0)`, which X_ITE reads as
/// if it were `rgb(r, g, b, a)`: it drops only a trailing `)`, splits on spaces and commas and
/// takes `parseFloat` of each piece. The first piece (`lab(50`, `color(srgb`) is therefore NaN, and
/// the remaining pieces land one channel to the left, with `/` and `none` as NaN too.
///
/// `numbers` are the serialized components (`first_is_glued`: the first one follows the opening
/// parenthesis directly), `alpha` the serialized alpha (`None`: omitted),
/// `Some(None)`: written as `none`).
fn unparsed_function(
    numbers: &[Option<f64>],
    first_is_glued: bool,
    alpha: Option<Option<f64>>,
) -> [f64; 4] {
    let mut values: Vec<f64> = vec![f64::NAN];

    // `lab(50` is one piece, so the first number is part of the NaN; `color(srgb 1 ...` is not.
    let skip = usize::from(first_is_glued);

    values.extend(numbers[skip..].iter().map(|v| v.map_or(f64::NAN, sig6)));

    if let Some(a) = alpha {
        values.push(f64::NAN);
        values.push(a.map_or(f64::NAN, sig6));
    }

    [
        values[0] / 255.0,
        values.get(1).copied().unwrap_or(f64::NAN) / 255.0,
        values.get(2).copied().unwrap_or(f64::NAN) / 255.0,
        values.get(3).copied().unwrap_or(1.0),
    ]
}

/// A hue as Chrome serializes it: in degrees, normalized to [0, 360); `none` stays `none`.
fn hue_degrees(c: Comp) -> Option<Option<f64>> {
    match c {
        Comp::None => Some(None),
        Comp::Num(v) | Comp::Angle(v) => Some(Some(v.rem_euclid(360.0))),
        Comp::Pct(_) => None,
    }
}

/// The color of a CSS Color 4 function in its modern syntax, or `None` where Chrome rejects it.
fn color4(name: &str, inner: &str) -> Option<[f64; 4]> {
    let (words, alpha) = modern_args(inner)?;
    let alpha = serialized_alpha(alpha);

    // A component: `none`, a number, or a percentage that is `pct_scale` units per 100%.
    // `lo`/`hi` clamp the result; angles are not valid here.
    let scaled = |c: Comp, pct_scale: f64, lo: f64, hi: f64| -> Option<Option<f64>> {
        match c {
            Comp::None => Some(None),
            Comp::Num(v) => Some(Some(v.clamp(lo, hi))),
            Comp::Pct(v) => Some(Some((v / 100.0 * pct_scale).clamp(lo, hi))),
            Comp::Angle(_) => None,
        }
    };

    let comps: Vec<Comp> = words.iter().map(|w| comp(w)).collect::<Option<_>>()?;
    let inf = f64::INFINITY;

    match name {
        "hwb" if comps.len() == 3 => {
            let h = hue_degrees(comps[0])?.unwrap_or(0.0);
            // Whiteness and blackness: a percentage, or a bare number read the same way.
            let fraction = |c: Comp| -> Option<f64> {
                match c {
                    Comp::None => Some(0.0),
                    Comp::Num(v) | Comp::Pct(v) => Some(v.max(0.0) / 100.0),
                    Comp::Angle(_) => None,
                }
            };
            let (w, b) = (fraction(comps[1])?, fraction(comps[2])?);
            let a = match alpha {
                None => 1.0,
                Some(a) => a.unwrap_or(0.0),
            };
            // The closest evaluation order found for Blink: the pure hue in 32-bit float, the
            // mix of whiteness and blackness in 64-bit, then the 32-bit scale and rounding.
            // Values whose channel is exactly x.5 in real arithmetic can still land one level
            // away from Chrome (see `KNOWN_HWB_TIES` in the test).
            let to_byte = |v: f64| ((v as f32 * 255.0f32) + 0.5f32).floor() as f64;

            let rgb_unit: [f64; 3] = if w + b >= 1.0 {
                let g = w / (w + b);
                [g, g, g]
            } else {
                let [r, g, bl] = hue_rgb_f32(h as f32, 1.0, 0.5).map(f64::from);
                let k = 1.0 - w - b;
                [r * k + w, g * k + w, bl * k + w]
            };

            Some(rgb(
                to_byte(rgb_unit[0]),
                to_byte(rgb_unit[1]),
                to_byte(rgb_unit[2]),
                a,
            ))
        }
        "lab" if comps.len() == 3 => Some(unparsed_function(
            &[
                scaled(comps[0], 100.0, 0.0, 100.0)?,
                scaled(comps[1], 125.0, -inf, inf)?,
                scaled(comps[2], 125.0, -inf, inf)?,
            ],
            true,
            alpha,
        )),
        "lch" if comps.len() == 3 => Some(unparsed_function(
            &[
                scaled(comps[0], 100.0, 0.0, 100.0)?,
                scaled(comps[1], 150.0, 0.0, inf)?,
                hue_degrees(comps[2])?,
            ],
            true,
            alpha,
        )),
        "oklab" if comps.len() == 3 => Some(unparsed_function(
            &[
                scaled(comps[0], 1.0, 0.0, 1.0)?,
                scaled(comps[1], 0.4, -inf, inf)?,
                scaled(comps[2], 0.4, -inf, inf)?,
            ],
            true,
            alpha,
        )),
        "oklch" if comps.len() == 3 => Some(unparsed_function(
            &[
                scaled(comps[0], 1.0, 0.0, 1.0)?,
                scaled(comps[1], 0.4, 0.0, inf)?,
                hue_degrees(comps[2])?,
            ],
            true,
            alpha,
        )),
        _ => None,
    }
}

/// `color(<space> r g b [/ alpha])`: a predefined RGB or XYZ color space.
fn color_function(inner: &str) -> Option<[f64; 4]> {
    let (words, alpha) = modern_args(inner)?;
    let alpha = serialized_alpha(alpha);

    if words.len() != 4 {
        return None;
    }

    let space = words[0].to_ascii_lowercase();

    if !matches!(
        space.as_str(),
        "srgb"
            | "srgb-linear"
            | "display-p3"
            | "a98-rgb"
            | "prophoto-rgb"
            | "rec2020"
            | "xyz"
            | "xyz-d50"
            | "xyz-d65"
    ) {
        return None;
    }

    let mut numbers = [None; 3];

    for (slot, word) in numbers.iter_mut().zip(&words[1..]) {
        *slot = match comp(word)? {
            Comp::None => None,
            Comp::Num(v) => Some(v),
            Comp::Pct(v) => Some(v / 100.0),
            Comp::Angle(_) => return None,
        };
    }

    Some(unparsed_function(&numbers, false, alpha))
}

/// The RGB of a hue (degrees), saturation and lightness, all unrounded, in [0, 1], in 32-bit float.
fn hue_rgb_f32(h: f32, s: f32, l: f32) -> [f32; 3] {
    let hue = h.rem_euclid(360.0) / 60.0;
    let t2 = if l <= 0.5 {
        l * (s + 1.0)
    } else {
        l + s - l * s
    };
    let t1 = l * 2.0 - t2;
    let f = |mut hv: f32| {
        if hv < 0.0 {
            hv += 6.0;
        }

        if hv >= 6.0 {
            hv -= 6.0;
        }

        if hv < 1.0 {
            (t2 - t1) * hv + t1
        } else if hv < 3.0 {
            t2
        } else if hv < 4.0 {
            (t2 - t1) * (4.0 - hv) + t1
        } else {
            t1
        }
    };

    [f(hue + 2.0), f(hue), f(hue - 2.0)]
}

fn functional(s: &str) -> Option<[f64; 4]> {
    let open = s.find('(')?;
    let name = s[..open].to_ascii_lowercase();
    let inner = &s[open + 1..s.len() - 1];

    // CSS Color 5 relative colors: `lab(from red l a b)`.
    if inner
        .trim_start()
        .strip_prefix("from")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_whitespace()))
    {
        return relative::relative_color(&name, inner);
    }

    match name.as_str() {
        "hwb" | "lab" | "lch" | "oklab" | "oklch" => {
            return color4(&name, &s[open + 1..s.len() - 1]);
        }
        "color" => return color_function(&s[open + 1..s.len() - 1]),
        _ => {}
    }

    let args: Vec<&str> = s[open + 1..s.len() - 1]
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|a| !a.is_empty())
        .collect();

    let channel = |a: &str| -> Option<f64> {
        match a.strip_suffix('%') {
            Some(p) => Some((p.parse::<f64>().ok()? / 100.0 * 255.0).clamp(0.0, 255.0)),
            None => Some(a.parse::<f64>().ok()?.clamp(0.0, 255.0)),
        }
    };

    let alpha = |a: Option<&&str>| -> Option<f64> {
        match a {
            None => Some(1.0),
            Some(a) => match a.strip_suffix('%') {
                Some(p) => Some((p.parse::<f64>().ok()? / 100.0).clamp(0.0, 1.0)),
                None => Some(a.parse::<f64>().ok()?.clamp(0.0, 1.0)),
            },
        }
    };

    match name.as_str() {
        "rgb" | "rgba" if args.len() == 3 || args.len() == 4 => {
            let (r, g, b) = (
                channel(args[0])?.round(),
                channel(args[1])?.round(),
                channel(args[2])?.round(),
            );
            Some(rgb(r, g, b, alpha(args.get(3))?))
        }
        "hsl" | "hsla" if args.len() == 3 || args.len() == 4 => {
            // Angle units of CSS Color 4: deg (default), grad, rad, turn.
            let h = if let Some(v) = args[0].strip_suffix("deg") {
                v.parse::<f64>().ok()?
            } else if let Some(v) = args[0].strip_suffix("grad") {
                v.parse::<f64>().ok()? * 0.9
            } else if let Some(v) = args[0].strip_suffix("rad") {
                v.parse::<f64>().ok()? * 180.0 / std::f64::consts::PI
            } else if let Some(v) = args[0].strip_suffix("turn") {
                v.parse::<f64>().ok()? * 360.0
            } else {
                args[0].parse::<f64>().ok()?
            };
            let s = (args[1].strip_suffix('%')?.parse::<f64>().ok()? / 100.0).clamp(0.0, 1.0);
            let l = (args[2].strip_suffix('%')?.parse::<f64>().ok()? / 100.0).clamp(0.0, 1.0);

            // CSS Color 4 hue-to-rgb with the hue in sixths. Blink keeps the channel as a 32-bit
            // float and scales it in 32-bit before rounding: `f32` cast first, then `* 255`.
            let hue = h.rem_euclid(360.0) / 60.0;
            let t2 = if l <= 0.5 {
                l * (s + 1.0)
            } else {
                l + s - l * s
            };
            let t1 = l * 2.0 - t2;
            let f = |mut hv: f64| {
                if hv < 0.0 {
                    hv += 6.0;
                }

                if hv >= 6.0 {
                    hv -= 6.0;
                }

                let v = if hv < 1.0 {
                    (t2 - t1) * hv + t1
                } else if hv < 3.0 {
                    t2
                } else if hv < 4.0 {
                    (t2 - t1) * (4.0 - hv) + t1
                } else {
                    t1
                };

                ((v as f32 * 255.0f32) as f64 + 0.5).floor()
            };

            Some(rgb(f(hue + 2.0), f(hue), f(hue - 2.0), alpha(args.get(3))?))
        }
        _ => None,
    }
}

/// The computed color of `value` as a CSS parser would give it, or `None` where the browser
/// ignores the declaration (an invalid color keeps the wrapper's color, see `convert_color`).
pub fn try_convert_color(value: &str) -> Option<[f64; 4]> {
    let mut lower = value.to_ascii_lowercase();

    // A relative color left open at the end of the text is closed there, as CSS parsing does
    // (the SVG color token ends at the first `)`, so `rgb(from red r g b / max(r, 1)` is open).
    if let Some(open) = lower.find('(') {
        let depth = lower
            .matches('(')
            .count()
            .saturating_sub(lower.matches(')').count());

        if depth > 0
            && lower[open + 1..]
                .trim_start()
                .strip_prefix("from")
                .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_whitespace()))
        {
            lower.push_str(&")".repeat(depth));
        }
    }

    if lower == "transparent" {
        return Some([0.0, 0.0, 0.0, 0.0]);
    }

    if let Some(h) = lower.strip_prefix('#') {
        return hex(h);
    }

    if lower.ends_with(')') {
        return functional(&lower);
    }

    NAMED.iter().find(|(n, _)| *n == lower).map(|(_, c)| {
        rgb(
            ((c >> 16) & 0xff) as f64,
            ((c >> 8) & 0xff) as f64,
            (c & 0xff) as f64,
            1.0,
        )
    })
}

/// Like `try_convert_color`, with `currentcolor` inside a relative color meaning `current`
/// (`[r, g, b, a]` in [0, 1]), the color of the wrapper element `convertColor` sets.
pub fn try_convert_color_in(value: &str, current: [f64; 4]) -> Option<[f64; 4]> {
    relative::with_current(current, || try_convert_color(value))
}

/// Returns `[r, g, b, a]` in [0, 1] like X3DParser.convertColor.
pub fn convert_color(value: &str) -> [f64; 4] {
    try_convert_color(value).unwrap_or(WHITE)
}
