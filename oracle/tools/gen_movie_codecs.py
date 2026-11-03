#!/usr/bin/env python3
"""Synthetic VP8 (and, once admitted, AV1) WebM streams for the native MovieTexture decoders (issue #90).

  python3 -I oracle/tools/gen_movie_codecs.py

Every stream is made here from ffmpeg's own test-pattern sources (`testsrc2`, `mandelbrot`), so no
historical or third-party media is involved. For each stream the script writes

  oracle/synthetic-movie-codecs/media/<name>.webm        the stream
  oracle/synthetic-movie-codecs/media/<name>.planes.tsv  one line per shown picture: index, SHA-256 of the
                                                         Y, U and V planes (4:2:0, chroma ceil(w/2) x ceil(h/2),
                                                         rows without padding) as ffmpeg's own decoder outputs them

ffmpeg's VP8 decoder is bit-exact with libvpx, so the hashes are an independent reference for the Rust decoder
(crates/x3d-runtime/src/webm.rs tests). ffmpeg is needed only to regenerate; the tests read the committed files.
"""
import hashlib, os, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-movie-codecs"))
media = os.path.join(root, "media")
os.makedirs(media, exist_ok=True)

# name: (lavfi source, size w, h, rate, seconds, libvpx options)
VP8 = {
    # Key frames every 6 frames, inter frames between, hidden alt-ref frames, three loop-filter-visible textures.
    "vp8-testsrc": ("testsrc2", 96, 64, 10, 2.0, ["-g", "6", "-auto-alt-ref", "1", "-lag-in-frames", "6", "-crf", "20", "-b:v", "0"]),
    # A size that is not a multiple of 16 (the last macroblocks are cropped); the chroma planes are 25 x 17.
    "vp8-crop": ("testsrc2", 50, 34, 10, 1.0, ["-g", "4", "-crf", "24", "-b:v", "0"]),
    # Four token partitions, strong sharpness, high quantizer, moving fractal: split MVs, many modes.
    "vp8-parts": ("mandelbrot", 128, 80, 10, 1.5, ["-g", "8", "-slices", "4", "-sharpness", "5", "-crf", "40", "-b:v", "0"]),
    # Bitstream versions 1 to 3 (bilinear prediction, simple loop filter, full-pixel chroma).
    "vp8-profile1": ("testsrc2", 64, 48, 10, 1.0, ["-profile:v", "1", "-g", "5", "-crf", "20", "-b:v", "0"]),
    "vp8-profile2": ("testsrc2", 64, 48, 10, 1.0, ["-profile:v", "2", "-g", "5", "-crf", "20", "-b:v", "0"]),
    "vp8-profile3": ("testsrc2", 64, 48, 10, 1.0, ["-profile:v", "3", "-g", "5", "-crf", "20", "-b:v", "0"]),
}


