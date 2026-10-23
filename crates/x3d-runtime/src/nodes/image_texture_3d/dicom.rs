//! Browser/Texturing3D/DICOMParser.js over the dicom-parser library (`dicomParser.parseDicom`, the
//! build the oracle page loads).
//!
//! Ported: the Part 10 header and data set walk (implicit and explicit little endian, explicit big
//! endian, sequences, encapsulated pixel data), the pixel data of the uncompressed transfer
//! syntaxes and RLE Lossless (8 and 16 bit), the stored-bits mask, MONOCHROME1 / MONOCHROME2 / RGB
//! (also planar and the YBR_RCT, YBR_ICT, YBR_FULL_422 labels X_ITE treats as RGB), the vertical
//! flip and the normalization to 8 bit.
//!
//! Not ported (each ends the load with an `Error`, never a wrong picture): the JPEG Baseline,
//! JPEG Lossless, JPEG-LS and JPEG 2000 decoders X_ITE calls (jpeg-js, jpeg-lossless-decoder-js,
//! CharLS, OpenJPEG), PALETTE COLOR and YBR_FULL. The Deflated transfer syntax fails as it does in
//! the oracle page, which loads no inflater (`pako`).

use super::Volume;

pub enum DicomError {
    /// `dicomParser.parseDicom` threw: `console .error (error)`, "not DICOM".
    NotDicom(String),
    /// DICOMParser threw after the parse: the candidate fails with this `String (error)`.
    Throw(String),
}

use DicomError::*;

const UNDEFINED: u32 = 0xffff_ffff;

struct Element {
    tag: u32,
    #[allow(dead_code)]
    vr: [u8; 2],
    length: u32,
    data_offset: usize,
    little: bool,
    /// Encapsulated pixel data: basic offset table and fragments (offset, position, length).
    encapsulated: bool,
    basic_offset_table: Vec<u32>,
    fragments: Vec<(u32, usize, usize)>,
}

struct Stream<'a> {
    bytes: &'a [u8],
    pos: usize,
    little: bool,
}

fn parser_name(little: bool) -> &'static str {
    if little {
        "littleEndianByteArrayParser"
    } else {
        "bigEndianByteArrayParser"
    }
}

impl Stream<'_> {
    fn u16(&mut self) -> Result<u16, String> {
        if self.pos + 2 > self.bytes.len() {
            return Err(format!(
                "{}.readUint16: attempt to read past end of buffer",
                parser_name(self.little)
            ));
        }

        let a = [self.bytes[self.pos], self.bytes[self.pos + 1]];
        self.pos += 2;

        Ok(if self.little {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        })
    }

    fn u32(&mut self) -> Result<u32, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err(format!(
                "{}.readUint32: attempt to read past end of buffer",
                parser_name(self.little)
            ));
        }

        let a = [
            self.bytes[self.pos],
            self.bytes[self.pos + 1],
            self.bytes[self.pos + 2],
            self.bytes[self.pos + 3],
        ];
        self.pos += 4;

        Ok(if self.little {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        })
    }

    fn tag(&mut self) -> Result<u32, String> {
        let group = self.u16()? as u32;
        let element = self.u16()? as u32;

        Ok(group << 16 | element)
    }

    fn fixed_string(&mut self, length: usize) -> Result<String, String> {
        let s = read_fixed_string(self.bytes, self.pos, length)?;
        self.pos += length;

        Ok(s)
    }
}

/// `dicomParser.readFixedString`: stops at the first NUL.
fn read_fixed_string(bytes: &[u8], pos: usize, length: usize) -> Result<String, String> {
    if pos + length > bytes.len() {
        return Err("dicomParser.readFixedString: attempt to read past end of buffer".into());
    }

    Ok(bytes[pos..pos + length]
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect())
}

fn length_size(vr: &[u8; 2]) -> usize {
    match vr {
        b"OB" | b"OD" | b"OL" | b"OW" | b"SQ" | b"OF" | b"UC" | b"UR" | b"UT" | b"UN" => 4,
        _ => 2,
    }
}

