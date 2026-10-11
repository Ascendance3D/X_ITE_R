#!/usr/bin/env python3
"""Copy a deterministic sample of VRML97 files OUT of the read-only archives.

Reads recon/index/*.jsonl. Opens sources with mode "rb" only. Copies exact bytes
(compression state preserved). Writes only under the destination directory.
Skips any path in backup/IVN trees (member PII) and anything not VRML 2.0.

Usage: pick_corpus.py <dest_dir> <count>
"""
import hashlib, json, os, sys

INDEX = "/home/ryan/Projects/cybertown/X_ITE/recon/index"
SKIP = ("backup", "ivn", ".pem")

def main():
    dest, count = sys.argv[1], int(sys.argv[2])
    os.makedirs(dest, exist_ok=False)
    seen, recs = set(), []
    for name in ("skatefm.jsonl", "wbct.jsonl"):
        with open(os.path.join(INDEX, name), encoding="utf-8") as fh:
            for line in fh:
                r = json.loads(line)
                low = r["path"].lower()
                if any(s in low for s in SKIP):
                    continue
                if not (r.get("header") or "").startswith("#VRML V2.0"):
                    continue
                if (r.get("inflated_size") or r["raw_size"]) > 200_000:
                    continue
                if r["sha256"] in seen:
                    continue
                seen.add(r["sha256"])
                recs.append(r)
    # Spread across the corpus deterministically: order by content hash.
    recs.sort(key=lambda r: r["sha256"])
    step = max(1, len(recs) // count)
    picked = recs[::step][:count]
    prov = []
    for i, r in enumerate(picked):
        with open(r["path"], "rb") as fh:
            data = fh.read()
        if hashlib.sha256(data).hexdigest() != r["sha256"]:
            sys.exit("hash mismatch: %s" % r["path"])
        out = "%02d-%s" % (i, os.path.basename(r["rel"]))
        with open(os.path.join(dest, out), "xb") as fh:
            fh.write(data)
        prov.append({"file": out, "root": r["root"], "rel": r["rel"], "sha256": r["sha256"],
                     "compression": r.get("compression")})
    with open(os.path.join(dest, "PROVENANCE.json"), "x", encoding="utf-8") as fh:
        json.dump({"source": "recon/index sample, ordered by sha256", "eligible": len(recs),
                   "files": prov}, fh, indent=1)
    print("eligible %d, copied %d" % (len(recs), len(prov)))

main()
