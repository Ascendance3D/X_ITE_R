//! WOFF 1.0 to SFNT (REA batch 08, lane net): opentype.js `parseBuffer` accepts `wOFF` fonts (it inflates the zlib
//! compressed tables); `x3d_text::Font::parse` takes SFNT and, after its own decompression, WOFF2. The runtime
//! converts a WOFF 1.0 file here before it parses the font, which keeps the zlib dependency in this crate.

use std::io::Read;

fn be32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*d.get(o)?, *d.get(o + 1)?, *d.get(o + 2)?, *d.get(o + 3)?]))
}

fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

/// The SFNT file inside a WOFF 1.0 file, or the data unchanged when it is not WOFF. `None`: a WOFF file that cannot be unpacked.
pub fn woff1_to_sfnt(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 || &data[0..4] != b"wOFF" {
        return Some(data.to_vec());
    }

    let flavor = be32(data, 4)?;
    let count = be16(data, 12)? as usize;
    let mut tables: Vec<([u8; 4], u32, Vec<u8>)> = Vec::with_capacity(count);

    for i in 0..count {
        let e = 44 + i * 20;
        let tag: [u8; 4] = data.get(e..e + 4)?.try_into().ok()?;
        let (offset, comp, orig, checksum) = (be32(data, e + 4)? as usize, be32(data, e + 8)? as usize, be32(data, e + 12)? as usize, be32(data, e + 16)?);
        let raw = data.get(offset..offset.checked_add(comp)?)?;
        let body = if comp < orig {
            let mut out = Vec::with_capacity(orig);
            flate2::read::ZlibDecoder::new(raw).read_to_end(&mut out).ok()?;

            if out.len() != orig {
                return None;
            }

            out
        } else {
            raw.to_vec()
        };

        tables.push((tag, checksum, body));
    }

    tables.sort_by_key(|t| t.0);

    let mut entry_selector = 0u16;

    while (1usize << (entry_selector + 1)) <= count.max(1) {
        entry_selector += 1;
    }

    let search_range = (1u16 << entry_selector) * 16;
    let range_shift = (count as u16) * 16 - search_range;
    let mut out = Vec::with_capacity(data.len() * 2);

    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(count as u16).to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    let mut offset = 12 + count * 16;

    for (tag, checksum, body) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        offset += body.len().div_ceil(4) * 4;
    }

    for (_, _, body) in &tables {
        out.extend_from_slice(body);
        out.resize(out.len().div_ceil(4) * 4, 0);
    }

    Some(out)
}
