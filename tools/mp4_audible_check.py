"""Finds the tones of h264-aac.mp4 in a recording (tools/mp4-audible.sh).

The file sounds 8 segments of 0.25 s: 300, 400, ... 1000 Hz. The recording is raw float32 little-endian stereo at
48 kHz. The script reads 20 ms windows every 10 ms, names the strongest of the 8 tones in each window that is not
silent, and prints the runs. It passes when the runs are the 8 tones in order, each 0.18 s to 0.3 s long.
"""

import math
import struct
import sys

RATE = 48000
TONES = [300 + 100 * k for k in range(8)]


def power(s, f):
    w = 2 * math.pi * f / RATE
    c = 2 * math.cos(w)
    a = b = 0.0
    for x in s:
        a, b = x + c * a - b, a
    return a * a + b * b - c * a * b


def main(path):
    raw = open(path, "rb").read()
    # parec writes whole stereo float32 frames; anything else is not this script's recording.
    if len(raw) % 8 or len(raw) < 8 * RATE:
        print(f"result: FAIL (the recording has {len(raw)} bytes: not at least 1 s of stereo float32)")
        return 1
    n = len(raw) // 8
    left = [v for v in struct.unpack(f"<{2 * n}f", raw[: 8 * n])[0::2]]
    win, hop = RATE // 50, RATE // 100
    runs = []
    for start in range(0, max(0, n - win), hop):
        s = left[start : start + win]
        rms = math.sqrt(sum(x * x for x in s) / win)
        tone = None if rms < 0.02 else max(range(8), key=lambda k: power(s, TONES[k]))
        t = start / RATE
        if runs and runs[-1][0] == tone:
            runs[-1][2] = t
        else:
            runs.append([tone, t, t])
    sounding = [r for r in runs if r[0] is not None and r[2] - r[1] >= 0.05]
    print(f"recording: {n / RATE:.2f} s")
    for tone, a, b in sounding:
        print(f"  {TONES[tone]:4d} Hz from {a:.3f} s to {b:.3f} s ({b - a:.3f} s)")
    order = [r[0] for r in sounding]
    ok = order == list(range(8)) and all(0.18 <= b - a <= 0.3 for _, a, b in sounding)
    print("result:", "the 8 tones, in order" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