/// readDicomElementExplicit.
fn read_explicit(s: &mut Stream) -> Result<Element, String> {
    let tag = s.tag()?;
    let vr_s = s.fixed_string(2)?;
    let mut vr = [b' '; 2];

    for (i, b) in vr_s.bytes().take(2).enumerate() {
        vr[i] = b;
    }

    let mut e = Element {
        tag,
        vr,
        length: 0,
        data_offset: 0,
        little: s.little,
        encapsulated: false,
        basic_offset_table: vec![],
        fragments: vec![],
    };

    if length_size(&vr) == 2 {
        e.length = s.u16()? as u32;
    } else {
        s.pos += 2;
        e.length = s.u32()?;
    }

    e.data_offset = s.pos;

    if &vr == b"SQ" {
        skip_sequence(s, &mut e, true)?;
        return Ok(e);
    }

    if e.length == UNDEFINED {
        if tag == 0x7fe0_0010 {
            find_end_of_encapsulated(s, &mut e)?;
        } else if &vr == b"UN" {
            skip_sequence(s, &mut e, false)?;
        } else {
            find_item_delimitation(s, &mut e);
        }

        return Ok(e);
    }

    s.pos += e.length as usize;

    Ok(e)
}

/// readDicomElementImplicit.
fn read_implicit(s: &mut Stream) -> Result<Element, String> {
    let tag = s.tag()?;
    let length = s.u32()?;
    let mut e = Element {
        tag,
        vr: [0; 2],
        length,
        data_offset: s.pos,
        little: s.little,
        encapsulated: false,
        basic_offset_table: vec![],
        fragments: vec![],
    };
    let private = (tag >> 16) & 1 == 1;

    // isSequence: the next tag is an item or a sequence delimiter.
    let sequence = if s.pos + 4 <= s.bytes.len() {
        let save = s.pos;
        let next = s.tag()?;
        s.pos = save;
        next == 0xfffe_e000 || next == 0xfffe_e0dd
    } else {
        false
    };

    if sequence && (!private || length == UNDEFINED) {
        skip_sequence(s, &mut e, false)?;
        return Ok(e);
    }

    if length == UNDEFINED {
        find_item_delimitation(s, &mut e);
        return Ok(e);
    }

    s.pos += length as usize;

    Ok(e)
}

fn read_element(s: &mut Stream, explicit: bool) -> Result<Element, String> {
    if explicit {
        read_explicit(s)
    } else {
        read_implicit(s)
    }
}

/// readSequenceItems*: moves past the sequence, with its nested items.
fn skip_sequence(s: &mut Stream, e: &mut Element, explicit: bool) -> Result<(), String> {
    if e.length != UNDEFINED {
        s.pos = e.data_offset + e.length as usize;
        return Ok(());
    }

    loop {
        if s.pos >= s.bytes.len() {
            // A sequence without its delimiter ends at the end of the data.
            e.length = (s.pos - e.data_offset) as u32;
            return Ok(());
        }

        let tag = s.tag()?;
        let length = s.u32()?;

        if tag == 0xfffe_e0dd {
            e.length = (s.pos - e.data_offset) as u32;
            return Ok(());
        }

        if tag != 0xfffe_e000 {
            return Err(format!(
                "dicomParser.readSequenceItem: item tag (FFFE,E000) not found at offset {}",
                s.pos - 8
            ));
        }

        if length != UNDEFINED {
            s.pos += length as usize;
            continue;
        }

        // An item of undefined length: its elements up to the item delimitation item.
        loop {
            if s.pos + 4 > s.bytes.len() {
                return Ok(());
            }

            let save = s.pos;
            let next = s.tag()?;

            if next == 0xfffe_e00d {
                s.u32()?;
                break;
            }

            s.pos = save;
            read_element(s, explicit)?;
        }
    }
}

/// findItemDelimitationItem: scans for FFFE,E00D.
fn find_item_delimitation(s: &mut Stream, e: &mut Element) {
    while s.pos + 4 <= s.bytes.len() {
        let group = u16::from_le_bytes([s.bytes[s.pos], s.bytes[s.pos + 1]]);
        let element = u16::from_le_bytes([s.bytes[s.pos + 2], s.bytes[s.pos + 3]]);
        let (group, element) = if s.little {
            (group, element)
        } else {
            (group.swap_bytes(), element.swap_bytes())
        };
        s.pos += 4;

        if group == 0xfffe && element == 0xe00d {
            e.length = (s.pos - e.data_offset) as u32;
            s.pos += 4;
            return;
        }
    }

    s.pos = s.bytes.len();
    e.length = (s.pos - e.data_offset) as u32;
}

