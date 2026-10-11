#!/usr/bin/env python3
"""Generate x10-scripts/{plane,lookat}-*.json (the PLANE and LOOKAT viewers, new files only).

Worlds (hand-written, same directory): plane-nav.wrl, lookat-nav.wrl (perspective Viewpoint),
plane-ortho.wrl, lookat-ortho.wrl (OrthoViewpoint). 512x512, dt = 1/30. Pointer y is measured from
the bottom of the canvas. All events use "t": 0, so Date.now () is the previous frame's time.

JS facts the scripts rely on (PlaneViewer.js / LookAtViewer.js in the pin):
  * PLANE pans with the MIDDLE button (or Alt + left); a plain left drag does nothing.
  * PLANE and LOOKAT wheel: PLANE changes fieldOfViewScale (and the translation that keeps the point
    under the pointer fixed), LOOKAT moves the viewpoint toward the center of rotation with a chaser.
  * LOOKAT: left drag looks around (trackball + OrientationChaser); a double click (no Alt) looks at the
    bounding box of the clicked shape, with Alt at the clicked point (X3DViewer.lookAtBBox / lookAtPoint).
"""
import json, os
out = os.path.join(os.path.dirname(__file__), "..", "x10-scripts")

def write(name, frames, events, capture=None, dump=None):
    ev = sorted(events, key=lambda e: e["frame"])
    cap = capture or [frames]
    d = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "capture": cap, "dump": dump or [], "frames": frames, "events": ev}
    json.dump(d, open(os.path.join(out, name + ".json"), "w"), indent=1)

def drag(button, start, steps, f0=2, per_frame=1, end_extra=0):
    """mousedown at f0, `steps` points (one or `per_frame` moves per frame), mouseup after."""
    ev = [{"frame": f0, "type": "mousedown", "button": button, "x": start[0], "y": start[1]}]
    f = f0 + 1
    pts = list(steps)
    i = 0
    while i < len(pts):
        for p in pts[i:i + per_frame]:
            ev.append({"frame": f, "type": "mousemove", "x": p[0], "y": p[1]})
        i += per_frame
        f += 1
    last = pts[-1]
    ev.append({"frame": f + end_extra, "type": "mouseup", "button": button, "x": last[0], "y": last[1]})
    return ev, f + end_extra

def line(start, d, n):
    return [(start[0] + d[0] * (i + 1), start[1] + d[1] * (i + 1)) for i in range(n)]

def wheel(frames, delta, x, y):
    return [{"frame": f, "type": "wheel", "deltaY": delta, "x": x, "y": y} for f in frames]

# --- PLANE -----------------------------------------------------------------------------------
for world in ("plane-nav", "plane-ortho"):
    tag = "plane" if world == "plane-nav" else "plane-ortho"
    ev, last = drag(1, (256, 256), line((256, 256), (9, 6), 12))
    write(f"{tag}-drag", last + 25, ev, capture=[6, 14, last + 25])
    ev, last = drag(0, (256, 256), line((256, 256), (9, 6), 12))
    write(f"{tag}-drag-left", last + 5, ev, capture=[6, last + 5])
    ev = [{"frame": 2, "type": "mods", "shift": False, "ctrl": False, "alt": True}]
    d, last = drag(0, (256, 256), line((256, 256), (-7, 9), 10), f0=3)
    ev += d + [{"frame": last + 1, "type": "mods", "shift": False, "ctrl": False, "alt": False}]
    write(f"{tag}-drag-alt", last + 25, ev, capture=[8, last + 25])

ev = wheel(range(2, 8), -100, 300, 200) + wheel(range(20, 30), 100, 300, 200) + wheel(range(40, 44), -100, 120, 400)
write("plane-wheel", 60, ev, capture=[5, 12, 35, 60])
# a wheel zoom out far enough to meet constrainFieldOfViewScale (Viewpoint: fov * scale < pi)
write("plane-wheel-out", 90, wheel(range(2, 80), 100, 256, 256), capture=[30, 90])

mid, last = drag(1, (256, 256), line((256, 256), (8, -5), 10), f0=2)
ev = mid + wheel(range(last + 3, last + 9), -100, 200, 300) + wheel(range(last + 15, last + 20), 100, 320, 180)
write("plane-ortho-wheel", last + 30, wheel(range(2, 8), -100, 300, 200) + wheel(range(20, 28), 100, 300, 200), capture=[5, 12, 35])
write("plane-ortho", last + 30, ev, capture=[last + 1, last + 10, last + 30])

# several mouse moves per frame while panning (PLANE applies each move at once)
ev, last = drag(1, (256, 256), line((256, 256), (3, 2), 36), per_frame=6)
write("plane-drag-multi", last + 8, ev, capture=[4, last + 8])
# wheel inside a pan
pan, last = drag(1, (256, 256), line((256, 256), (6, 4), 12), f0=2)
inner = wheel([6], -100, 256 + 6 * 4, 256 + 4 * 4) + wheel([7], -100, 256 + 6 * 5, 256 + 4 * 5)  # each on the pointer of its frame
write("plane-wheel-in-drag", last + 10, pan + inner, capture=[8, last + 10])

# --- LOOKAT ----------------------------------------------------------------------------------
for world in ("lookat-nav", "lookat-ortho"):
    tag = "lookat" if world == "lookat-nav" else "lookat-ortho"
    cap = [3, 6, 12, 20, 40, 50]
    write(f"{tag}-click", 50, [{"frame": 2, "type": "dblclick", "x": 256, "y": 256}], capture=cap)
    write(f"{tag}-click-miss", 20, [{"frame": 2, "type": "dblclick", "x": 20, "y": 490}], capture=[3, 20])
    ev = [{"frame": 2, "type": "mods", "shift": False, "ctrl": False, "alt": True},
          {"frame": 3, "type": "dblclick", "x": 256, "y": 256},
          {"frame": 4, "type": "mods", "shift": False, "ctrl": False, "alt": False}]
    write(f"{tag}-click-alt", 55, ev, capture=[4, 10, 25, 55])
    ev = [{"frame": 2, "type": "dblclick", "x": 400, "y": 300}, {"frame": 60, "type": "dblclick", "x": 256, "y": 40}]
    write(f"{tag}-click-twice", 120, ev, capture=[40, 70, 100, 120])
    # a second double click while the first transition is still running
    write(f"{tag}-click-interrupt", 80, [{"frame": 2, "type": "dblclick", "x": 400, "y": 300}, {"frame": 12, "type": "dblclick", "x": 256, "y": 256}], capture=[11, 20, 45, 80])

ev, last = drag(0, (256, 256), line((256, 256), (-9, 6), 12))
write("lookat-drag", last + 30, ev, capture=[6, 14, last + 30])
ev, last = drag(0, (256, 256), line((256, 256), (-4, 3), 36), per_frame=6)
write("lookat-drag-multi", last + 30, ev, capture=[4, last + 30])
ev, last = drag(0, (256, 256), line((256, 256), (11, -8), 14))
write("lookat-drag-then-click", 100, ev + [{"frame": last + 40, "type": "dblclick", "x": 256, "y": 256}], capture=[last + 30, last + 50, 100])
write("lookat-wheel", 90, wheel(range(2, 8), -100, 256, 256) + wheel(range(30, 40), 100, 256, 256), capture=[5, 12, 35, 60, 90])
ev, last = drag(0, (256, 256), line((256, 256), (-9, 6), 12))
write("lookat-wheel-in-drag", last + 30, ev + wheel([6], -100, 256 - 9 * 4, 256 + 6 * 4) + wheel([7], -100, 256 - 9 * 5, 256 + 6 * 5), capture=[8, last + 30])
