import sys
src = open(sys.argv[1]).read() + "\n" + open(sys.argv[3]).read()
def between(start, end, after=0):
    i = src.index(start, after); j = src.index(end, i) + len(end)
    return src[i:j]
def balanced(start, after=0):
    # from start (which begins a function expression) to the matching close of the first {...} or (...) body
    i = src.index(start, after)
    k = i; depth = 0; seen = False
    while True:
        c = src[k]
        if c in '({[': depth += 1; seen = True
        elif c in ')}]': depth -= 1
        k += 1
        if seen and depth == 0 and src[k-1] == '}': return src[i:k]
items = []
items.append(("AUDIO_INIT", balanced("a =>\n   {\n      let context = null;")))
items.append(("PIN_FRAME_RATE", 'rate => Object .defineProperty (document .getElementById ("c") .browser, "currentFrameRate", { get: () => rate, configurable: true })'))
items.append(("PHYSICS_SETUP", balanced("async () =>\n      {\n         const browser = document .getElementById (\"c\") .browser;\n\n         await browser .loadComponents")))
for name, s in [("READY","() => window .ready"),("LOAD","([u, t, d, w]) => window .load (u, t, d, w)"),("SET_DT","d => window .setDt (d)"),
                ("GEOMETRY","() => window .geometry ()"),("FOCUS","() => window .focusCanvas ()"),("PHYS_SNAP","n => window .__physSnap (n)"),
                ("AUDIO_ARM","t => window .__audioArm (t)"),("AUDIO_WAIT","k => window .__audioWait (k)"),("AUDIO_FLUSH","() => window .__audioFlushDecodes ()"),
                ("SET_DATE","ms => window .setDate (ms)"),("SET_VIEWER","n => window .setViewer (n)"),
                ("RESIZE",'([ w, h ]) => { const c = document .getElementById ("c"); c .style .width = w + "px"; c .style .height = h + "px"; }'),
                ("TWO_FRAMES","() => new Promise (r => requestAnimationFrame (() => requestAnimationFrame (r)))"),
                ("PH_FRAME","k => { window .__phFrame = k; }"),("STEP","t => window .step (t)"),("AUDIO_RESUME","() => window .__audioResume ()"),
                ("VRML","() => window .vrml ()"),("PIXELS","() => window .pixels ()"),("PH_DT","() => window .__phDt"),
                ("GET_LOG","() => window .getLog ()"),("AUDIO_DONE","() => window .__audioDone ()"),("INFO","() => window .info ()")]:
    assert s in src, name
    items.append((name, s))
items.append(("YIELD_TASK", between("() => new Promise (resolve =>\n   {\n      const channel", "}))") [:-1]))
items.append(("POSE", balanced("() =>\n         {\n            // fieldOfViewScale")))
items.append(("INLINES", balanced("() =>\n         {\n            const\n               browser = document .getElementById (\"c\") .browser,\n               out     = { };")))
items.append(("PICK_PROBES", balanced("points =>\n      {")))
items.append(("COLLISION_PROBES", balanced("directions =>\n      {")))
out = ["//! Page-side JavaScript of oracle/x_ite-anim.mjs, byte for byte (the `page.evaluate` / `page.addInitScript`",
       "//! bodies). Generated from that file; the unit test `page_js_is_verbatim` checks every constant still occurs",
       "//! in oracle/x_ite-anim.mjs (and the MSAA / sized variants use the same text).", ""]
for name, s in items:
    assert src.count(s) >= 1, name
    h = "#" * 3
    assert '"' + h not in s
    out.append(f"pub const {name}: &str = r{h}\"{s}\"{h};\n")
open(sys.argv[2], "w").write("\n".join(out))
print(len(items))