/// findEndOfEncapsulatedElement.
fn find_end_of_encapsulated(s: &mut Stream, e: &mut Element) -> Result<(), String> {
    e.encapsulated = true;

    if s.tag()? != 0xfffe_e000 {
        return Err(
            "dicomParser.findEndOfEncapsulatedElement: basic offset table not found".into(),
        );
    }

    let table_length = s.u32()?;

    for _ in 0..table_length / 4 {
        let offset = s.u32()?;
        e.basic_offset_table.push(offset);
    }

    let base = s.pos;

    while s.pos < s.bytes.len() {
        let tag = s.tag()?;
        let mut length = s.u32()? as usize;

        if tag == 0xfffe_e0dd {
            s.pos += length;
            e.length = (s.pos - e.data_offset) as u32;
            return Ok(());
        } else if tag == 0xfffe_e000 {
            e.fragments.push(((s.pos - base - 8) as u32, s.pos, length));
        } else {
            length = length.min(s.bytes.len() - s.pos);
            e.fragments.push(((s.pos - base - 8) as u32, s.pos, length));
            s.pos += length;
            e.length = (s.pos - e.data_offset) as u32;
            return Ok(());
        }

        s.pos += length;
    }

    Ok(())
}

struct DataSet<'a> {
    bytes: &'a [u8],
    elements: std::collections::HashMap<u32, Element>,
}

impl DataSet<'_> {
    /// `dataSet .uint16 (tag)`: `None` for a missing or empty element.
    fn uint16(&self, tag: u32) -> Result<Option<u16>, String> {
        let Some(e) = self.elements.get(&tag).filter(|e| e.length != 0) else {
            return Ok(None);
        };

        if e.data_offset + 2 > self.bytes.len() {
            return Err(format!(
                "{}.readUint16: attempt to read past end of buffer",
                parser_name(e.little)
            ));
        }

        let a = [self.bytes[e.data_offset], self.bytes[e.data_offset + 1]];

        Ok(Some(if e.little {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        }))
    }

    /// `dataSet .string (tag)`: trimmed, `None` for a missing or empty element.
    fn string(&self, tag: u32) -> Result<Option<String>, String> {
        let Some(e) = self
            .elements
            .get(&tag)
            .filter(|e| e.length > 0 && e.length != UNDEFINED)
        else {
            return Ok(None);
        };
        let s = read_fixed_string(self.bytes, e.data_offset, e.length as usize)?;

        Ok(Some(super::js_trim(&s)))
    }
}

const BIG_ENDIAN: &str = "1.2.840.10008.1.2.2";

