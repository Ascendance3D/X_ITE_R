//! decodeAudioData as in Chromium, for PCM, A-law and mu-law WAV files and for MP3, FLAC and Ogg Vorbis files.
//!
//! Chromium decodes with FFmpeg (`media/filters/audio_file_reader.cc`) and converts the samples
//! to float with `media/base/audio_sample_types.h`: integer formats scale negative and positive
//! values by different factors. A file whose rate is not the context's is resampled with
//! `media::SincResampler` (request size 64, so a 32-tap kernel) by Blink's
//! `AudioBus::CreateBySampleRateConverting`. `web-audio-api` (symphonia, rubato) differs in both. MP3, FLAC and
//! Ogg Vorbis are decoded here with symphonia at the file's own rate and resampled the same way; other formats are
//! left to `web-audio-api`.
//!
//! Chromium is BSD-3-Clause, Copyright The Chromium Authors.

/// The decoded channels and the file's sample rate, or None if the bytes are not a WAV file
/// that FFmpeg reads as U8, S16, S32 (24 and 32 bit), float, A-law or mu-law.
pub fn decode_wav(bytes: &[u8]) -> Option<(Vec<Vec<f32>>, f32)> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let u16_at = |p: usize| u16::from_le_bytes([bytes[p], bytes[p + 1]]);
    let u32_at =
        |p: usize| u32::from_le_bytes([bytes[p], bytes[p + 1], bytes[p + 2], bytes[p + 3]]);
    let mut pos = 12;
    let mut format = None;

    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32_at(pos + 4) as usize;
        let body = pos + 8;

        if id == b"fmt " && body + 16 <= bytes.len() {
            let mut tag = u16_at(body);
            let channels = u16_at(body + 2) as usize;
            let rate = u32_at(body + 4);
            let bits = u16_at(body + 14);

            // WAVE_FORMAT_EXTENSIBLE: the sub format's first two bytes.
            if tag == 0xFFFE && size >= 40 && body + 26 <= bytes.len() {
                tag = u16_at(body + 24);
            }

            format = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            let (tag, channels, rate, bits) = format?;
            let data = &bytes[body..(body + size).min(bytes.len())];
            return convert(data, tag, channels, rate, bits).map(|c| (c, rate as f32));
        }

        pos = body + size + (size & 1);
    }

    None
}

/// FFmpeg's `alaw_to_linear` (G.711 A-law to 16-bit).
fn alaw(a: u8) -> i16 {
    let a = a ^ 0x55;
    let mut t = ((a & 0x0f) as i32) << 4;
    let seg = ((a & 0x70) >> 4) as u32;

    match seg {
        0 => t += 8,
        1 => t += 0x108,
        _ => {
            t += 0x108;
            t <<= seg - 1;
        }
    }

    (if a & 0x80 != 0 { t } else { -t }) as i16
}

/// FFmpeg's `ulaw_to_linear` (G.711 mu-law to 16-bit).
fn mulaw(u: u8) -> i16 {
    let u = !u;
    let mut t = (((u & 0x0f) as i32) << 3) + 0x84;

    t <<= (u & 0x70) >> 4;
    (if u & 0x80 != 0 { 0x84 - t } else { t - 0x84 }) as i16
}

