#!/usr/bin/env python3
"""Freeze a copy of new-items/item-categories test inputs (Ryan-authored Mall items).

Read-only on the source: opens with "rb", writes only under <dest>. Keeps the
directory layout so relative Inline/EXTERNPROTO paths between .wrl files still resolve.
Copies .wrl, and .js/.json except bundled X_ITE preview copies and minified files.

Usage: snapshot_items.py [--no-edit] <dest_dir>

--no-edit (corpus-items2): also skip *.edit.wrl (plain-text editing copies; the real item
is the gzipped .wrl beside it), qa / qa-* folders (test copies, not items), and scratch/backup .wrl (tmp_*, tmp-*, *.bak*). The skipped
and kept lists are written into PROVENANCE.json. Without the flag the behavior is unchanged.
"""
import hashlib, json, os, subprocess, sys

SRC_REPO = "/home/ryan/Projects/cybertown/new-items"
SRC = os.path.join(SRC_REPO, "item-categories")

NO_EDIT = False

def wanted(rel):
    parts = rel.split(os.sep)
    base = os.path.basename(rel)
    if NO_EDIT and rel.endswith(".wrl") and (base.startswith(("tmp_", "tmp-")) or ".bak" in base):
        return False
    if NO_EDIT and (rel.endswith(".edit.wrl") or any(p == "qa" or p.startswith("qa-") for p in parts)):
        return False
    if "preview" in parts or "node_modules" in parts or any(p.startswith(".") for p in parts):
        return False
    if rel.endswith(".wrl"):
        return True
    return (rel.endswith(".js") or rel.endswith(".json")) and not rel.endswith(".min.js")

def main():
    global NO_EDIT
    args = sys.argv[1:]
    if "--no-edit" in args:
        NO_EDIT = True
        args.remove("--no-edit")
    dest = args[0]
    os.makedirs(dest, exist_ok=False)
    files = []
    for dirpath, dirnames, filenames in os.walk(SRC):
        dirnames.sort()
        for name in sorted(filenames):
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, SRC)
            if os.path.islink(full) or not wanted(rel):
                continue
            with open(full, "rb") as fh:
                data = fh.read()
            out = os.path.join(dest, rel)
            os.makedirs(os.path.dirname(out), exist_ok=True)
            with open(out, "xb") as fh:
                fh.write(data)
            files.append({"rel": rel, "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)})
    head = subprocess.run(["git", "-C", SRC_REPO, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "-C", SRC_REPO, "status", "--porcelain", "--", "item-categories"],
                           capture_output=True, text=True).stdout.count("\n")
    with open(os.path.join(dest, "PROVENANCE.json"), "x", encoding="utf-8") as fh:
        json.dump({"source": SRC, "head": head, "dirty_paths_in_item_categories": dirty, "no_edit": NO_EDIT,
                   "files": files}, fh, indent=1)
    kinds = {}
    for f in files:
        k = f["rel"].rsplit(".", 1)[-1]
        kinds[k] = kinds.get(k, 0) + 1
    print("copied", len(files), kinds, "head", head[:9], "dirty", dirty)

main()