/// `dicomParser .parseDicom (bytes)` and the getters of `DICOMParser.parse`.
pub fn parse_dicom(input: &[u8]) -> Result<Volume, DicomError> {
    // readPart10Header.
    let mut s = Stream {
        bytes: input,
        pos: 0,
        little: true,
    };

    if input.len() < 132 {
        return Err(NotDicom(
            "dicomParser.readFixedString: attempt to read past end of buffer".into(),
        ));
    }

    if &input[128..132] != b"DICM" {
        return Err(NotDicom("dicomParser.readPart10Header: DICM prefix not found at location 132 - this is not a valid DICOM P10 file.".into()));
    }

    s.pos = 132;
    let mut meta = std::collections::HashMap::new();

    while s.pos < input.len() {
        let at = s.pos;
        let e = read_explicit(&mut s).map_err(NotDicom)?;

        if e.tag > 0x0002_ffff {
            s.pos = at;
            break;
        }

        meta.insert(e.tag, e);
    }

    let transfer_syntax = {
        let Some(e) = meta.get(&0x0002_0010) else {
            return Err(NotDicom(
                "Error: dicomParser.parseDicom: missing required meta header attribute 0002,0010"
                    .into(),
            ));
        };

        read_fixed_string(input, e.data_offset, e.length as usize).map_err(NotDicom)?
    };

    let explicit = transfer_syntax != "1.2.840.10008.1.2";

    if transfer_syntax == "1.2.840.10008.1.2.1.99" {
        return Err(NotDicom(
            "dicomParser.parseDicom: no inflater available to handle deflate transfer syntax"
                .into(),
        ));
    }

    if s.pos >= input.len() {
        return Err(NotDicom("dicomParser.ByteStream: parameter 'position' cannot be greater than or equal to 'byteArray' length".into()));
    }

    let mut data = Stream {
        bytes: input,
        pos: s.pos,
        little: transfer_syntax != BIG_ENDIAN,
    };
    let mut elements = std::collections::HashMap::new();
    let dataset_error = |_: String| NotDicom("[object Object]".into());

    while data.pos < input.len() {
        let e = read_element(&mut data, explicit).map_err(dataset_error)?;
        elements.insert(e.tag, e);
    }

    if explicit && data.pos > input.len() {
        return Err(NotDicom("[object Object]".into()));
    }

    for (k, v) in meta {
        elements.insert(k, v);
    }

    let ds = DataSet {
        bytes: input,
        elements,
    };

    build(&ds, &transfer_syntax).map_err(Throw)
}

fn nan(v: Option<u16>) -> f64 {
    v.map_or(f64::NAN, |v| v as f64)
}

/// Typed array contents of a frame, as the numbers JavaScript reads.
fn typed(frame: &[u8], bits_allocated: u32, signed: bool) -> Result<Vec<f64>, String> {
    Ok(match bits_allocated {
        8 => frame
            .iter()
            .map(|&b| if signed { b as i8 as f64 } else { b as f64 })
            .collect(),
        16 => frame
            .chunks_exact(2)
            .map(|c| {
                if signed {
                    i16::from_le_bytes([c[0], c[1]]) as f64
                } else {
                    u16::from_le_bytes([c[0], c[1]]) as f64
                }
            })
            .collect(),
        32 => frame
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64)
            .collect(),
        _ => return Err("Error: DICOM: unsupported pixel format.".into()),
    })
}

fn to_uint8(x: f64) -> u8 {
    if !x.is_finite() {
        return 0;
    }

    x.trunc().rem_euclid(256.0) as u8
}

