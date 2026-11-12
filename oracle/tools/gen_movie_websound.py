#!/usr/bin/env python3
"""Synthetic WebM and Ogg files for the browser MovieTexture sound path (Phase 9A, after ADR 0015).

  python3 -I oracle/tools/gen_movie_websound.py

Every file is made here from generated samples, so no historical or third-party media is involved. The timeline is the
one of oracle/tools/gen_movie_mp4.py (the constants are copied, because `python3 -I` does not import a sibling file):
SEGMENTS segments of SEGMENT seconds; in segment k every picture is the grey level GREY[k] and the audio is a sine of
TONE[k] Hz. A test reads the segment of the sound from its frequency.

  crates/x3d-runtime/tests/data/websound/<name>   the files of the runtime tests, and SHA256SUMS
  crates/x3d-web/tests/browser/worlds/movie/<name> copies of the files the browser tests load (BROWSER)

ffmpeg (with libvpx, libopus, libvorbis and libtheora) is a development tool here: it is needed only to regenerate; the
tests read the committed files.
"""
import hashlib, math, os, shutil, struct, subprocess, tempfile

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(ROOT, "crates", "x3d-runtime", "tests", "data", "websound")
web_dir = os.path.join(ROOT, "crates", "x3d-web", "tests", "browser", "worlds", "movie")
W, H, RATE = 64, 48, 20
SEGMENT, SEGMENTS = 0.25, 8
GREY = [16 + 30 * k for k in range(SEGMENTS)]
TONE = [300 + 100 * k for k in range(SEGMENTS)]
AUDIO_RATE = 48000
# The files the browser tests load.
BROWSER = ["vp8-opus.webm", "vp8-vorbis.webm", "theora-vorbis.ogv", "theora-opus.ogv"]


def video_raw(path):
    """Grey RGB pictures, SEGMENT * RATE pictures per segment."""
    per = round(SEGMENT * RATE)
    with open(path, "wb") as f:
        for k in range(SEGMENTS):
            f.write(bytes([GREY[k]] * (W * H * 3)) * per)


def audio_raw(path, start_segment=0):
    """Stereo f32le: segment k is a sine of TONE[k] Hz at half scale, the phase continuous."""
    per = round(SEGMENT * AUDIO_RATE)
    phase = 0.0
    with open(path, "wb") as f:
        for k in range(start_segment, SEGMENTS):
            step = 2 * math.pi * TONE[k] / AUDIO_RATE
            frames = bytearray()
            for _ in range(per):
                s = 0.5 * math.sin(phase)
                frames += struct.pack("<ff", s, s)
                phase += step
            f.write(frames)


def run(args):
    subprocess.run(["ffmpeg", "-y", "-v", "error"] + args, check=True)


COMMON = ["-map_metadata", "-1", "-fflags", "+bitexact", "-flags", "+bitexact"]
VIDEO_IN = ["-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(RATE)]
AUDIO_IN = ["-f", "f32le", "-ar", str(AUDIO_RATE), "-ac", "2"]
VP8 = ["-c:v", "libvpx", "-pix_fmt", "yuv420p", "-g", "5", "-b:v", "1M", "-threads", "1"]
THEORA = ["-c:v", "libtheora", "-pix_fmt", "yuv420p", "-g", "5", "-q:v", "8"]
OPUS = ["-c:a", "libopus", "-b:a", "96k"]
VORBIS = ["-c:a", "libvorbis", "-q:a", "5"]
# The audio starts 2 segments (0.5 s) after the video and carries segments 2 to 7, so the movie time of each tone is the
# same as in the file without delay.
LATE = 2


def main():
    os.makedirs(out_dir, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="xiter-websound-gen-") as tmp:
        v, a, late = (os.path.join(tmp, n) for n in ("v.rgb", "a.f32", "late.f32"))
        video_raw(v)
        audio_raw(a)
        audio_raw(late, start_segment=LATE)
        both = VIDEO_IN + ["-i", v] + AUDIO_IN + ["-i", a]
        delayed = VIDEO_IN + ["-i", v] + ["-itsoffset", str(LATE * SEGMENT)] + AUDIO_IN + ["-i", late]
        files = {
            "vp8-opus.webm": both + VP8 + OPUS,
            "vp8-vorbis.webm": both + VP8 + VORBIS,
            "vp8-only.webm": VIDEO_IN + ["-i", v] + VP8 + ["-an"],
            "vp8-opus-late.webm": delayed + VP8 + OPUS,
            "vp8-vorbis-late.webm": delayed + VP8 + VORBIS,
            "theora-vorbis.ogv": both + THEORA + VORBIS,
            "theora-opus.ogv": both + THEORA + OPUS,
            "theora-vorbis-late.ogv": delayed + THEORA + VORBIS,
            "theora-opus-late.ogv": delayed + THEORA + OPUS,
        }
        for name, args in files.items():
            run(args + COMMON + [os.path.join(out_dir, name)])

    lines = []
    for name in sorted(files):
        with open(os.path.join(out_dir, name), "rb") as f:
            data = f.read()
        lines.append(f"{hashlib.sha256(data).hexdigest()}  {name}")
        print(name, len(data), "bytes")
    with open(os.path.join(out_dir, "SHA256SUMS"), "w") as f:
        f.write("\n".join(lines) + "\n")
    for name in BROWSER:
        shutil.copyfile(os.path.join(out_dir, name), os.path.join(web_dir, name))


if __name__ == "__main__":
    main()
