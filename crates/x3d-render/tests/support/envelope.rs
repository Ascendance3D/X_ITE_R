//! The reference of a world whose multisampled X_ITE picture is not the same in every run: the envelope of the runs
//! (`oracle/tools/rea_oit_msaa4_envelope.py`). Shared by render_rea_i92_items and render_rea_oit_i92_items.

/// The pictures of a world whose multisampled X_ITE output differs between runs: the lowest and the highest value of every
/// channel over the recorded runs (RGBA, alpha 255).
pub struct Envelope {
    pub width: u32,
    pub height: u32,
    min: Vec<u8>,
    max: Vec<u8>,
}

/// The largest spread of a channel an envelope may have (the recorder's MAX_SPREAD), and the fewest runs.
const ENVELOPE_MAX_SPREAD: u8 = 2;
const ENVELOPE_MIN_RUNS: u64 = 5;

impl Envelope {
    /// For every channel the value of the envelope nearest to `rgba`'s.
    pub fn nearest(&self, rgba: &[u8]) -> Vec<u8> {
        rgba.iter()
            .zip(self.min.iter().zip(&self.max))
            .map(|(v, (lo, hi))| (*v).clamp(*lo, *hi))
            .collect()
    }
}

/// `<world>.min.png` and `<world>.max.png` of `dir`, checked: a recording of enough runs with the `orderIndependentTransparency`
/// option as `oit` says, and pictures that agree with index.json and with each other (min <= max, no channel further apart than the limit).
pub fn load_envelope(dir: &std::path::Path, world: &str, oit: bool) -> Result<Envelope, String> {
    let index: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("index.json"))
            .map_err(|e| format!("no envelope index.json: {e}"))?,
    )
    .map_err(|e| format!("envelope index.json: {e}"))?;
    let entry = &index["worlds"][world];

    // A recording with the option off has no `oit` key.
    if index["oit"].as_bool().unwrap_or(false) != oit
        || index["runs"].as_u64().unwrap_or(0) < ENVELOPE_MIN_RUNS
    {
        return Err(format!(
            "the envelope is not a recording of at least 5 runs with oit {oit}"
        ));
    }

    if entry["runs"].as_array().map(|r| r.len() as u64) != index["runs"].as_u64() {
        return Err("the envelope does not list every run of the world".into());
    }

    let (w, h, min) = x3d_render::compare::load_png(&dir.join(format!("{world}.min.png")))
        .map_err(|e| format!("envelope min: {e}"))?;
    let (w2, h2, max) = x3d_render::compare::load_png(&dir.join(format!("{world}.max.png")))
        .map_err(|e| format!("envelope max: {e}"))?;

    if (w, h) != (w2, h2) || min.len() != max.len() {
        return Err("the envelope pictures differ in size".into());
    }

    if min.iter().zip(&max).any(|(lo, hi)| lo > hi) {
        return Err("the envelope has a lowest value above the highest".into());
    }

    let spread = min
        .iter()
        .zip(&max)
        .map(|(lo, hi)| hi - lo)
        .max()
        .unwrap_or(0);
    let differing = min
        .chunks_exact(4)
        .zip(max.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count() as u64;

    if spread > ENVELOPE_MAX_SPREAD
        || Some(spread as u64) != entry["largestSpread"].as_u64()
        || Some(differing) != entry["differingPixels"].as_u64()
    {
        return Err(format!(
            "the envelope pictures ({differing} pixels differ, spread {spread}) do not agree with index.json or the limit"
        ));
    }

    Ok(Envelope {
        width: w,
        height: h,
        min,
        max,
    })
}