fn convert(data: &[u8], tag: u16, channels: usize, rate: u32, bits: u16) -> Option<Vec<Vec<f32>>> {
    if channels == 0 || rate == 0 {
        return None;
    }

    // FixedSampleTypeTraits::To<float>: (float) (value - zero point) times 1 / (max - zero) or
    // 1 / (zero - min), both computed in float.
    let s16 = |v: i16| {
        let x = v as f32;
        if x < 0.0 {
            x * (1.0f32 / 32768.0)
        } else {
            x * (1.0f32 / 32767.0)
        }
    };
    let u8f = |v: u8| {
        let x = (v as i32 - 128) as f32;
        if x < 0.0 {
            x * (1.0f32 / 128.0)
        } else {
            x * (1.0f32 / 127.0)
        }
    };
    let s32 = |v: i32| {
        let x = v as f32;
        if x < 0.0 {
            x * (1.0f32 / 2147483648.0f32)
        } else {
            x * (1.0f32 / 2147483647i32 as f32)
        }
    };

    let width = match (tag, bits) {
        (1, 8) | (6, 8) | (7, 8) => 1,
        (1, 16) => 2,
        (1, 24) => 3,
        (1, 32) | (3, 32) => 4,
        _ => return None,
    };
    let frames = data.len() / (width * channels);
    let mut out = vec![Vec::with_capacity(frames); channels];

    for f in 0..frames {
        for (c, o) in out.iter_mut().enumerate() {
            let p = (f * channels + c) * width;
            let b = &data[p..p + width];
            o.push(match (tag, bits) {
                (1, 8) => u8f(b[0]),
                // pcm_alaw and pcm_mulaw decode to S16.
                (6, _) => s16(alaw(b[0])),
                (7, _) => s16(mulaw(b[0])),
                (1, 16) => s16(i16::from_le_bytes([b[0], b[1]])),
                // pcm_s24le decodes to S32 (value << 8).
                (1, 24) => s32(i32::from_le_bytes([0, b[0], b[1], b[2]])),
                (1, 32) => s32(i32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            });
        }
    }

    Some(out)
}

/// The planar samples of an MP3, FLAC or Ogg Vorbis file at the file's own rate (`web-audio-api` resamples
/// by itself, which Chromium does not), or None for any other file.
pub fn decode_compressed(bytes: &[u8]) -> Option<(Vec<Vec<f32>>, f32)> {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::codecs::audio::well_known::{
        CODEC_ID_FLAC, CODEC_ID_MP3, CODEC_ID_VORBIS,
    };
    use symphonia::core::errors::Error;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::meta::MetadataOptions;

    let source = symphonia::core::io::MediaSourceStream::new(
        Box::new(std::io::Cursor::new(bytes.to_vec())),
        Default::default(),
    );
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let id = track.id;
    let params = track.codec_params.as_ref()?.audio()?.clone();

    if ![CODEC_ID_MP3, CODEC_ID_FLAC, CODEC_ID_VORBIS].contains(&params.codec) {
        return None;
    }

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .ok()?;
    let mut channels: Vec<Vec<f32>> = vec![];
    let mut rate = 0u32;
    let mut part = Vec::new();
    // The frames of the last decoded packet.
    let mut last_frames = 0usize;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(_) => return None,
        };

        if packet.track_id != id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(buffer) => {
                rate = buffer.spec().rate();
                buffer.copy_to_vecs_planar::<f32>(&mut part);

                // A buffer without channels carries nothing (and has no `part[0]`): skip it.
                if part.is_empty() {
                    continue;
                }

                if channels.is_empty() {
                    channels = vec![vec![]; part.len()];
                }

                if part.len() != channels.len() {
                    return None;
                }

                for (c, p) in channels.iter_mut().zip(&part) {
                    c.extend_from_slice(p);
                }

                last_frames = part[0].len();
            }
            // A packet that does not decode (corrupt data, a cut-off last packet) is skipped, as FFmpeg skips it: the file
            // yields the packets that did decode. The behavior on damaged files is not compared with Chromium (only
            // that it neither panics nor loops, see the truncation test); any other error refuses the file.
            Err(Error::DecodeError(_) | Error::IoError(_)) => {}
            Err(_) => return None,
        }
    }

    // Chromium (FFmpeg) returns none of the frames of an Ogg Vorbis file's last packet: 215, 46, 64 and 128 frames
    // fewer for the four recorded files (oracle/rea-b24-audio-decode-baseline and -2-baseline), with or without a
    // trim at the end of the stream. The rule is "the last decoded packet of a Vorbis
    // stream, always", not a property of the trim or of the metadata; it is applied to every Vorbis file, because the
    // four recorded ones differ in size, rate, channel count and trim and all follow it. A file that is only one packet
    // is kept whole (not recorded).
    if params.codec == CODEC_ID_VORBIS && channels.first().is_some_and(|c| c.len() > last_frames) {
        for c in &mut channels {
            c.truncate(c.len() - last_frames);
        }
    }

    (!channels.is_empty() && rate > 0).then_some((channels, rate as f32))
}

const KERNEL_SIZE: usize = 32;
const KERNEL_OFFSET_COUNT: usize = 32;
const REQUEST_FRAMES: usize = 64;

/// ScopedSubnormalFloatDisabler: denormal inputs and results are zero.
fn ftz(x: f32) -> f32 {
    if x.is_subnormal() {
        0.0 * x.signum()
    } else {
        x
    }
}

