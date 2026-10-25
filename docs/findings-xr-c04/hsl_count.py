import json
import math
import struct

def parse_rgba(s):
    if s.startswith("rgb("): s = s[4:-1]
    elif s.startswith("rgba("): s = s[5:-1]
    else: return None
    parts = s.split(",")
    return [int(parts[0].strip()), int(parts[1].strip()), int(parts[2].strip())]

def parse_hsl(s):
    if not s.startswith("hsl("): return None
    s = s[4:-1]
    args = s.replace(",", " ").split()
    args = [a for a in args if a and a != '/']
    h_str = args[0]
    if h_str.endswith("deg"): h = float(h_str[:-3])
    elif h_str.endswith("grad"): h = float(h_str[:-4]) * 0.9
    elif h_str.endswith("rad"): h = float(h_str[:-3]) * 180.0 / math.pi
    elif h_str.endswith("turn"): h = float(h_str[:-4]) * 360.0
    else: h = float(h_str)
    s_val = float(args[1][:-1]) / 100.0
    l_val = float(args[2][:-1]) / 100.0
    return h, max(0.0, min(1.0, s_val)), max(0.0, min(1.0, l_val))

def f32(val): return struct.unpack('!f', struct.pack('!f', float(val)))[0]
def round_half_up(val): return math.floor(val + 0.5)
def round_half_away(val): return math.floor(val + 0.5) if val >= 0 else math.ceil(val - 0.5)

def calc_color(h, s, l, algo):
    if algo == "rust_current":
        hue = (h % 360.0) / 60.0
        t2 = l * (s + 1.0) if l <= 0.5 else l + s - l * s
        t1 = l * 2.0 - t2
        def f(hv):
            if hv < 0.0: hv += 6.0
            if hv >= 6.0: hv -= 6.0
            if hv < 1.0: v = (t2 - t1) * hv + t1
            elif hv < 3.0: v = t2
            elif hv < 4.0: v = (t2 - t1) * (4.0 - hv) + t1
            else: v = t1
            return round_half_up(f32(f32(v) * f32(255.0)))
        return [int(f(hue + 2.0)), int(f(hue)), int(f(hue - 2.0))]

    elif algo == "legacy_f32":
        hue = f32(f32(h % 360.0) / f32(60.0))
        t2 = f32(f32(l) * f32(s + 1.0)) if l <= 0.5 else f32(f32(l + s) - f32(l * s))
        t1 = f32(f32(2.0 * l) - t2)
        def f(hv):
            if hv < 0.0: hv = f32(hv + 6.0)
            if hv >= 6.0: hv = f32(hv - 6.0)
            if hv < 1.0: v = f32(f32(f32(t2 - t1) * hv) + t1)
            elif hv < 3.0: v = t2
            elif hv < 4.0: v = f32(f32(f32(t2 - t1) * f32(4.0 - hv)) + t1)
            else: v = t1
            return round_half_away(f32(v * 255.0))
        return [f(f32(hue + 2.0)), f(hue), f(f32(hue - 2.0))]

    elif algo == "gfx_f32":
        hue = f32(f32(h % 360.0) / f32(360.0))
        t2 = f32(f32(l) * f32(s + 1.0)) if l <= 0.5 else f32(f32(l + s) - f32(l * s))
        t1 = f32(f32(2.0 * l) - t2)
        def f(hv):
            if hv < 0.0: hv = f32(hv + 1.0)
            if hv > 1.0: hv = f32(hv - 1.0)
            if f32(6.0 * hv) < 1.0: v = f32(t1 + f32(f32(t2 - t1) * f32(6.0 * hv)))
            elif f32(2.0 * hv) < 1.0: v = t2
            elif f32(3.0 * hv) < 2.0: v = f32(t1 + f32(f32(t2 - t1) * f32(f32(2.0/3.0 - hv) * 6.0)))
            else: v = t1
            return round_half_up(f32(v * 255.0))
        return [f(f32(hue + 1.0/3.0)), f(hue), f(f32(hue - 1.0/3.0))]

    elif algo == "legacy_f64":
        hue = (h % 360.0) / 60.0
        t2 = l * (s + 1.0) if l <= 0.5 else l + s - l * s
        t1 = l * 2.0 - t2
        def f(hv):
            if hv < 0.0: hv += 6.0
            if hv >= 6.0: hv -= 6.0
            if hv < 1.0: v = (t2 - t1) * hv + t1
            elif hv < 3.0: v = t2
            elif hv < 4.0: v = (t2 - t1) * (4.0 - hv) + t1
            else: v = t1
            return round_half_away(v * 255.0)
        return [f(hue + 2.0), f(hue), f(hue - 2.0)]

    elif algo == "gfx_f64":
        hue = (h % 360.0) / 360.0
        t2 = l * (s + 1.0) if l <= 0.5 else l + s - l * s
        t1 = l * 2.0 - t2
        def f(hv):
            if hv < 0.0: hv += 1.0
            if hv > 1.0: hv -= 1.0
            if 6.0 * hv < 1.0: v = t1 + (t2 - t1) * 6.0 * hv
            elif 2.0 * hv < 1.0: v = t2
            elif 3.0 * hv < 2.0: v = t1 + (t2 - t1) * (2.0 / 3.0 - hv) * 6.0
            else: v = t1
            return round_half_up(v * 255.0)
        return [f(hue + 1.0/3.0), f(hue), f(hue - 1.0/3.0)]

def main():
    with open('oracle/synthetic-rea-svg/css-colors-baseline.json') as f: data = json.load(f)
    algos = ["rust_current", "legacy_f32", "gfx_f32", "legacy_f64", "gfx_f64"]
    mismatches = {algo: 0 for algo in algos}

    cases_43 = []

    for value, expected in data:
        if not expected: continue
        hsl = parse_hsl(value)
        if not hsl: continue
        e_rgb = parse_rgba(expected)
        if not e_rgb: continue

        for algo in algos:
            a_rgb = calc_color(hsl[0], hsl[1], hsl[2], algo)
            if a_rgb and (a_rgb[0] != e_rgb[0] or a_rgb[1] != e_rgb[1] or a_rgb[2] != e_rgb[2]):
                mismatches[algo] += 1
                if algo == "rust_current":
                    cases_43.append((value, a_rgb, e_rgb, hsl))

    for algo in algos: print(f"{algo}: {mismatches[algo]} mismatches")

    print("\n--- 43 Verified Cases ---")
    for value, a_rgb, e_rgb, hsl in cases_43:
        h, s, l = hsl
        hue = (h % 360.0) / 60.0
        t2 = l * (s + 1.0) if l <= 0.5 else l + s - l * s
        t1 = l * 2.0 - t2
        def exact_f(hv):
            if hv < 0.0: hv += 6.0
            if hv >= 6.0: hv -= 6.0
            if hv < 1.0: return (t2 - t1) * hv + t1
            elif hv < 3.0: return t2
            elif hv < 4.0: return (t2 - t1) * (4.0 - hv) + t1
            else: return t1

        c_idx = next(i for i in range(3) if a_rgb[i] != e_rgb[i])
        exact_val = exact_f(hue + 2.0 if c_idx == 0 else hue if c_idx == 1 else hue - 2.0) * 255.0
        print(f"Input: {value}, Differs: {['R','G','B'][c_idx]}, exact={exact_val:.1f}, Chrome: {e_rgb[c_idx]}, Rust: {a_rgb[c_idx]}")

if __name__ == "__main__": main()
