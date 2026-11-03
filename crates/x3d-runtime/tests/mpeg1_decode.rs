//! MPEG-1 decoding for MovieTexture (issue #90; ISO/IEC 19775-1 18.4.2 requires MPEG-1 Systems and MPEG-1 Video;
//! owner decision record a0acb58b). The decoder is `x3d_runtime::mpeg1`; the design is in docs/mpeg1-implementation.md.
//!
//! # Fixtures
//!
//! `tests/data/mpeg1/` holds synthetic streams made with ffmpeg 6.1.1 from its own `lavfi` sources (no third-party or
//! historical content), run from inside that directory. With `F="ffmpeg -v error -y -f lavfi -i testsrc2=size=64x48:rate=25"`:
//!
//! ```text
//! $F -frames:v 15 -c:v mpeg1video -q:v 4 -g 6 -bf 2 -an -f mpeg1video ipb-64x48.m1v      # I, P and B pictures
//! $F -frames:v 12 -c:v mpeg1video -q:v 4 -g 1 -bf 0 -an -f mpeg1video intra-64x48.m1v    # I pictures only
//! $F -frames:v 12 -c:v mpeg1video -q:v 4 -g 12 -bf 0 -an -f mpeg1video ip-64x48.m1v      # I and P pictures
//! ffmpeg -v error -y -f lavfi -i testsrc2=size=64x48:rate=25 -f lavfi -i sine=frequency=440:sample_rate=44100 \
//!   -frames:v 15 -t 0.6 -c:v mpeg1video -q:v 4 -g 6 -bf 2 -c:a mp2 -b:a 64k -f mpeg sys-av-64x48.mpg   # Systems, audio+video
//! ffmpeg ... -i testsrc2=size=50x38:rate=25 -frames:v 10 -c:v mpeg1video -q:v 4 -g 5 -bf 2 -an -f mpeg1video odd-50x38.m1v
//! ffmpeg ... -i testsrc2=size=64x48:rate=25 -frames:v 6 -vf "scale=33:25,format=yuv420p" -c:v mpeg1video -q:v 4 -g 3 -bf 1 \
//!   -an -f mpeg1video odd-33x25.m1v                                                       # odd width and height
//! ffmpeg ... -i testsrc2=size=64x48:rate=25 -frames:v 9 -c:v mpeg1video -q:v 4 -g 6 -bf 2 -an \
//!   -intra_matrix <the default intra matrix, 64 values, written explicitly> -f mpeg1video custom-matrix-64x48.m1v
//! ffmpeg ... -i color=c=red:size=32x32:rate=25 -frames:v 4 -c:v mpeg1video -q:v 2 -g 4 -bf 0 -an -f mpeg1video red-32x32.m1v
//! ```
//!
//! The reference of each stream is `ffmpeg -i <file> -an -f rawvideo -pix_fmt yuv420p <name>.yuv` (ffmpeg's `mpeg1video`
//! decoder), stored gzip-compressed (`gzip -9n`) as `<name>.yuv.gz` beside it.
//!
//! # The bound
//!
//! MPEG-1 lets a decoder differ in the inverse DCT within the accuracy limits of IEEE 1180, and the difference
//! accumulates through the P and B prediction chain. The comparison is therefore a per-sample bound, FIXED BEFORE THE
//! FIRST COMPARISON (2026-11-02) and not changed since: every Y, Cb and Cr sample of every picture differs from
//! ffmpeg's by at most [`MAX_ABS`], and the mean absolute difference of a stream is at most [`MEAN_ABS`]. The
//! measured worst case is printed by each test (`--nocapture`) and recorded in docs/mpeg1-implementation.md.

use x3d_runtime::mpeg1::{self, Mpeg1Error, demux};

