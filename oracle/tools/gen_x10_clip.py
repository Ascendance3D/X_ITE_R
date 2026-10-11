#!/usr/bin/env python3
"""Write the X10 clipping regression scripts (EXAMINE drag/zoom stress) into oracle/x10-scripts/.
New files only; the existing scripts are never rewritten. Run: python3 oracle/tools/gen_x10_clip.py
Each scenario is written at 512x512 and at 1024x768 (x3d-view's default window); `-multi`
variants send several mouse moves per frame, like a real pointing device does."""
import json, math, os
out = os.path.join(os.path.dirname(__file__), "..", "x10-scripts")
DT = 1 / 30

def write(name, s):
    with open(os.path.join(out, name), "w") as fh:
        json.dump(s, fh, indent=1)

def gen(size, tag, per_frame=1):
    W, H = size
    cx, cy = W // 2, H // 2
    sx, sy = W / 512, H / 512

    def base(frames, capture):
        return {"size": [W, H], "t0": 946684800, "dt": DT, "capture": capture, "dump": [], "frames": frames, "events": []}

    def drag(ev, f, button, pts):
        """pts are spread over frames: per_frame moves each."""
        ev.append({"frame": f, "type": "mousedown", "button": button, "x": cx, "y": cy})
        for i, (px, py) in enumerate(pts):
            ev.append({"frame": f + 1 + i // per_frame, "type": "mousemove", "x": int(px), "y": int(py)})
        last = f + 1 + (len(pts) - 1) // per_frame
        ev.append({"frame": last + 1, "type": "mouseup", "button": button, "x": int(pts[-1][0]), "y": int(pts[-1][1])})
        return last + 2

    def wheel(ev, f, n, dy):
        for i in range(n):
            ev.append({"frame": f + i, "type": "wheel", "deltaY": dy, "x": cx, "y": cy})

    n = 19 * per_frame
    # 1. big rotations: sweep across the whole canvas, several times.
    s = base(120, [30, 60, 90, 120]); f = 2
    for k in range(4):
        pts = [(cx + sx * 220 * math.sin(i / (3 * per_frame) + k), cy + sy * 180 * math.cos(i / (4 * per_frame) + k)) for i in range(1, n + 1)]
        f = drag(s["events"], f, 0, pts) + 2
    write(f"clip-rotate-large{tag}.json", s)

    # 2. zoom in close with the wheel, rotate while close, zoom in more.
    s = base(140, [40, 80, 110, 140])
    wheel(s["events"], 2, 30, -100)
    drag(s["events"], 40, 0, [(cx + sx * 12 * i / per_frame, cy + sy * 5 * i / per_frame) for i in range(1, n + 1)])
    wheel(s["events"], 100, 10, -100)
    write(f"clip-zoom-in-rotate{tag}.json", s)

    # 3. zoom out far with the wheel, rotate while far, zoom out more.
    s = base(140, [40, 80, 110, 140])
    wheel(s["events"], 2, 40, 100)
    drag(s["events"], 50, 0, [(cx + sx * 12 * i / per_frame, cy + sy * 5 * i / per_frame) for i in range(1, n + 1)])
    wheel(s["events"], 80, 40, 100)
    write(f"clip-zoom-out-rotate{tag}.json", s)

    # 4. middle-button (zoom) and right-button (pan) drags, then rotate. The captures avoid the
    # first frames of a drag with several moves per frame: X_ITE starts its chaser one frame
    # earlier than we do there (a transient of at most 6 frames; see oracle/README.md).
    s = base(120, [30, 75, 100, 120])
    m = 24 * per_frame
    f = drag(s["events"], 2, 1, [(cx, cy - sy * 8 * i / per_frame) for i in range(1, m + 1)])
    f = drag(s["events"], f + 2, 2, [(cx + sx * 6 * i / per_frame, cy) for i in range(1, m + 1)])
    drag(s["events"], f + 2, 0, [(cx + sx * 9 * i / per_frame, cy + sy * 9 * i / per_frame) for i in range(1, m + 1)])
    write(f"clip-pan-zoom-drag{tag}.json", s)

gen((512, 512), "")
gen((1024, 768), "-1024")
gen((1024, 768), "-1024-multi", per_frame=6)