def run(args):
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def planes_hashes(path, w, h):
    """Decode `path` with ffmpeg to raw yuv420p and hash each picture's Y, U and V planes."""
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-f", "rawvideo", "-pix_fmt", "yuv420p", "-"],
        check=True, stdout=subprocess.PIPE).stdout
    cw, ch = (w + 1) // 2, (h + 1) // 2
    sizes = (w * h, cw * ch, cw * ch)
    n = sum(sizes)
    assert len(raw) % n == 0
    lines = []
    for i in range(len(raw) // n):
        at = i * n
        hs = []
        for s in sizes:
            hs.append(hashlib.sha256(raw[at:at + s]).hexdigest())
            at += s
        lines.append("\t".join([str(i)] + hs))
    return lines


def make_vp8(name, spec):
    src, w, h, rate, seconds, opts = spec
    out = os.path.join(media, name + ".webm")
    run(["ffmpeg", "-y", "-v", "error", "-f", "lavfi", "-t", str(seconds), "-i", f"{src}=size={w}x{h}:rate={rate}",
         "-pix_fmt", "yuv420p", "-c:v", "libvpx", "-an", "-map_metadata", "-1", "-fflags", "+bitexact",
         "-flags:v", "+bitexact"] + opts + [out])
    with open(os.path.join(media, name + ".planes.tsv"), "w") as f:
        f.write("\n".join(planes_hashes(out, w, h)) + "\n")
    print(name, os.path.getsize(out), "bytes")


WORLD = """#X3D V4.0 utf8
PROFILE Full
NavigationInfo { type "NONE" headlight FALSE }
Background { skyColor 0.12 0.14 0.2 }
Viewpoint { position 0 0 4 }
Shape { appearance Appearance { texture MovieTexture { url "media/%s.webm" } } geometry Rectangle2D { size 3 2 } }
"""


# Streams with an audio track (issue #90, video audio): name: (video codec, audio codec, audio rate).
# A 440 Hz sine of 0.6 s, mono, with 6 pictures at 10 per second. `<name>.audio.f32` is ffmpeg's own decode of the
# audio track (float32 little-endian samples at the track's rate), the reference of the Rust decoder.
AV = {
    "vp8-vorbis": ("libvpx", "libvorbis", 16000),
    "vp9-vorbis": ("libvpx-vp9", "libvorbis", 16000),
    # Opus is not decodable natively (no decoder is admitted): the stream exists to test the diagnostic.
    "vp8-opus": ("libvpx", "libopus", 48000),
}


def make_ramp():
    """vp8-ramp: eight uniform gray pictures at 10 per second, picture k with luma 16 + 28 k (so a decoder or a
    browser shows which picture it is on); the timing reference of the playback comparison."""
    out = os.path.join(media, "vp8-ramp.webm")
    run(["ffmpeg", "-y", "-v", "error", "-f", "lavfi", "-t", "0.8", "-i", "color=c=black:s=64x48:r=10",
         "-vf", "format=yuv420p,geq=lum='16+28*N':cb=128:cr=128", "-c:v", "libvpx", "-g", "4", "-crf", "10",
         "-b:v", "0", "-an", "-map_metadata", "-1", "-fflags", "+bitexact", "-flags:v", "+bitexact", out])
    with open(os.path.join(media, "vp8-ramp.planes.tsv"), "w") as f:
        f.write("\n".join(planes_hashes(out, 64, 48)) + "\n")
    make_world("vp8-ramp")
    print("vp8-ramp", os.path.getsize(out), "bytes")


def make_av(name, spec):
    vcodec, acodec, rate = spec
    out = os.path.join(media, name + ".webm")
    run(["ffmpeg", "-y", "-v", "error", "-f", "lavfi", "-t", "0.6", "-i", "testsrc2=size=64x48:rate=10",
         "-f", "lavfi", "-t", "0.6", "-i", f"sine=frequency=440:sample_rate={rate}",
         "-ac", "1", "-c:v", vcodec, "-g", "5", "-crf", "20", "-b:v", "0", "-pix_fmt", "yuv420p",
         "-c:a", acodec, "-map_metadata", "-1", "-fflags", "+bitexact", "-flags:v", "+bitexact",
         "-flags:a", "+bitexact", "-shortest", out])
    if acodec == "libvorbis":
        raw = subprocess.run(["ffmpeg", "-v", "error", "-i", out, "-vn", "-f", "f32le", "-ac", "1", "-"],
                             check=True, stdout=subprocess.PIPE).stdout
        with open(os.path.join(media, name + ".audio.f32"), "wb") as f:
            f.write(raw)
    make_world_for(name)
    print(name, os.path.getsize(out), "bytes")


def make_world_for(name):
    make_world(name)


def make_world(name):
    """The still world of one stream: its first picture on an unlit rectangle (pinned X_ITE and the Rust renderer
    both show picture 0 before the node starts)."""
    with open(os.path.join(root, name + ".wrl"), "w") as f:
        f.write(WORLD % name)


if __name__ == "__main__":
    only = sys.argv[1:]
    for name, spec in VP8.items():
        if not only or name in only:
            make_vp8(name, spec)
            make_world(name)
    if not only or "vp8-ramp" in only:
        make_ramp()
    for name, spec in AV.items():
        if not only or name in only:
            make_av(name, spec)