fn build(ds: &DataSet, transfer_syntax: &str) -> Result<Volume, String> {
    let e = |m: String| format!("Error: {m}");
    let photometric = ds.string(0x0028_0004).map_err(&e)?;
    let components_tag = nan(ds.uint16(0x0028_0002).map_err(&e)?);
    let width = nan(ds.uint16(0x0028_0011).map_err(&e)?);
    let height = nan(ds.uint16(0x0028_0010).map_err(&e)?);
    let depth = if ds.elements.contains_key(&0x0028_0008) {
        // intString: parseInt of the first value.
        match ds.string(0x0028_0008).map_err(&e)? {
            Some(v) if ds.elements[&0x0028_0008].length > 0 => super::js_parse_int(&v),
            _ => f64::NAN,
        }
    } else {
        1.0
    };
    let mut bits_allocated = nan(ds.uint16(0x0028_0100).map_err(&e)?);
    let bits_stored = ds.uint16(0x0028_0101).map_err(&e)?;
    let pixel_representation = ds.uint16(0x0028_0103).map_err(&e)?.unwrap_or(0);
    let planar = ds.uint16(0x0028_0006).map_err(&e)?.unwrap_or(0);

    let pixel_element = ds
        .elements
        .get(&0x7fe0_0010)
        .or_else(|| ds.elements.get(&0x7fe0_0008));
    let is_palette = photometric.as_deref() == Some("PALETTE COLOR");
    let components = if is_palette { 3.0 } else { components_tag };
    let image_length = width * height * components;
    let byte_length = image_length * depth;
    let count = if byte_length.is_nan() {
        0
    } else if (0.0..=2147483648.0).contains(&byte_length) {
        byte_length as usize
    } else {
        return Err("RangeError: Array buffer allocation failed".into());
    };
    let mut bytes = vec![0u8; count];

    // getFrames.
    let mut frames: Vec<Vec<u8>> = vec![];
    let n_frames = if depth.is_nan() {
        0
    } else {
        depth.max(0.0) as usize
    };

    if let Some(p) = pixel_element.filter(|p| p.encapsulated) {
        if !p.basic_offset_table.is_empty() {
            for i in 0..n_frames {
                let table = &p.basic_offset_table;
                let start = p
                    .fragments
                    .iter()
                    .position(|f| f.0 == table[i.min(table.len() - 1)])
                    .ok_or("dicomParser.readEncapsulatedImageFrame: could not find fragment")?;
                let n = if i == table.len() - 1 {
                    p.fragments.len() - start
                } else {
                    (start + 1..p.fragments.len()).find(|&j| p.fragments[j].0 == table[i + 1]).map(|j| j - start).ok_or("dicomParser.calculateNumberOfFragmentsForFrame: could not find fragment with offset matching basic offset table")?
                };
                let mut out = vec![];

                for f in &p.fragments[start..start + n] {
                    out.extend_from_slice(&ds.bytes[f.1..f.1 + f.2]);
                }

                frames.push(out);
            }
        } else if n_frames != p.fragments.len() {
            return Err("Error: DICOM: JPEG basic offset table creation is not ported".into());
        } else {
            for f in &p.fragments {
                frames.push(ds.bytes[f.1..f.1 + f.2].to_vec());
            }
        }
    } else {
        let per_frame = width * height * components_tag;

        match bits_allocated as i64 {
            1 => {
                let Some(p) = pixel_element else {
                    return Err(
                        "TypeError: Cannot read properties of undefined (reading 'dataOffset')"
                            .into(),
                    );
                };

                for i in 0..n_frames {
                    let offset = p.data_offset as f64 + i as f64 * per_frame / 8.0;
                    let mut out = vec![0u8; per_frame as usize];

                    for (k, v) in out.iter_mut().enumerate() {
                        let byte = ds
                            .bytes
                            .get((offset as usize) + k / 8)
                            .copied()
                            .unwrap_or(0);
                        *v = (byte >> (k % 8)) & 1;
                    }

                    frames.push(out);
                }

                bits_allocated = 8.0;
            }
            8 | 16 | 32 => {
                let bytes_allocated = bits_allocated / 8.0;

                for i in 0..n_frames {
                    let Some(p) = pixel_element else {
                        return Err(
                            "TypeError: Cannot read properties of undefined (reading 'dataOffset')"
                                .into(),
                        );
                    };
                    let offset = p.data_offset as f64 + i as f64 * per_frame * bytes_allocated;
                    let len = per_frame * bytes_allocated;

                    if offset.is_nan() || len.is_nan() {
                        frames.push(vec![]);
                        continue;
                    }

                    if offset > ds.bytes.len() as f64 {
                        return Err(format!(
                            "RangeError: Start offset {offset} is outside the bounds of the buffer"
                        ));
                    }

                    if offset + len > ds.bytes.len() as f64 {
                        return Err(format!("RangeError: Invalid typed array length: {len}"));
                    }

                    frames.push(ds.bytes[offset as usize..(offset + len) as usize].to_vec());
                }
            }
            _ => return Err("Error: DICOM: unsupported pixel format.".into()),
        }
    }

    let planar_config = planar;
    let (w_u, h_u) = (width, height);

    for (f, frame) in frames.into_iter().enumerate() {
        // The transfer syntax.
        let frame = match transfer_syntax {
            "1.2.840.10008.1.2" | "1.2.840.10008.1.2.1" | "1.2.840.10008.1.2.1.99" => frame,
            "1.2.840.10008.1.2.2" => {
                if bits_allocated == 16.0 {
                    let mut f = frame;

                    for c in f.chunks_exact_mut(2) {
                        c.swap(0, 1);
                    }

                    // The swapped pairs are read in the host (little endian) order below.
                    f
                } else {
                    frame
                }
            }
            "1.2.840.10008.1.2.5" => decode_rle(
                &frame,
                bits_allocated,
                planar_config,
                w_u,
                h_u,
                components_tag,
            )?,
            "1.2.840.10008.1.2.4.50"
            | "1.2.840.10008.1.2.4.51"
            | "1.2.840.10008.1.2.4.57"
            | "1.2.840.10008.1.2.4.70"
            | "1.2.840.10008.1.2.4.80"
            | "1.2.840.10008.1.2.4.81"
            | "1.2.840.10008.1.2.4.90"
            | "1.2.840.10008.1.2.4.91" => {
                return Err(format!(
                    "Error: DICOM: the JPEG decoder of transfer syntax {transfer_syntax} is not ported."
                ));
            }
            "1.2.840.10008.1.2.4.52"
            | "1.2.840.10008.1.2.4.53"
            | "1.2.840.10008.1.2.4.54"
            | "1.2.840.10008.1.2.4.55"
            | "1.2.840.10008.1.2.4.56"
            | "1.2.840.10008.1.2.4.58"
            | "1.2.840.10008.1.2.4.59"
            | "1.2.840.10008.1.2.4.60"
            | "1.2.840.10008.1.2.4.61"
            | "1.2.840.10008.1.2.4.62"
            | "1.2.840.10008.1.2.4.63"
            | "1.2.840.10008.1.2.4.64"
            | "1.2.840.10008.1.2.4.65"
            | "1.2.840.10008.1.2.4.66"
            | "1.2.840.10008.1.2.4.92"
            | "1.2.840.10008.1.2.4.93" => {
                return Err(format!(
                    "Error: DICOM: this JPEG encoding ({transfer_syntax}) is not supported."
                ));
            }
            other => {
                return Err(format!(
                    "Error: DICOM: unsupported transfer syntax '{other}'."
                ));
            }
        };

        let mut values = typed(&frame, bits_allocated as u32, pixel_representation == 1)?;

        // Bits stored.
        if pixel_representation == 1 {
            if let Some(bits) = bits_stored {
                let mask = (1i32.wrapping_shl(bits as u32 & 31)).wrapping_sub(1);

                for v in values.iter_mut() {
                    let int = if v.is_finite() {
                        (v.trunc() as i64) as i32
                    } else {
                        0
                    };
                    let r = (int & mask) as f64;

                    *v = match bits_allocated as u32 {
                        8 => r as i32 as i8 as f64,
                        16 => r as i32 as i16 as f64,
                        _ => r as f32 as f64,
                    };
                }
            }
        }

        // The photometric interpretation.
        match photometric.as_deref() {
            Some("MONOCHROME1" | "MONOCHROME2") => {}
            Some("RGB" | "YBR_RCT" | "YBR_ICT" | "YBR_FULL_422") => {
                if planar_config == 1 {
                    if values.len() % 3 != 0 {
                        return Err("Error: DICOM: convertRGBColorByPlane: RGB buffer length must be divisble by 3.".into());
                    }

                    let n = values.len() / 3;
                    let mut out = Vec::with_capacity(values.len());

                    for i in 0..n {
                        out.push(values[i]);
                        out.push(values[n + i]);
                        out.push(values[2 * n + i]);
                    }

                    values = out;
                }
            }
            Some("YBR_FULL") => return Err("Error: DICOM: YBR_FULL is not ported.".into()),
            Some("PALETTE COLOR") => {
                return Err("Error: DICOM: PALETTE COLOR is not ported.".into());
            }
            other => {
                return Err(format!(
                    "Error: DICOM: unsupported image type '{}'.",
                    other.unwrap_or("undefined")
                ));
            }
        }

        // flipImage.
        let row = (components * width) as usize;
        let mut flipped = values.clone();

        if row > 0 && !height.is_nan() {
            for y in 0..height as usize {
                let (input, output) = (row * (height as usize - 1 - y), row * y);

                for x in 0..row {
                    if let (Some(&v), Some(o)) =
                        (values.get(input + x), flipped.get_mut(output + x))
                    {
                        *o = v;
                    }
                }
            }
        }

        // getNormalizeOffsetAndFactor and the copy.
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);

        for &v in &flipped {
            min = min.min(v);
            max = max.max(v);
        }

        let divergence = max - min;
        let factor = if divergence != 0.0 && !divergence.is_nan() {
            1.0 / divergence * 255.0
        } else {
            0.0
        };
        let base = if image_length.is_nan() {
            usize::MAX
        } else {
            f * image_length as usize
        };

        for (i, &v) in flipped.iter().enumerate() {
            if let Some(b) = base.checked_add(i).and_then(|b| bytes.get_mut(b)) {
                *b = to_uint8((v - min) * factor);
            }
        }
    }

    if photometric.as_deref() == Some("MONOCHROME1") {
        for b in bytes.iter_mut() {
            *b = 255 - *b;
        }
    }

    Ok(Volume {
        width,
        height,
        depth,
        components,
        data: Some(bytes),
    })
}

