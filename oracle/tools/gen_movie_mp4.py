#!/usr/bin/env python3
"""Synthetic MP4 files for the MovieTexture MP4 path (ADR 0015).

  python3 -I oracle/tools/gen_movie_mp4.py

Every file is made here from generated samples, so no historical or third-party media is involved. The content
codes one timeline in both tracks: the movie has SEGMENTS segments of SEGMENT seconds; in segment k every picture
is the grey level GREY[k] and the audio is a sine of TONE[k] Hz. A test reads the segment of a picture from its
grey level and the segment of the sound from its frequency, so it can check that both tracks show the same
segment at the same movie time.

  crates/x3d-runtime/tests/data/mp4/<name>   the file
  crates/x3d-runtime/tests/data/mp4/SHA256SUMS

ffmpeg (with libx264, libx265 and libaom) is a development tool here: it is needed only to regenerate; the tests
read the committed files.
"""
import hashlib, math, os, struct, subprocess, tempfile

out_dir = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..",
                                        "crates", "x3d-runtime", "tests", "data", "mp4"))
W, H, RATE = 64, 48, 20
SEGMENT, SEGMENTS = 0.25, 8
GREY = [16 + 30 * k for k in range(SEGMENTS)]
TONE = [300 + 100 * k for k in range(SEGMENTS)]
AUDIO_RATE = 48000


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
H264 = ["-c:v", "libx264", "-profile:v", "baseline", "-pix_fmt", "yuv420p", "-g", "5", "-bf", "0", "-crf", "10",
        "-x264-params", "threads=1"]
HEVC = ["-c:v", "libx265", "-pix_fmt", "yuv420p", "-x265-params", "keyint=5:bframes=0:crf=10:pools=1:log-level=error",
        "-tag:v", "hvc1"]
AV1 = ["-c:v", "libaom-av1", "-pix_fmt", "yuv420p", "-g", "5", "-crf", "20", "-b:v", "0", "-cpu-used", "8",
       "-threads", "1"]
AAC = ["-c:a", "aac", "-b:a", "128k"]


def main():
    os.makedirs(out_dir, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="xiter-mp4-gen-") as tmp:
        v, a, late = (os.path.join(tmp, n) for n in ("v.rgb", "a.f32", "late.f32"))
        video_raw(v)
        audio_raw(a)
        audio_raw(late, start_segment=1)
        files = {
            "h264-aac.mp4": VIDEO_IN + ["-i", v] + AUDIO_IN + ["-i", a] + H264 + AAC,
            "h264-only.mp4": VIDEO_IN + ["-i", v] + H264 + ["-an"],
            "aac-only.m4a": AUDIO_IN + ["-i", a] + AAC + ["-vn"],
            # The audio starts one segment (0.25 s) after the video and carries segments 1 to 7, so the movie time of
            # each tone is the same as in h264-aac.mp4.
            "h264-aac-audio-late.mp4": VIDEO_IN + ["-i", v] + ["-itsoffset", str(SEGMENT)] + AUDIO_IN + ["-i", late]
            + H264 + AAC,
            "hevc-aac.mp4": VIDEO_IN + ["-i", v] + AUDIO_IN + ["-i", a] + HEVC + AAC,
            "av1-aac.mp4": VIDEO_IN + ["-i", v] + AUDIO_IN + ["-i", a] + AV1 + AAC,
        }
        for name, args in files.items():
            run(args + COMMON + ["-movflags", "+faststart", os.path.join(out_dir, name)])

    lines = []
    for name in sorted(files):
        with open(os.path.join(out_dir, name), "rb") as f:
            data = f.read()
        lines.append(f"{hashlib.sha256(data).hexdigest()}  {name}")
        print(name, len(data), "bytes")
    with open(os.path.join(out_dir, "SHA256SUMS"), "w") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
