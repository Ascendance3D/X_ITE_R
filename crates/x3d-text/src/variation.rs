//! The default instance of a variable font, as opentype.js 2.0.0 computes it (`VariationManager`, `VariationProcessor.getNormalizedCoords`):
//! the axis coordinates of `font .variation .get ()` and their normalized values (`fvar` min / default / max, then the `avar` segment maps).
//! X_ITE never sets coordinates itself; the coordinates come from the font: the first named instance whose coordinates equal the axis defaults,
//! else the first instance named "Regular", else the axis defaults.
//!
//! Limits (opentype.js throws or logs where this returns `None` or skips): an `fvar` with another version, an `avar` whose axis count differs,
//! offsets outside the table. The instance name is read from the `name` table platforms 0, 1 and 3 with the languages `en` (Windows 0x409,
//! Macintosh 0) and records whose language is another one; a name record of another platform, encoding or language table is not looked at.

use std::collections::HashMap;

const EPS: f64 = f64::EPSILON;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as i16)
}

fn fixed_at(d: &[u8], o: usize) -> Option<f64> {
    Some(i16_at(d, o)? as f64 + u16_at(d, o + 2)? as f64 / 65536.0)
}

/// A platform's names: key to (language to text).
type Translations = HashMap<String, HashMap<String, String>>;

struct Axis {
    tag: String,
    min: f64,
    default: f64,
    max: f64,
}

struct Instance {
    name_id: u16,
    coords: HashMap<String, f64>,
}

/// `nameTableNames` has 23 entries (0 to 22): a name ID below 256 that is one of them is keyed by its name, any other by its number.
fn name_key(id: u16) -> String {
    const NAMES: [&str; 23] = [
        "copyright",
        "fontFamily",
        "fontSubfamily",
        "uniqueID",
        "fullName",
        "version",
        "postScriptName",
        "trademark",
        "manufacturer",
        "designer",
        "description",
        "manufacturerURL",
        "designerURL",
        "license",
        "licenseURL",
        "reserved",
        "preferredFamily",
        "preferredSubfamily",
        "compatibleFullName",
        "sampleText",
        "postScriptFindFontName",
        "wwsFamily",
        "wwsSubfamily",
    ];

    NAMES
        .get(id as usize)
        .map_or_else(|| id.to_string(), |s| s.to_string())
}

/// `getNameByID (names, nameID, [2, 17])`: the translations (language to text) of the first platform that has the name.
fn instance_name(name: &[u8], id: u16) -> Option<HashMap<String, String>> {
    if id < 256 && (id as usize) < 23 && ![2, 17].contains(&id) {
        return None;
    }

    let want = name_key(id);
    let count = u16_at(name, 2)? as usize;
    let string_offset = u16_at(name, 4)? as usize;
    // platforms in order of first appearance: the order a JavaScript object keeps its keys
    let mut platforms: Vec<(u16, Translations)> = vec![];

    for i in 0..count {
        let r = 6 + i * 12;
        let (platform, encoding, language, nid, len, off) = (
            u16_at(name, r)?,
            u16_at(name, r + 2)?,
            u16_at(name, r + 4)?,
            u16_at(name, r + 6)?,
            u16_at(name, r + 8)? as usize,
            u16_at(name, r + 10)? as usize,
        );
        let lang = match platform {
            0 if language == 65535 => Some("und".to_string()),
            1 if encoding <= 32 => Some(if language == 0 { "en" } else { "mac-other" }.to_string()),
            3 if encoding == 1 || encoding == 10 => {
                Some(if language == 0x409 { "en" } else { "win-other" }.to_string())
            }
            _ => None,
        };
        let Some(lang) = lang else { continue };
        let bytes = name.get(string_offset + off..string_offset + off + len)?;
        let text: String = if platform == 1 {
            bytes
                .iter()
                .map(|&b| if b <= 127 { b as char } else { '\u{fffd}' })
                .collect()
        } else {
            bytes
                .chunks_exact(2)
                .map(|c| {
                    char::from_u32(u16::from_be_bytes([c[0], c[1]]) as u32).unwrap_or('\u{fffd}')
                })
                .collect()
        };

        if text.is_empty() {
            continue;
        }

        let pos = platforms
            .iter()
            .position(|(p, _)| *p == platform)
            .unwrap_or_else(|| {
                platforms.push((platform, HashMap::new()));
                platforms.len() - 1
            });

        platforms[pos]
            .1
            .entry(name_key(nid))
            .or_default()
            .insert(lang, text);
    }

    platforms
        .into_iter()
        .find_map(|(_, mut keys)| keys.remove(&want))
}

