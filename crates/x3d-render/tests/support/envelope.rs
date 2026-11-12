//! The reference of a world whose multisampled X_ITE picture is not the same in every run (owner decision 2026-11-11): the
//! interval between the lowest and the highest value of every channel of every pixel over the stored runs of pinned X_ITE
//! (`oracle/tools/rea_oit_msaa4_envelope.py`, `<baseline>/msaa4-envelope-v1`). Shared by render_rea_i92_items,
//! render_rea_oit_i92_items and rea_msaa4_envelope.
//!
//! The interval is the measured one, with no margin. A channel of Rust's picture counts as different only outside the interval,
//! by its distance to the interval. The bound on that distance is the one of every other comparison ([`THRESHOLD`]), and a shot
//! with an envelope may have no pixel over it.
#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::path::Path;

/// The directory of the stored runs and their envelope, in a baseline directory.
pub const VERSION_DIR: &str = "msaa4-envelope-v1";

/// The largest distance of a channel from the interval that is not a failure: the threshold of `x3d-shot compare`.
pub const THRESHOLD: u8 = 8;

/// The X_ITE source of the stored runs (oracle/x_ite-pin/PINNED_COMMIT).
const PINNED: &str = "34cdf9f97";

/// The fewest stored runs of an envelope.
const MIN_CAPTURES: usize = 5;

/// The five shots the owner approved (baseline directory, world), all with Multisampling 4. No other world, baseline or
/// Multisampling value has an envelope; each of those has one exact reference picture.
pub const APPROVED: &[(&str, &str)] = &[
    ("rea-i92-render-items-baseline", "i92-fg-transparent.wrl"),
    ("rea-i92-render-items-baseline", "i92-tr-many.wrl"),
    ("rea-i92-render-items-baseline", "i92-tr-ortho.wrl"),
    ("rea-oit-i92-render-items-baseline", "i92-tr-many.wrl"),
    ("rea-oit-i92-render-items-baseline", "i92-tr-nested.wrl"),
];

/// Whether the shot of `world` in `baseline` with Multisampling `multisampling` is compared with an envelope.
pub fn approved(baseline: &str, world: &str, multisampling: u32) -> bool {
    multisampling == 4 && APPROVED.contains(&(baseline, world))
}

/// The lowest and the highest value of every channel (RGB) over the stored runs of a world.
#[derive(Debug, PartialEq)]
pub struct Envelope {
    pub width: u32,
    pub height: u32,
    pub captures: usize,
    min: Vec<u8>,
    max: Vec<u8>,
}

/// A picture measured against an envelope.
#[derive(Debug, PartialEq)]
pub struct Outside {
    /// Pixels with a channel outside the interval, by any amount.
    pub pixels_outside: usize,
    /// Pixels with a channel further than [`THRESHOLD`] outside the interval.
    pub pixels_over: usize,
    /// The largest distance of a channel from the interval.
    pub max_distance: u8,
}

/// The RGB channels of an RGBA picture.
fn rgb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect()
}

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Envelope {
    /// The envelope of `captures` (RGBA pictures of `width` x `height`).
    pub fn of(width: u32, height: u32, captures: &[Vec<u8>]) -> Result<Envelope, String> {
        if captures.len() < MIN_CAPTURES {
            return Err(format!(
                "{} pictures: an envelope needs at least {MIN_CAPTURES}",
                captures.len()
            ));
        }

        let size = (width * height * 4) as usize;

        if captures.iter().any(|c| c.len() != size) {
            return Err(format!("a picture is not {width}x{height}"));
        }

        let mut min = rgb(&captures[0]);
        let mut max = min.clone();

        for capture in &captures[1..] {
            for (i, v) in rgb(capture).into_iter().enumerate() {
                min[i] = min[i].min(v);
                max[i] = max[i].max(v);
            }
        }

        Ok(Envelope {
            width,
            height,
            captures: captures.len(),
            min,
            max,
        })
    }

    /// The pixels whose lowest and highest values differ, and the largest spread of a channel.
    pub fn variation(&self) -> (usize, u8) {
        let pixels = self
            .min
            .chunks_exact(3)
            .zip(self.max.chunks_exact(3))
            .filter(|(lo, hi)| lo != hi)
            .count();
        let spread = self
            .min
            .iter()
            .zip(&self.max)
            .map(|(lo, hi)| hi - lo)
            .max()
            .unwrap_or(0);

        (pixels, spread)
    }

    /// The interval of the channel `channel` (0 to 2) of the pixel at `x`, `y`.
    pub fn interval(&self, x: u32, y: u32, channel: usize) -> (u8, u8) {
        let i = ((y * self.width + x) * 3) as usize + channel;

        (self.min[i], self.max[i])
    }

    /// The RGBA picture `rgba` of `width` x `height` against the interval of every channel. A picture of another size is an error.
    pub fn outside(&self, width: u32, height: u32, rgba: &[u8]) -> Result<Outside, String> {
        if (width, height) != (self.width, self.height)
            || rgba.len() != (self.width * self.height * 4) as usize
        {
            return Err(format!(
                "the picture is {width}x{height} ({} bytes), the envelope {}x{}",
                rgba.len(),
                self.width,
                self.height
            ));
        }

        let mut out = Outside {
            pixels_outside: 0,
            pixels_over: 0,
            max_distance: 0,
        };

        for (p, pixel) in rgba.chunks_exact(4).enumerate() {
            let distance = (0..3)
                .map(|c| {
                    let (v, lo, hi) = (pixel[c], self.min[p * 3 + c], self.max[p * 3 + c]);

                    lo.saturating_sub(v).max(v.saturating_sub(hi))
                })
                .max()
                .unwrap_or(0);

            out.pixels_outside += (distance > 0) as usize;
            out.pixels_over += (distance > THRESHOLD) as usize;
            out.max_distance = out.max_distance.max(distance);
        }

        Ok(out)
    }
}