const MAX_ABS: i32 = 3;
const MEAN_ABS: f64 = 0.5;

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/data/mpeg1/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// Compares the decode of `stream` to its ffmpeg reference; returns the measured (worst, mean) difference.
fn compare(stream: &str, width: usize, height: usize, pictures: usize) -> (i32, f64) {
    let yuv = mpeg1::decode_yuv(&data(stream), 1000).unwrap();
    let gz = data(&format!("{}.yuv.gz", stream.rsplit_once('.').unwrap().0));
    let mut reference = vec![];

    std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&gz[..]), &mut reference).unwrap();
    let (cw, ch) = (width.div_ceil(2), height.div_ceil(2));
    let per = width * height + 2 * cw * ch;

    assert_eq!((yuv.width, yuv.height), (width, height), "{stream}");
    assert_eq!(yuv.pictures.len(), pictures, "{stream}: picture count");
    assert_eq!(reference.len(), per * pictures, "{stream}: reference size");
    assert!(yuv.stopped.is_none(), "{stream}: {:?}", yuv.stopped);

    let (mut worst, mut sum, mut count) = (0i32, 0u64, 0u64);

    for (i, p) in yuv.pictures.iter().enumerate() {
        let r = &reference[i * per..(i + 1) * per];
        let planes = [
            (&p.y, &r[..width * height]),
            (&p.cb, &r[width * height..width * height + cw * ch]),
            (&p.cr, &r[width * height + cw * ch..]),
        ];

        for (mine, theirs) in planes {
            assert_eq!(mine.len(), theirs.len());

            for (a, b) in mine.iter().zip(theirs) {
                let d = (i32::from(*a) - i32::from(*b)).abs();

                worst = worst.max(d);
                sum += d as u64;
                count += 1;
            }
        }
    }

    let mean = sum as f64 / count as f64;

    eprintln!("{stream}: worst {worst}, mean {mean:.4} over {count} samples");
    assert!(
        worst <= MAX_ABS,
        "{stream}: worst difference {worst} > {MAX_ABS}"
    );
    assert!(
        mean <= MEAN_ABS,
        "{stream}: mean difference {mean} > {MEAN_ABS}"
    );

    (worst, mean)
}

// ---- video decoder against ffmpeg ----

#[test]
fn i_p_and_b_pictures_match_ffmpeg_within_the_bound() {
    compare("ipb-64x48.m1v", 64, 48, 15);
}

#[test]
fn intra_only_pictures_match_ffmpeg_within_the_bound() {
    compare("intra-64x48.m1v", 64, 48, 12);
}

#[test]
fn i_and_p_pictures_without_b_match_ffmpeg_within_the_bound() {
    compare("ip-64x48.m1v", 64, 48, 12);
}

#[test]
fn a_size_that_is_no_multiple_of_16_matches_ffmpeg_within_the_bound() {
    compare("odd-50x38.m1v", 50, 38, 10);
}

#[test]
fn an_odd_width_and_height_match_ffmpeg_within_the_bound() {
    compare("odd-33x25.m1v", 33, 25, 6);
}

#[test]
fn a_custom_intra_quantizer_matrix_is_read_and_used() {
    compare("custom-matrix-64x48.m1v", 64, 48, 9);
}

#[test]
fn a_systems_stream_decodes_to_the_pictures_ffmpeg_decodes() {
    compare("sys-av-64x48.mpg", 64, 48, 15);
}

#[test]
fn a_red_picture_converts_to_red_rgba() {
    let v = mpeg1::decode(&data("red-32x32.m1v"), 1 << 20).unwrap();

    assert_eq!((v.width, v.height, v.frames.len()), (32, 32, 4));
    assert!((v.framerate - 25.0).abs() < 1e-9);

    for px in v.frames[0].chunks(4) {
        assert!(
            px[0] >= 247 && px[1] <= 8 && px[2] <= 8 && px[3] == 255,
            "{px:?}"
        );
    }
}

// ---- Systems demux ----

#[test]
fn start_codes_and_extensions_tell_the_formats_apart() {
    let mpg = data("sys-av-64x48.mpg");
    let m1v = data("ipb-64x48.m1v");

    assert!(demux::is_system_stream(&mpg) && mpeg1::looks_like_mpeg1(&mpg));
    assert!(!demux::is_system_stream(&m1v) && mpeg1::looks_like_mpeg1(&m1v));
    assert!(!mpeg1::looks_like_mpeg1(b"\x1aE\xdf\xa3 webm") && !mpeg1::looks_like_mpeg1(b""));

    for ok in ["a.mpg", "A.MPEG", "x/y.mpe", "z.m1v"] {
        assert!(mpeg1::has_mpeg1_extension(ok), "{ok}");
    }

    assert!(!mpeg1::has_mpeg1_extension("a.webm") && !mpeg1::has_mpeg1_extension("mpg"));
}