#[derive(Clone, Copy)]
enum Convolve {
    C,
    Sse,
    Avx2,
    #[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
    Neon,
}

/// SincResampler::InitializeCPUSpecificFeatures.
fn convolve_proc() -> Convolve {
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma")
        {
            return Convolve::Avx2;
        }

        return Convolve::Sse;
    }

    #[cfg(target_arch = "aarch64")]
    {
        return Convolve::Neon;
    }

    #[allow(unreachable_code)]
    Convolve::C
}

fn convolve(proc_: Convolve, input: &[f32], k1: &[f32], k2: &[f32], factor: f64) -> f32 {
    match proc_ {
        Convolve::C => {
            let mut sum1 = 0f32;
            let mut sum2 = 0f32;

            for i in 0..input.len() {
                sum1 = ftz(sum1 + ftz(ftz(input[i]) * ftz(k1[i])));
                sum2 = ftz(sum2 + ftz(ftz(input[i]) * ftz(k2[i])));
            }

            ((1.0 - factor) * sum1 as f64 + factor * sum2 as f64) as f32
        }
        Convolve::Sse | Convolve::Neon => {
            // Four lanes, multiply then add (vmlaq_f32 is not fused either).
            let mut s1 = [0f32; 4];
            let mut s2 = [0f32; 4];

            for i in (0..input.len()).step_by(4) {
                for j in 0..4 {
                    let x = ftz(input[i + j]);
                    s1[j] = ftz(s1[j] + ftz(x * ftz(k1[i + j])));
                    s2[j] = ftz(s2[j] + ftz(x * ftz(k2[i + j])));
                }
            }

            let a = ftz((1.0 - factor) as f32);
            let b = ftz(factor as f32);
            let m: Vec<f32> = (0..4)
                .map(|j| ftz(ftz(s1[j] * a) + ftz(s2[j] * b)))
                .collect();

            // SSE: movehl + add, then lane 0 + lane 1. NEON: high + low, then pairwise.
            let h0 = ftz(m[2] + m[0]);
            let h1 = ftz(m[3] + m[1]);
            ftz(h0 + h1)
        }
        Convolve::Avx2 => {
            let mut s1 = [0f32; 8];
            let mut s2 = [0f32; 8];

            for i in (0..input.len()).step_by(8) {
                for j in 0..8 {
                    let x = ftz(input[i + j]);
                    s1[j] = ftz(x.mul_add(ftz(k1[i + j]), s1[j]));
                    s2[j] = ftz(x.mul_add(ftz(k2[i + j]), s2[j]));
                }
            }

            let a = ftz((1.0 - factor) as f32);
            let b = ftz(factor as f32);
            let m: Vec<f32> = (0..4)
                .map(|j| ftz(ftz(ftz(s1[j] + s1[j + 4]) * a) + ftz(ftz(s2[j] + s2[j + 4]) * b)))
                .collect();
            let h0 = ftz(m[2] + m[0]);
            let h1 = ftz(m[3] + m[1]);
            ftz(h0 + h1)
        }
    }
}

/// media::SincResampler with request_frames 64, reading one channel.
struct SincResampler<'a> {
    ratio: f64,
    kernel: Vec<f32>,
    input_buffer: Vec<f32>,
    /// Offsets into `input_buffer`: r0 (the read target), r2, r3, r4; r1 is 0.
    r0: usize,
    r3: usize,
    r4: usize,
    block_size: usize,
    virtual_source_idx: f64,
    buffer_primed: bool,
    source: &'a [f32],
    proc_: Convolve,
}