/// decodeRLE (8 bit, planar 8 bit, 16 bit); the result is the bytes of an Int8Array.
fn decode_rle(
    frame: &[u8],
    bits_allocated: f64,
    planar: u16,
    width: f64,
    height: f64,
    components: f64,
) -> Result<Vec<u8>, String> {
    let frame_size = (width * height) as usize;
    let comps = components as usize;
    let i32_at = |at: usize| -> Result<i32, String> {
        frame
            .get(at..at + 4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| "RangeError: Offset is outside the bounds of the DataView".to_string())
    };
    let segments = i32_at(0)?;

    if bits_allocated == 8.0 {
        let mut out = vec![0u8; frame_size * comps];

        for s in 0..segments.max(0) as usize {
            let mut out_index = if planar != 0 { s * frame_size } else { s };
            let mut in_index = i32_at((s + 1) * 4)? as usize;
            let mut max_index = i32_at((s + 2) * 4).unwrap_or(0) as usize;

            if max_index == 0 {
                max_index = frame.len();
            }

            let end_of_segment = frame_size * segments as usize;
            let step = if planar != 0 { 1 } else { comps };

            while in_index < max_index {
                let n = frame.get(in_index).copied().unwrap_or(0) as i8;
                in_index += 1;

                if n >= 0 {
                    let mut i = 0;

                    while i < n as usize + 1 && out_index < end_of_segment {
                        if let Some(o) = out.get_mut(out_index) {
                            *o = frame.get(in_index).copied().unwrap_or(0);
                        }
                        in_index += 1;
                        out_index += step;
                        i += 1;
                    }
                } else if n >= -127 {
                    let value = frame.get(in_index).copied().unwrap_or(0);
                    in_index += 1;
                    let mut j = 0;

                    while j < (-(n as i32)) as usize + 1 && out_index < end_of_segment {
                        if let Some(o) = out.get_mut(out_index) {
                            *o = value;
                        }
                        out_index += step;
                        j += 1;
                    }
                }
            }
        }

        return Ok(out);
    }

    if bits_allocated == 16.0 {
        let mut out = vec![0u8; frame_size * comps * 2];

        for s in 0..segments.max(0) as usize {
            let mut out_index = 0usize;
            let high = if s == 0 { 1 } else { 0 };
            let mut in_index = i32_at((s + 1) * 4)? as usize;
            let mut max_index = i32_at((s + 2) * 4).unwrap_or(0) as usize;

            if max_index == 0 {
                max_index = frame.len();
            }

            while in_index < max_index {
                let n = frame.get(in_index).copied().unwrap_or(0) as i8;
                in_index += 1;

                if n >= 0 {
                    let mut i = 0;

                    while i < n as usize + 1 && out_index < frame_size {
                        if let Some(o) = out.get_mut(out_index * 2 + high) {
                            *o = frame.get(in_index).copied().unwrap_or(0);
                        }
                        in_index += 1;
                        out_index += 1;
                        i += 1;
                    }
                } else if n >= -127 {
                    let value = frame.get(in_index).copied().unwrap_or(0);
                    in_index += 1;
                    let mut j = 0;

                    while j < (-(n as i32)) as usize + 1 && out_index < frame_size {
                        if let Some(o) = out.get_mut(out_index * 2 + high) {
                            *o = value;
                        }
                        out_index += 1;
                        j += 1;
                    }
                }
            }
        }

        return Ok(out);
    }

    Err("Error: DICOM: unsupported pixel format for RLE.".into())
}