/// The normalized default coordinates (one per `fvar` axis), or `None` when the font has no readable `fvar`.
pub fn default_normalized(
    fvar: &[u8],
    avar: Option<&[u8]>,
    name: Option<&[u8]>,
) -> Option<Vec<f64>> {
    if u16_at(fvar, 0)? != 1 || u16_at(fvar, 2)? != 0 {
        return None;
    }

    let data = u16_at(fvar, 4)? as usize;
    let axis_count = u16_at(fvar, 8)? as usize;
    let axis_size = u16_at(fvar, 10)? as usize;
    let instance_count = u16_at(fvar, 12)? as usize;
    let instance_size = u16_at(fvar, 14)? as usize;
    let mut axes = vec![];

    for i in 0..axis_count {
        let o = data + i * axis_size;

        axes.push(Axis {
            tag: fvar
                .get(o..o + 4)?
                .iter()
                .map(|&b| b as char)
                .collect::<String>(),
            min: fixed_at(fvar, o + 4)?,
            default: fixed_at(fvar, o + 8)?,
            max: fixed_at(fvar, o + 12)?,
        });
    }

    let mut instances = vec![];

    for j in 0..instance_count {
        let o = data + axis_count * axis_size + j * instance_size;
        let mut coords = HashMap::new();

        for (i, a) in axes.iter().enumerate() {
            coords.insert(a.tag.clone(), fixed_at(fvar, o + 4 + i * 4)?);
        }

        instances.push(Instance {
            name_id: u16_at(fvar, o)?,
            coords,
        });
    }

    // `getDefaultCoordinates`: an object keyed by tag, the last axis of a tag wins
    let mut defaults: HashMap<String, f64> = HashMap::new();

    for a in &axes {
        defaults.insert(a.tag.clone(), a.default);
    }

    let by_defaults = instances
        .iter()
        .position(|i| defaults.iter().all(|(k, v)| i.coords.get(k) == Some(v)));
    let chosen = by_defaults.or_else(|| {
        instances.iter().position(|i| {
            name.and_then(|n| instance_name(n, i.name_id))
                .and_then(|t| t.get("en").cloned())
                .is_some_and(|t| t == "Regular")
        })
    });
    let coords = chosen.map_or(defaults, |i| instances[i].coords.clone());
    let mut norm: Vec<f64> = axes
        .iter()
        .map(|a| {
            let v = coords.get(&a.tag).copied().unwrap_or(a.default);

            if v < a.default {
                (v - a.default + EPS) / (a.default - a.min + EPS)
            } else {
                (v - a.default + EPS) / (a.max - a.default + EPS)
            }
        })
        .collect();

    if let Some(avar) = avar {
        let count = u16_at(avar, 6)? as usize;

        if count == axes.len() {
            let mut o = 8;

            for n in norm.iter_mut() {
                let maps = u16_at(avar, o)? as usize;
                let mut pairs = vec![];

                for j in 0..maps {
                    pairs.push((
                        i16_at(avar, o + 2 + j * 4)? as f64 / 16384.0,
                        i16_at(avar, o + 4 + j * 4)? as f64 / 16384.0,
                    ));
                }

                o += 2 + maps * 4;

                for j in 0..pairs.len() {
                    if j >= 1 && *n < pairs[j].0 {
                        let prev = pairs[j - 1];

                        *n = ((*n - prev.0) * (pairs[j].1 - prev.1) + EPS)
                            / (pairs[j].0 - prev.0 + EPS)
                            + prev.1;
                        break;
                    }
                }
            }
        }
    }

    Some(norm)
}