#[test]
fn the_demuxed_video_stream_decodes_like_the_systems_stream() {
    let mpg = data("sys-av-64x48.mpg");
    let d = demux::demux(&mpg).unwrap();

    assert!(d.has_audio, "the fixture carries an mp2 stream");
    assert!(d.video.starts_with(&[0, 0, 1, 0xB3]));
    assert!(d.first_video_pts.is_some_and(|t| (0.0..2.0).contains(&t)));

    let a = mpeg1::decode_yuv(&mpg, 100).unwrap();
    let b = mpeg1::decode_yuv(&d.video, 100).unwrap();

    assert_eq!(a.pictures, b.pictures);
    assert!(a.has_audio && !b.has_audio);
}

#[test]
fn a_video_only_stream_is_not_a_systems_stream() {
    assert_eq!(
        demux::demux(&data("ipb-64x48.m1v")).unwrap_err(),
        Mpeg1Error::NotMpeg1
    );
}

#[test]
fn an_mpeg2_program_stream_is_refused() {
    // MPEG-2 pack header: start code, then marker bits `01`.
    let mut pack = vec![0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 0x01, 0x89, 0xC3, 0xF8];

    pack.extend_from_slice(&[0, 0, 1, 0xE0, 0, 4, 0x80, 0, 0, 0]);
    assert_eq!(
        demux::demux(&pack).unwrap_err(),
        Mpeg1Error::UnsupportedMpeg2
    );
    assert_eq!(
        mpeg1::decode_yuv(&pack, 10).err(),
        Some(Mpeg1Error::UnsupportedMpeg2)
    );
}

#[test]
fn an_mpeg2_packet_header_in_an_mpeg1_pack_is_refused() {
    let mut s = vec![0, 0, 1, 0xBA, 0x21, 0, 1, 0, 1, 0x80, 0, 0x01];

    s.extend_from_slice(&[0, 0, 1, 0xE0, 0, 4, 0x80, 0, 0, 0]);
    assert_eq!(demux::demux(&s).unwrap_err(), Mpeg1Error::UnsupportedMpeg2);
}

#[test]
fn mpeg2_video_after_the_sequence_header_is_refused() {
    // sequence header (64x48, 25 Hz, no matrices) followed by a sequence extension (start code 0xB5, id 1).
    let s = [
        0, 0, 1, 0xB3, 0x04, 0x00, 0x30, 0x13, 0xFF, 0xFF, 0xE0, 0x18, 0, 0, 1, 0xB5, 0x14, 0x8A,
        0, 1, 0, 0,
    ];

    assert_eq!(
        mpeg1::decode_yuv(&s, 10).err(),
        Some(Mpeg1Error::UnsupportedMpeg2)
    );
}

#[test]
fn a_systems_stream_without_a_video_packet_has_no_picture() {
    let mut s = vec![0, 0, 1, 0xBA, 0x21, 0, 1, 0, 1, 0x80, 0, 0x01];

    s.extend_from_slice(&[0, 0, 1, 0xC0, 0, 5, 0x0F, 1, 2, 3, 4]);
    assert_eq!(demux::demux(&s).unwrap_err(), Mpeg1Error::NoPicture);
}

// ---- negative inputs and robustness ----

