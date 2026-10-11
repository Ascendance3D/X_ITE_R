#!/usr/bin/env python3
"""Generate x10-scripts/rotate-then-zoom-*.json: a rotate drag (mousedown frame 2, a move per frame
3..14, mouseup frame 15) followed by wheel notches that start before the rotation chaser settles.
Variants: wheel from release+0/1/2/5 frames (zoom in), the same zooming out, and wheel during the
drag. Uses the 1024x768 geometry of msaa-orrery-*."""
import json, os
out = os.path.join(os.path.dirname(__file__), "..", "x10-scripts")
def script(first, delta, notches=6, frames=70, name=None):
    ev = [{"frame": 2, "type": "mousedown", "button": 0, "x": 512, "y": 384}]
    for i in range(12):
        ev.append({"frame": 3 + i, "type": "mousemove", "x": 512 - 14 * (i + 1), "y": 384 + 9 * (i + 1)})
    ev.append({"frame": 15, "type": "mouseup", "button": 0, "x": 344, "y": 492})
    for i in range(notches):
        f = first + i
        # X_ITE's harness moves the mouse to the wheel's x,y first, so a wheel inside the drag must sit
        # on the pointer or it is also a rotate step the Rust driver does not see.
        k = min(f, 14) - 2
        x, y = (512 - 14 * k, 384 + 9 * k) if f < 15 else ((344, 492) if f == 15 else (512, 384))
        ev.append({"frame": f, "type": "wheel", "deltaY": delta, "x": x, "y": y})
    ev.sort(key=lambda e: e["frame"])
    d = {"size": [1024, 768], "t0": 946684800, "dt": 1 / 30, "capture": [frames], "dump": [], "frames": frames, "events": ev}
    json.dump(d, open(os.path.join(out, name + ".json"), "w"), indent=1)
for k in (0, 1, 2, 5):
    script(15 + k, -100, name=f"rotate-then-zoom-in-r{k}")
script(17, 100, name="rotate-then-zoom-out-r2")
script(15, 100, name="rotate-then-zoom-out-r0")
script(9, -100, name="rotate-during-zoom-in-d9")
script(9, 100, name="rotate-during-zoom-out-d9")
