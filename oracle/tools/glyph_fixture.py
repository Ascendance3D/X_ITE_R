#!/usr/bin/env python3
"""Glyph geometry fixture: every libtess glyph triangle vertex of the 12 X_ITE default fonts, from the pinned X_ITE.

  node oracle/x_ite-text.mjs /tmp/x_ite-text-all.json --all-glyphs
  python3 -I oracle/tools/glyph_fixture.py build /tmp/x_ite-text-all.json   # writes the fixture
  python3 -I oracle/tools/glyph_fixture.py check                            # the fixture is consistent

The fixture is crates/x3d-text/tests/data/glyph-geometry/: manifest.json (identities, settings, per-glyph counts
and SHA-256) and vertices.f64le.gz (all vertex coordinates, little-endian f64, in manifest order; gzip with mtime 0,
so a rebuild from the same recording gives the same bytes). crates/x3d-text/tests/glyph_geometry.rs compares
x3d-text with it. It is separate from the protected answer key oracle/x_ite-text.json, which it does not read or change.
"""
import gzip, hashlib, json, os, struct, sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
OUT = os.path.join(ROOT, "crates", "x3d-text", "tests", "data", "glyph-geometry")
FONT_DIR = os.path.join(ROOT, "oracle", "x_ite-pin", "src", "assets", "fonts")
FAMILIES = ["SERIF", "SANS", "TYPEWRITER"]
STYLES = ["PLAIN", "ITALIC", "BOLD", "BOLDITALIC"]
# The default font files (crates/x3d-text/src/fonts.rs default_font_file, X_ITE X3DFontStyleNode), in STYLES order.
FILES = {
    "SERIF": ["Droid/DroidSerif-Regular.woff2", "Droid/DroidSerif-Italic.woff2", "Droid/DroidSerif-Bold.woff2", "Droid/DroidSerif-BoldItalic.woff2"],
    "SANS": ["Ubuntu/Ubuntu-R.woff2", "Ubuntu/Ubuntu-RI.woff2", "Ubuntu/Ubuntu-B.woff2", "Ubuntu/Ubuntu-BI.woff2"],
    "TYPEWRITER": ["Ubuntu/UbuntuMono-R.woff2", "Ubuntu/UbuntuMono-RI.woff2", "Ubuntu/UbuntuMono-B.woff2", "Ubuntu/UbuntuMono-BI.woff2"],
}
CHARS = "8AaWgeE&@Qi.,ofyéüß"


def sha256(b):
    return hashlib.sha256(b).hexdigest()


def build(recording):
    raw = open(recording, "rb").read()
    rec = json.loads(raw)
    stream = bytearray()
    fonts = []
    for family in FAMILIES:
        for i, style in enumerate(STYLES):
            name = f"{family}-{style}"
            info = rec["fonts"][name]
            if not info.get("loaded"):
                raise SystemExit(f"{name}: not loaded in the recording")
            if "".join(info["glyphs"]) != CHARS:
                raise SystemExit(f"{name}: characters {''.join(info['glyphs'])!r}, want {CHARS!r}")
            glyphs = []
            for ch, g in info["glyphs"].items():
                v = g["vertices"]
                if len(v) != g["triangles"] * 6:
                    raise SystemExit(f"{name} {ch!r}: {len(v)} coordinates for {g['triangles']} triangles (not full)")
                b = b"".join(struct.pack("<d", float(x)) for x in v)
                glyphs.append({"char": ch, "index": g["index"], "advance": g["advance"], "triangles": g["triangles"],
                               "area": g["area"], "offset": len(stream) // 8, "count": len(v), "sha256": sha256(b)})
                stream += b
            path = FILES[family][i]
            fonts.append({"name": name, "file": path, "file_sha256": sha256(open(os.path.join(FONT_DIR, path), "rb").read()),
                          "unitsPerEm": info["unitsPerEm"], "glyphs": glyphs})
    gz = gzip.compress(bytes(stream), compresslevel=9, mtime=0)
    manifest = {
        "format": "x3d-text glyph geometry 1",
        "reference": "X_ITE 16.5.0, oracle/x_ite-pin (34cdf9f97), PolygonText glyph triangulation (libtess)",
        "recorder": "oracle/x_ite-text.mjs --all-glyphs (headless Chrome, page oracle/x_ite-pin/src/shot.html)",
        "browser": rec.get("browser"),
        "recording_sha256": sha256(raw),
        "settings": {"quality": "MEDIUM (PrimitiveQuality::Medium; X_ITE default)", "units": "em (font units / unitsPerEm)",
                     "vertex": "x, y per point; 3 points per triangle", "bound": "exact (f64 bits)"},
        "characters": CHARS,
        "vertices_file": "vertices.f64le.gz",
        "vertices_gz_sha256": sha256(gz),
        "vertices_raw_sha256": sha256(bytes(stream)),
        "coordinates": len(stream) // 8,
        "fonts": fonts,
    }
    os.makedirs(OUT, exist_ok=True)
    open(os.path.join(OUT, "vertices.f64le.gz"), "wb").write(gz)
    open(os.path.join(OUT, "manifest.json"), "w").write(json.dumps(manifest, indent=1, ensure_ascii=False) + "\n")
    print(f"{len(fonts)} fonts, {sum(len(f['glyphs']) for f in fonts)} glyphs, {len(stream) // 8} coordinates, gz {len(gz)} bytes")


def check():
    m = json.load(open(os.path.join(OUT, "manifest.json")))
    gz = open(os.path.join(OUT, m["vertices_file"]), "rb").read()
    bad = []
    if sha256(gz) != m["vertices_gz_sha256"]:
        bad.append("vertices gz sha256")
    raw = gzip.decompress(gz)
    if sha256(raw) != m["vertices_raw_sha256"] or len(raw) != m["coordinates"] * 8:
        bad.append("vertices raw sha256 or length")
    n = 0
    for f in m["fonts"]:
        if sha256(open(os.path.join(FONT_DIR, f["file"]), "rb").read()) != f["file_sha256"]:
            bad.append(f"{f['name']}: font file sha256")
        for g in f["glyphs"]:
            if g["offset"] != n or sha256(raw[n * 8:(n + g["count"]) * 8]) != g["sha256"]:
                bad.append(f"{f['name']} {g['char']!r}: offset or sha256")
            n += g["count"]
    if n != m["coordinates"]:
        bad.append("coordinate total")
    print("\n".join(bad) or f"OK: {len(m['fonts'])} fonts, {sum(len(f['glyphs']) for f in m['fonts'])} glyphs, {n} coordinates")
    return not bad


if __name__ == "__main__":
    if sys.argv[1:2] == ["build"] and len(sys.argv) == 3:
        build(sys.argv[2])
        sys.exit(0 if check() else 1)
    if sys.argv[1:] == ["check"]:
        sys.exit(0 if check() else 1)
    raise SystemExit(__doc__)
