//! X3DParser.convertColor: X_ITE lets the browser's CSS engine parse a color name and reads
//! back the computed `rgb()`/`rgba()`. An invalid color inherits the wrapper's `white`.
//! This covers CSS named colors, hex notation and `rgb()/rgba()/hsl()/hsla()`.

const NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff), ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff), ("beige", 0xf5f5dc), ("bisque", 0xffe4c4), ("black", 0x000000),
    ("blanchedalmond", 0xffebcd), ("blue", 0x0000ff), ("blueviolet", 0x8a2be2), ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887), ("cadetblue", 0x5f9ea0), ("chartreuse", 0x7fff00), ("chocolate", 0xd2691e),
    ("coral", 0xff7f50), ("cornflowerblue", 0x6495ed), ("cornsilk", 0xfff8dc), ("crimson", 0xdc143c),
    ("cyan", 0x00ffff), ("darkblue", 0x00008b), ("darkcyan", 0x008b8b), ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9), ("darkgreen", 0x006400), ("darkgrey", 0xa9a9a9), ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b), ("darkolivegreen", 0x556b2f), ("darkorange", 0xff8c00), ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000), ("darksalmon", 0xe9967a), ("darkseagreen", 0x8fbc8f), ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f), ("darkslategrey", 0x2f4f4f), ("darkturquoise", 0x00ced1), ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493), ("deepskyblue", 0x00bfff), ("dimgray", 0x696969), ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff), ("firebrick", 0xb22222), ("floralwhite", 0xfffaf0), ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff), ("gainsboro", 0xdcdcdc), ("ghostwhite", 0xf8f8ff), ("gold", 0xffd700),
    ("goldenrod", 0xdaa520), ("gray", 0x808080), ("green", 0x008000), ("greenyellow", 0xadff2f),
    ("grey", 0x808080), ("honeydew", 0xf0fff0), ("hotpink", 0xff69b4), ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082), ("ivory", 0xfffff0), ("khaki", 0xf0e68c), ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5), ("lawngreen", 0x7cfc00), ("lemonchiffon", 0xfffacd), ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080), ("lightcyan", 0xe0ffff), ("lightgoldenrodyellow", 0xfafad2), ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90), ("lightgrey", 0xd3d3d3), ("lightpink", 0xffb6c1), ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa), ("lightskyblue", 0x87cefa), ("lightslategray", 0x778899), ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de), ("lightyellow", 0xffffe0), ("lime", 0x00ff00), ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6), ("magenta", 0xff00ff), ("maroon", 0x800000), ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd), ("mediumorchid", 0xba55d3), ("mediumpurple", 0x9370db), ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee), ("mediumspringgreen", 0x00fa9a), ("mediumturquoise", 0x48d1cc), ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970), ("mintcream", 0xf5fffa), ("mistyrose", 0xffe4e1), ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead), ("navy", 0x000080), ("oldlace", 0xfdf5e6), ("olive", 0x808000),
    ("olivedrab", 0x6b8e23), ("orange", 0xffa500), ("orangered", 0xff4500), ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa), ("palegreen", 0x98fb98), ("paleturquoise", 0xafeeee), ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5), ("peachpuff", 0xffdab9), ("peru", 0xcd853f), ("pink", 0xffc0cb),
    ("plum", 0xdda0dd), ("powderblue", 0xb0e0e6), ("purple", 0x800080), ("rebeccapurple", 0x663399),
    ("red", 0xff0000), ("rosybrown", 0xbc8f8f), ("royalblue", 0x4169e1), ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072), ("sandybrown", 0xf4a460), ("seagreen", 0x2e8b57), ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d), ("silver", 0xc0c0c0), ("skyblue", 0x87ceeb), ("slateblue", 0x6a5acd),
    ("slategray", 0x708090), ("slategrey", 0x708090), ("snow", 0xfffafa), ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4), ("tan", 0xd2b48c), ("teal", 0x008080), ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347), ("turquoise", 0x40e0d0), ("violet", 0xee82ee), ("wheat", 0xf5deb3),
    ("white", 0xffffff), ("whitesmoke", 0xf5f5f5), ("yellow", 0xffff00), ("yellowgreen", 0x9acd32),
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
        6 => rgb(d(0) * 16.0 + d(1), d(2) * 16.0 + d(3), d(4) * 16.0 + d(5), 1.0),
        8 => rgb(d(0) * 16.0 + d(1), d(2) * 16.0 + d(3), d(4) * 16.0 + d(5), (d(6) * 16.0 + d(7)) / 255.0),
        _ => return None,
    })
}

fn functional(s: &str) -> Option<[f64; 4]> {
    let open = s.find('(')?;
    let name = s[..open].to_ascii_lowercase();
    let args: Vec<&str> = s[open + 1..s.len() - 1].split(|c: char| c == ',' || c == '/' || c.is_whitespace()).filter(|a| !a.is_empty()).collect();

    let channel = |a: &str| -> Option<f64> {
        match a.strip_suffix('%') {
            Some(p) => Some((p.parse::<f64>().ok()? * 2.55).clamp(0.0, 255.0)),
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
            let (r, g, b) = (channel(args[0])?.round(), channel(args[1])?.round(), channel(args[2])?.round());
            Some(rgb(r, g, b, alpha(args.get(3))?))
        }
        "hsl" | "hsla" if args.len() == 3 || args.len() == 4 => {
            let h = args[0].trim_end_matches("deg").parse::<f64>().ok()?.rem_euclid(360.0) / 360.0;
            let s = (args[1].strip_suffix('%')?.parse::<f64>().ok()? / 100.0).clamp(0.0, 1.0);
            let l = (args[2].strip_suffix('%')?.parse::<f64>().ok()? / 100.0).clamp(0.0, 1.0);
            let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
            let p = 2.0 * l - q;
            let f = |t: f64| {
                let t = t.rem_euclid(1.0);
                let v = if t < 1.0 / 6.0 {
                    p + (q - p) * 6.0 * t
                } else if t < 0.5 {
                    q
                } else if t < 2.0 / 3.0 {
                    p + (q - p) * (2.0 / 3.0 - t) * 6.0
                } else {
                    p
                };
                (v * 255.0).round()
            };
            Some(rgb(f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0), alpha(args.get(3))?))
        }
        _ => None,
    }
}

/// Returns `[r, g, b, a]` in [0, 1] like X3DParser.convertColor.
pub fn convert_color(value: &str) -> [f64; 4] {
    let lower = value.to_ascii_lowercase();

    if lower == "transparent" {
        return [0.0, 0.0, 0.0, 0.0];
    }

    if let Some(h) = lower.strip_prefix('#') {
        return hex(h).unwrap_or(WHITE);
    }

    if lower.ends_with(')') {
        return functional(&lower).unwrap_or(WHITE);
    }

    match NAMED.iter().find(|(n, _)| *n == lower) {
        Some((_, c)) => rgb(((c >> 16) & 0xff) as f64, ((c >> 8) & 0xff) as f64, (c & 0xff) as f64, 1.0),
        None => WHITE,
    }
}
