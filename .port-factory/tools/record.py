#!/usr/bin/env python3
"""record.py <id> <PASS|FAIL|BLOCKED|UNBLOCK> "<note>" [--kind focused|integration] [--command C] [--evidence PATH]
PASS also rebuilds the ledger; the task moves to `completed` only if the ledger now says COMPLETE
(status changes go through the REA set_status.py first). BLOCKED is never a pass."""
import sys, os, importlib.util
d = os.path.dirname(os.path.abspath(__file__))
def load(n):
    s = importlib.util.spec_from_file_location(n, os.path.join(d, n + ".py")); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
C = load("common"); S = load("sync")
a = sys.argv[1:]
if len(a) < 3 or a[1] not in ("PASS", "FAIL", "BLOCKED", "UNBLOCK"):
    sys.exit(__doc__)
tid, verdict, note = a[0], a[1], a[2]
opt = lambda n, dflt="": a[a.index(n) + 1] if n in a else dflt
st = C.load_json(C.STATE, {})
if verdict != "UNBLOCK":
    st.setdefault("test_results", []).append({"id": tid, "time": C.now(), "verdict": verdict,
        "kind": opt("--kind", "focused"), "command": opt("--command"), "evidence": opt("--evidence"), "note": note})
act = st.get("active")
if verdict == "FAIL" and act and act["id"] == tid:
    act["attempts"] += 1
elif verdict == "BLOCKED":
    st.setdefault("blockers", []).append({"id": tid, "date": C.today(), "note": note, "resolved": False})
    if act and act["id"] == tid:
        st["active"] = None
elif verdict == "UNBLOCK":
    for b in st.get("blockers", []):
        if b["id"] == tid and not b.get("resolved"):
            b["resolved"] = True; b["resolved_note"] = note
C.save_state(st)
if verdict == "PASS":
    rc, out = C.rebuild_ledger()
    items = C.load_json(C.LEDGER)["items"]
    st = C.load_json(C.STATE)
    it = next((i for i in items if i["id"] == tid), None)
    if it and it["status"] == "COMPLETE":
        if st.get("active") and st["active"]["id"] == tid:
            pass  # merge() marks source=factory and clears active
        for c in st.get("completed", []):
            if c["id"] == tid: c["evidence"] = [opt("--evidence")] if opt("--evidence") else c["evidence"]
    else:
        print("note: PASS recorded but ledger status is %s, not COMPLETE; task stays active/open" % (it and it["status"]))
    bl = S.merge(items, st)
    for c in st["completed"]:
        if c["id"] == tid and c["source"] == "factory" and opt("--evidence"):
            c["evidence"] = [opt("--evidence")]; c["note"] = note
    C.write_atomic(C.BACKLOG, S.backlog_md(bl, st))
    C.save_state(st)
print("recorded", tid, verdict)
