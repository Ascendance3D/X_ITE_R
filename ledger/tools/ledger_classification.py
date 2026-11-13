"""The classification rules of the ledger (owner decision 2026-11-12), used by verify_ledger.py and the ledger step tools.

The optional `classification` object of an entry is independent of its `status`. EVIDENCE_FIELDS are the fields that a step may
change on a DIFFERENT entry it names in its evidence set; the status and the other fields stay."""
import re

# The classification (owner decision 2026-11-12). It does not change a status or the totals.
CLASSES = {"ISO_COMPLIANT", "DOCUMENTED_DIFFERENCE", "UPSTREAM_DEFECT_CORRECTED", "IMPLEMENTATION_GAP", "UNVERIFIED"}
ISO_STATES = {"VERIFIED", "UNVERIFIED", "NOT_APPLICABLE"}
CLASS_KEYS = {"class", "iso", "decision", "upstream_defects", "impact", "reviewed"}
CLAUSE = re.compile(r"^\d{4,5}(-\d+)?:\d{4} [A-Z]?\d+(\.\d+)*$")  # for example "19775-1:2023 19.2.4"
# The fields of a DIFFERENT entry that a step may change when it names the entry in its evidence set.
EVIDENCE_FIELDS = {"rust", "tests", "oracle", "notes", "classification"}
def text(v): return isinstance(v, str) and v.strip() != ""

def classification_errors(i):
    """The rules of the classification object of entry i (an absent object is no error)."""
    if "classification" not in i: return []
    c, k, bad = i["classification"], i["id"], []
    if not isinstance(c, dict): return [f"{k}: classification is not an object"]
    bad += [f"{k}: classification key {x!r} unknown" for x in sorted(set(c) - CLASS_KEYS)]
    cls = c.get("class")
    if cls not in CLASSES: bad.append(f"{k}: classification class {cls!r} not allowed")
    if not (isinstance(c.get("reviewed"), str) and re.fullmatch(r"\d{4}-\d{2}-\d{2}", c["reviewed"])): bad.append(f"{k}: classification needs reviewed YYYY-MM-DD")
    iso = c.get("iso")
    if iso is not None:
        if not isinstance(iso, dict) or iso.get("state") not in ISO_STATES: bad.append(f"{k}: classification iso.state not one of {sorted(ISO_STATES)}")
        elif iso["state"] == "VERIFIED":
            clauses = iso.get("clauses")
            if not (isinstance(clauses, list) and clauses and all(isinstance(x, str) and CLAUSE.match(x) for x in clauses)): bad.append(f"{k}: iso VERIFIED needs clauses that were read (for example \"19775-1:2023 19.2.4\")")
            if not text(iso.get("source")): bad.append(f"{k}: iso VERIFIED needs its source")
    d = c.get("decision")
    if d is not None and not (isinstance(d, dict) and text(d.get("ref"))): bad.append(f"{k}: classification decision needs ref")
    defects = c.get("upstream_defects")
    if defects is not None and not (isinstance(defects, list) and all(isinstance(x, dict) and all(text(x.get(f)) for f in ("summary", "evidence")) for x in defects)): bad.append(f"{k}: every upstream_defects item needs summary and evidence")
    if cls == "ISO_COMPLIANT":
        if not (isinstance(iso, dict) and iso.get("state") == "VERIFIED"): bad.append(f"{k}: ISO_COMPLIANT needs iso.state VERIFIED")
        if not i.get("tests"): bad.append(f"{k}: ISO_COMPLIANT needs tests")
    if cls == "DOCUMENTED_DIFFERENCE" and not (isinstance(d, dict) and text(d.get("ref")) and text(c.get("impact"))): bad.append(f"{k}: DOCUMENTED_DIFFERENCE needs decision.ref and impact")
    if cls == "UPSTREAM_DEFECT_CORRECTED" and not (isinstance(defects, list) and defects and all(isinstance(x, dict) and text(x.get("rust_test")) for x in defects)): bad.append(f"{k}: UPSTREAM_DEFECT_CORRECTED needs upstream_defects with rust_test")
    if cls == "IMPLEMENTATION_GAP" and not text(i.get("missing")): bad.append(f"{k}: IMPLEMENTATION_GAP needs missing text")
    return bad

def frozen(i): return {f: v for f, v in i.items() if f not in EVIDENCE_FIELDS}