fn json(path: &Path) -> Result<serde_json::Value, String> {
    serde_json::from_str(
        &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// The stored runs of `world` in `dir` (a `msaa4-envelope-v1` directory of `baseline`), each checked against the SHA-256 of
/// manifest.json: (width, height, RGBA pictures).
pub fn load_captures(
    dir: &Path,
    baseline: &str,
    world: &str,
    oit: bool,
) -> Result<(u32, u32, Vec<Vec<u8>>), String> {
    let manifest = json(&dir.join("manifest.json"))?;

    if manifest["baseline"] != baseline
        || manifest["pinned"] != PINNED
        || manifest["oit"] != oit
        || manifest["ms"] != "4"
        || manifest["gl"]["multisampling"] != 4
    {
        return Err(format!(
            "manifest.json is not a recording of pinned X_ITE {PINNED} for {baseline} with Multisampling 4 and oit {oit}"
        ));
    }

    let entry = &manifest["worlds"][world];
    let listed = entry["captures"]
        .as_array()
        .ok_or_else(|| format!("manifest.json has no stored runs of {world}"))?;

    if Some(listed.len() as u64) != manifest["runs"].as_u64() {
        return Err(format!("manifest.json does not list every run of {world}"));
    }

    let (width, height) = (
        entry["width"].as_u64().unwrap_or(0) as u32,
        entry["height"].as_u64().unwrap_or(0) as u32,
    );
    let mut captures = vec![];

    for capture in listed {
        let file = capture["file"].as_str().unwrap_or("?");
        let path = dir.join(file);
        let bytes = std::fs::read(&path).map_err(|e| format!("{file}: {e}"))?;

        if Some(sha256(&bytes).as_str()) != capture["fileSha256"].as_str() {
            return Err(format!(
                "{file}: the stored picture does not have the SHA-256 of manifest.json"
            ));
        }

        let (w, h, rgba) = x3d_render::compare::load_png(&path)?;

        if (w, h) != (width, height) {
            return Err(format!(
                "{file}: {w}x{h}, manifest.json says {width}x{height}"
            ));
        }

        captures.push(rgba);
    }

    Ok((width, height, captures))
}

/// The envelope of `world` for the shot of `baseline` with Multisampling `multisampling`, built from the stored runs of `dir`.
/// An error: a shot that is not one of [`APPROVED`], a stored run that is missing or changed, and stored envelope pictures
/// (`<world>.min.png`, `<world>.max.png`, envelope.json) that are not what the stored runs give.
pub fn load_envelope(
    dir: &Path,
    baseline: &str,
    world: &str,
    oit: bool,
    multisampling: u32,
) -> Result<Envelope, String> {
    if !approved(baseline, world, multisampling) {
        return Err(format!(
            "{baseline} {world} with Multisampling {multisampling} is not an approved envelope shot"
        ));
    }

    let (width, height, captures) = load_captures(dir, baseline, world, oit)?;
    let envelope = Envelope::of(width, height, &captures)?;
    let report = json(&dir.join("envelope.json"))?;
    let entry = &report["worlds"][world];
    let (pixels, spread) = envelope.variation();

    if report["margin"] != 0
        || entry["captures"].as_u64() != Some(captures.len() as u64)
        || entry["differingPixels"].as_u64() != Some(pixels as u64)
        || entry["largestSpread"].as_u64() != Some(spread as u64)
    {
        return Err(format!(
            "envelope.json does not agree with the stored runs of {world} ({pixels} pixels differ, spread {spread})"
        ));
    }

    for (name, key, values) in [
        ("min", "minSha256", &envelope.min),
        ("max", "maxSha256", &envelope.max),
    ] {
        let path = dir.join(format!("{world}.{name}.png"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;

        if Some(sha256(&bytes).as_str()) != entry[key].as_str() {
            return Err(format!(
                "{world}.{name}.png does not have the SHA-256 of envelope.json"
            ));
        }

        let (w, h, stored) = x3d_render::compare::load_png(&path)?;

        if (w, h) != (width, height) || &rgb(&stored) != values {
            return Err(format!(
                "{world}.{name}.png is not the {name} of the stored runs"
            ));
        }
    }

    Ok(envelope)
}