impl<'a> SincResampler<'a> {
    fn new(ratio: f64, source: &'a [f32]) -> SincResampler<'a> {
        let mut r = SincResampler {
            ratio,
            kernel: vec![0.0; KERNEL_SIZE * (KERNEL_OFFSET_COUNT + 1)],
            input_buffer: vec![0.0; REQUEST_FRAMES + KERNEL_SIZE],
            r0: 0,
            r3: 0,
            r4: 0,
            block_size: 0,
            virtual_source_idx: 0.0,
            buffer_primed: false,
            source,
            proc_: convolve_proc(),
        };

        r.update_regions(false);
        r.initialize_kernel();
        r
    }

    fn update_regions(&mut self, second_load: bool) {
        self.r0 = if second_load {
            KERNEL_SIZE
        } else {
            KERNEL_SIZE / 2
        };
        self.r3 = self.r0 + REQUEST_FRAMES - KERNEL_SIZE;
        self.r4 = self.r0 + REQUEST_FRAMES - KERNEL_SIZE / 2;
        self.block_size = self.r4 - KERNEL_SIZE / 2;
    }

    fn initialize_kernel(&mut self) {
        let (alpha, pi) = (0.16f64, std::f64::consts::PI);
        let (a0, a1, a2) = (0.5 * (1.0 - alpha), 0.5, 0.5 * alpha);
        // SincScaleFactor for kMinKernelSize.
        let sinc_scale_factor = (if self.ratio > 1.0 {
            1.0 / self.ratio
        } else {
            1.0
        }) * 0.90;
        let n = KERNEL_SIZE as i32;

        for offset_idx in 0..=KERNEL_OFFSET_COUNT {
            let subsample_offset = offset_idx as f32 / KERNEL_OFFSET_COUNT as f32;

            for i in 0..n {
                let idx = i as usize + offset_idx * KERNEL_SIZE;
                let pre_sinc = std::f32::consts::PI * ((i - n / 2) as f32 - subsample_offset);
                let x = (i as f32 - subsample_offset) / n as f32;
                let window = (a0 - a1 * (2.0 * pi * x as f64).cos()
                    + a2 * (4.0 * pi * x as f64).cos()) as f32;
                let p = pre_sinc as f64;
                self.kernel[idx] = (window as f64
                    * if pre_sinc != 0.0 {
                        (sinc_scale_factor * p).sin() / p
                    } else {
                        sinc_scale_factor
                    }) as f32;
            }
        }
    }

    /// The read callback: the next request_frames frames of the source, zero padded.
    fn read(&mut self) {
        let dest = &mut self.input_buffer[self.r0..self.r0 + REQUEST_FRAMES];
        let n = self.source.len().min(REQUEST_FRAMES);
        dest[..n].copy_from_slice(&self.source[..n]);
        dest[n..].fill(0.0);
        self.source = &self.source[n..];
    }

    fn resample(&mut self, destination: &mut [f32]) {
        let mut remaining = destination.len();
        let mut d = 0;

        if !self.buffer_primed && remaining > 0 {
            self.read();
            self.buffer_primed = true;
        }

        while remaining > 0 {
            while self.virtual_source_idx < self.block_size as f64 {
                let source_idx = self.virtual_source_idx as usize;
                let virtual_offset_idx =
                    (self.virtual_source_idx - source_idx as f64) * KERNEL_OFFSET_COUNT as f64;
                let offset_idx = virtual_offset_idx as usize;
                let start = offset_idx * KERNEL_SIZE;
                let k1 = &self.kernel[start..start + KERNEL_SIZE];
                let k2 = &self.kernel[start + KERNEL_SIZE..start + 2 * KERNEL_SIZE];
                let input = &self.input_buffer[source_idx..source_idx + KERNEL_SIZE];
                let factor = virtual_offset_idx - offset_idx as f64;

                destination[d] = convolve(self.proc_, input, k1, k2, factor);
                d += 1;
                self.virtual_source_idx += self.ratio;
                remaining -= 1;

                if remaining == 0 {
                    return;
                }
            }

            self.virtual_source_idx -= self.block_size as f64;
            self.input_buffer
                .copy_within(self.r3..self.r3 + KERNEL_SIZE, 0);

            if self.r0 == KERNEL_SIZE / 2 {
                self.update_regions(true);
            }

            self.read();
        }
    }
}

/// AudioBus::CreateBySampleRateConverting for one channel (mix_to_mono false).
pub fn resample(source: &[f32], source_rate: f64, destination_rate: f64) -> Vec<f32> {
    let ratio = source_rate / destination_rate;
    let length = (source.len() as f64 / ratio) as usize;
    let mut out = vec![0f32; length];

    // (A decoded bus is not flagged silent, so the IsSilent shortcut does not apply.)
    SincResampler::new(ratio, source).resample(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(format: u16, bits: u16, channels: u16, rate: u32, data: &[u8]) -> Vec<u8> {
        let block = channels * bits / 8;
        let mut w = b"RIFF".to_vec();
        w.extend((36 + data.len() as u32).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(16u32.to_le_bytes());
        w.extend(format.to_le_bytes());
        w.extend(channels.to_le_bytes());
        w.extend(rate.to_le_bytes());
        w.extend((rate * block as u32).to_le_bytes());
        w.extend(block.to_le_bytes());
        w.extend(bits.to_le_bytes());
        w.extend(b"data");
        w.extend((data.len() as u32).to_le_bytes());
        w.extend(data);
        w
    }

    #[test]
    fn int16_uses_chromium_asymmetric_scale() {
        let data: Vec<u8> = [i16::MIN, -1, 0, 1, i16::MAX]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let (channels, rate) = decode_wav(&wav(1, 16, 1, 22050, &data)).unwrap();
        assert_eq!(rate, 22050.0);
        assert_eq!(
            channels[0],
            vec![-1.0, -1.0 / 32768.0, 0.0, 1.0 / 32767.0, 1.0]
        );
    }

    #[test]
    fn g711_follows_ffmpeg() {
        // alaw_to_linear: 0x55 and 0xD5 are the smallest steps (-8 and +8), 0xAA and 0x2A the largest (+-32256).
        assert_eq!([alaw(0x55), alaw(0xD5)], [-8, 8]);
        assert_eq!([alaw(0xAA), alaw(0x2A)], [32256, -32256]);
        // ulaw_to_linear: 0xFF and 0x7F are zero, 0x00 and 0x80 the largest (+-32124).
        assert_eq!([mulaw(0xFF), mulaw(0x7F)], [0, 0]);
        assert_eq!([mulaw(0x80), mulaw(0x00)], [32124, -32124]);

        let (channels, rate) = decode_wav(&wav(6, 8, 1, 8000, &[0xD5, 0x55])).unwrap();
        assert_eq!(rate, 8000.0);
        assert_eq!(channels[0], vec![8.0 / 32767.0, -8.0 / 32768.0]);
    }

    #[test]
    fn files_that_are_not_mp3_flac_or_vorbis_are_left_alone() {
        // Not a compressed file, and a WAV (left to decode_wav).
        assert!(decode_compressed(b"not audio").is_none());
        assert!(decode_compressed(&wav(1, 16, 1, 8000, &[0, 0, 1, 0])).is_none());
    }

    /// Every cut of an MP3, a FLAC and an Ogg Vorbis file is refused or decodes to no more than the whole file, and
    /// never panics (damaged files: the packets that decode are kept).
    #[test]
    fn a_truncated_compressed_file_never_panics() {
        for (name, keeps_a_prefix) in [
            ("mp3-44100-stereo.mp3", true),
            ("flac-44100-stereo.flac", true),
            // A cut Ogg file is refused by the demuxer (no panic is all that is asked of it).
            ("ogg-44100-stereo.ogg", false),
        ] {
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../oracle/synthetic-rea-b24-audio")
                    .join(name),
            )
            .unwrap();
            let whole = decode_compressed(&bytes).unwrap().0[0].len();
            let step = (bytes.len() / 60).max(1);
            let mut kept = 0;

            for cut in (0..bytes.len()).step_by(step) {
                let r = std::panic::catch_unwind(|| decode_compressed(&bytes[..cut]));

                assert!(r.is_ok(), "{name} cut at {cut}: panic");

                if let Some((channels, _)) = r.unwrap() {
                    assert!(channels[0].len() <= whole, "{name} cut at {cut}");
                    kept += 1;
                }
            }

            assert!(
                kept > 0 || !keeps_a_prefix,
                "{name}: no cut decodes, the test proves nothing"
            );
        }
    }

    #[test]
    fn uint8_and_stereo_deinterleave() {
        let (channels, _) = decode_wav(&wav(1, 8, 2, 48000, &[0, 255, 128, 129])).unwrap();
        assert_eq!(channels, vec![vec![-1.0, 0.0], vec![1.0, 1.0 / 127.0]]);
    }

    #[test]
    fn resample_length_is_chromium_truncation() {
        let source: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin()).collect();
        assert_eq!(
            resample(&source, 22050.0, 48000.0).len(),
            (1000.0 / (22050.0 / 48000.0)) as usize
        );
    }
}
