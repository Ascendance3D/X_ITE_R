"""Compare depth-fighting between two jitter sets (x3d-zfight --color-dir / x_ite-zfight.mjs layout).

usage: python3 -I oracle/tools/zfight_compare.py <dirA> <dirB> [--frames 15,64] [--thr 8] [--save dir]

A pixel "flickers" in a set when, for any jitter i, its color differs from jitter 0 by more than
--thr in some channel while the pixel sits in a flat area of image 0 (every 3x3 neighbor within
--thr of it), so silhouette and shading-edge pixels are not counted. Prints, per frame, the
counts for A and B, the overlap (exact and within 1 pixel) and each image's own flicker density.
"""
import sys, os, numpy as np
from PIL import Image

def load(d, k, i):
    return np.asarray(Image.open(os.path.join(d, str(k), f"{i}.png")).convert("RGB")).astype(np.int16)

def flicker(d, k, n, thr):
    base = load(d, k, 0)
    h, w, _ = base.shape
    flat = np.ones((h, w), bool)
    pad = np.pad(base, ((1, 1), (1, 1), (0, 0)), mode="edge")
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            nb = pad[1 + dy:1 + dy + h, 1 + dx:1 + dx + w]
            flat &= (np.abs(nb - base).max(axis=2) <= thr)
    ch = np.zeros((h, w), bool)
    for i in range(1, n):
        ch |= (np.abs(load(d, k, i) - base).max(axis=2) > thr)
    return ch & flat, base

def dil(m):
    p = np.pad(m, 1)
    o = np.zeros_like(m)
    for dy in (0, 1, 2):
        for dx in (0, 1, 2):
            o |= p[dy:dy + m.shape[0], dx:dx + m.shape[1]]
    return o

def main():
    a, b = sys.argv[1], sys.argv[2]
    args = sys.argv[3:]
    opt = lambda f, d=None: args[args.index(f) + 1] if f in args else d
    thr = int(opt("--thr", 8))
    frames = [int(x) for x in opt("--frames", ",".join(sorted(os.listdir(a), key=lambda s: int(s) if s.isdigit() else 0)).replace("result.json", "")).split(",") if x]
    save = opt("--save")
    for k in frames:
        n = min(len([f for f in os.listdir(os.path.join(d, str(k))) if f.endswith(".png")]) for d in (a, b))
        fa, ba = flicker(a, k, n, thr)
        fb, bb = flicker(b, k, n, thr)
        base_over = int((np.abs(ba - bb).max(axis=2) > thr).sum())
        both = int((fa & fb).sum())
        near = int((fa & dil(fb)).sum())
        print(f"frame {k}: A flicker {int(fa.sum())}, B flicker {int(fb.sum())}, exact overlap {both}, A within 1px of B {near} ({100.0 * near / max(1, int(fa.sum())):.0f}%), base images differing > {thr}: {base_over}")
        if save:
            os.makedirs(save, exist_ok=True)
            out = np.zeros(fa.shape + (3,), np.uint8)
            out[fa] = (255, 0, 0)
            out[fb] = (0, 160, 255)
            out[fa & fb] = (255, 255, 255)
            Image.fromarray(out).save(os.path.join(save, f"{k}.png"))

main()
