#!/usr/bin/env python3
"""Copy the media files that the frozen corpus-items .wrl files reference.

For every .wrl in <snapshot> (gzip-aware, in memory), find quoted url strings, resolve
relative ones against the same directory in the live new-items tree, and copy the
file into <dest> at the same relative path if it exists. Source opened "rb" only.
Kept separate from corpus-items so the X3-X6 answer key (made without media) stays valid.

Usage: snapshot_item_media.py <corpus-items dir> <dest dir>
"""
import gzip, hashlib, json, os, re, sys, shutil

SRC = "/home/ryan/Projects/cybertown/new-items/item-categories"
MEDIA = (".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".mp4", ".webm", ".ogv", ".ogg",
         ".mp3", ".wav", ".mid", ".midi", ".ttf", ".otf", ".woff", ".woff2")
URL = re.compile(rb'"([^"\r\n]{1,400})"')

def main():
    snap, dest = sys.argv[1], sys.argv[2]
    os.makedirs(dest, exist_ok=False)
    copied, missing = {}, set()
    for dirpath, _, names in os.walk(snap):
        for name in names:
            if not name.lower().endswith(".wrl"):
                continue
            with open(os.path.join(dirpath, name), "rb") as fh:
                data = fh.read()
            if data[:2] == b"\x1f\x8b":
                data = gzip.decompress(data)
            reldir = os.path.relpath(dirpath, snap)
            for m in URL.finditer(data):
                u = m.group(1).decode("latin-1").split("#")[0].split("?")[0]
                if "://" in u or u.startswith(("urn:", "data:", "javascript:", "vrmlscript:", "/")):
                    continue
                if not u.lower().endswith(MEDIA):
                    continue
                rel = os.path.normpath(os.path.join(reldir, u))
                if rel.startswith(".."):
                    missing.add(rel); continue
                src = os.path.join(SRC, rel)
                if rel in copied:
                    continue
                if os.path.isfile(src) and not os.path.islink(src):
                    out = os.path.join(dest, rel)
                    os.makedirs(os.path.dirname(out), exist_ok=True)
                    with open(src, "rb") as fi, open(out, "xb") as fo:
                        b = fi.read(); fo.write(b)
                    copied[rel] = hashlib.sha256(b).hexdigest()
                else:
                    missing.add(rel)
    with open(os.path.join(dest, "PROVENANCE.json"), "x", encoding="utf-8") as fh:
        json.dump({"source": SRC, "files": copied, "referenced_but_missing": sorted(missing)}, fh, indent=1)
    print("copied", len(copied), "missing", len(missing))

main()