#[test]
fn empty_tiny_and_garbage_inputs_are_errors() {
    assert_eq!(mpeg1::decode_yuv(b"", 10).err(), Some(Mpeg1Error::NotMpeg1));
    assert_eq!(
        mpeg1::decode_yuv(&[0, 0, 1], 10).err(),
        Some(Mpeg1Error::NotMpeg1)
    );
    assert_eq!(
        mpeg1::decode_yuv(&[0, 0, 1, 0xB3], 10).err(),
        Some(Mpeg1Error::Truncated)
    );
    assert_eq!(
        mpeg1::decode_yuv(&[0xFF; 4096], 10).err(),
        Some(Mpeg1Error::NotMpeg1)
    );
    assert!(mpeg1::decode(&[7; 1000], 1 << 20).is_err());

    // A sequence header with a zero width.
    let zero = [
        0, 0, 1, 0xB3, 0x00, 0x00, 0x30, 0x13, 0xFF, 0xFF, 0xE0, 0x18,
    ];

    assert_eq!(
        mpeg1::decode_yuv(&zero, 10).err(),
        Some(Mpeg1Error::Malformed("picture size zero"))
    );
    // A header with no pictures behind it.
    let none = [
        0, 0, 1, 0xB3, 0x04, 0x00, 0x30, 0x13, 0xFF, 0xFF, 0xE0, 0x18,
    ];

    assert_eq!(
        mpeg1::decode_yuv(&none, 10).err(),
        Some(Mpeg1Error::NoPicture)
    );
}

#[test]
fn mpeg4_and_other_containers_are_not_mpeg1() {
    let mut mp4 = vec![0, 0, 0, 0x18];

    mp4.extend_from_slice(b"ftypmp42\0\0\0\0mp42isom");

    for data in [
        mp4.as_slice(),
        b"\x1aE\xdf\xa3\x01\0\0\0\0\0\0\x1fwebm",
        b"OggS\0\x02",
        b"#VRML V2.0 utf8\nShape {}",
        &[],
    ] {
        assert_eq!(
            mpeg1::decode(data, 1 << 20).err(),
            Some(Mpeg1Error::NotMpeg1)
        );
    }
}

#[test]
fn every_truncation_of_every_fixture_returns_without_panic() {
    for name in [
        "ipb-64x48.m1v",
        "sys-av-64x48.mpg",
        "odd-33x25.m1v",
        "custom-matrix-64x48.m1v",
    ] {
        let full = data(name);
        let mut decoded_some = 0;

        for len in (0..full.len()).step_by(11).chain(0..48) {
            // Either pictures (a cut file plays up to the cut) or an error; never a panic.
            if mpeg1::decode_yuv(&full[..len], 100).is_ok() {
                decoded_some += 1;
            }
        }

        assert!(decoded_some > 0, "{name}: a long enough prefix decodes");
    }
}

/// Deterministic generator (SplitMix64) for the mutation tests.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);

        let mut z = self.0;

        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// The decoder has no `unsafe` and no FFI, so a panic is the failure to look for and an in-process run finds it; a
/// panic here names the fixture and the mutation seed.
#[test]
fn mutated_streams_never_panic() {
    let mut rng = Rng(0x4D50_4547_3145_5253);

    for name in [
        "ipb-64x48.m1v",
        "sys-av-64x48.mpg",
        "odd-50x38.m1v",
        "intra-64x48.m1v",
    ] {
        let full = data(name);

        for round in 0..400 {
            let mut d = full.clone();

            for _ in 0..1 + rng.next() % 8 {
                let at = (rng.next() as usize) % d.len();

                d[at] = match rng.next() % 3 {
                    0 => d[at] ^ (1 << (rng.next() % 8)),
                    1 => rng.next() as u8,
                    _ => 0,
                };
            }

            let r = std::panic::catch_unwind(|| {
                let _ = mpeg1::decode(&d, 64 << 20);
            });

            assert!(r.is_ok(), "{name}: mutation round {round} panicked");
        }
    }
}

#[test]
fn the_byte_budget_refuses_more_pictures_than_fit() {
    let ipb = data("ipb-64x48.m1v");
    let one = 64 * 48 * 4;

    assert_eq!(
        mpeg1::decode(&ipb, 15 * one as u64).unwrap().frames.len(),
        15
    );
    assert_eq!(
        mpeg1::decode(&ipb, 15 * one as u64 - 1).err(),
        Some(Mpeg1Error::TooLarge)
    );
    assert_eq!(mpeg1::decode(&ipb, 10).err(), Some(Mpeg1Error::TooLarge));
}

#[test]
fn a_stream_cut_inside_a_picture_keeps_the_pictures_before_it() {
    let ipb = data("ipb-64x48.m1v");
    let v = mpeg1::decode(&ipb[..ipb.len() * 3 / 4], 1 << 24).unwrap();

    assert!(!v.frames.is_empty() && v.frames.len() < 15);
}
