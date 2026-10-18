#!/usr/bin/env python3
"""next.py [--force]: pick the first ready backlog item, mark it active in state.json, print it as JSON.
If a task is already active it is printed and kept (use --force to replace)."""
import sys, os, json, importlib.util
d = os.path.dirname(os.path.abspath(__file__))
def load(n):
    s = importlib.util.spec_from_file_location(n, os.path.join(d, n + ".py")); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
C = load("common")
st = C.load_json(C.STATE, {})
items = C.load_json(C.LEDGER)["items"]
if st.get("active") and "--force" not in sys.argv:
    print(json.dumps({"already_active": st["active"]}, indent=1)); sys.exit(0)
ready = [x for x in C.ranked_backlog(items, st) if not x["blocked_by"]]
if not ready:
    print(json.dumps({"none_ready": True})); sys.exit(2)
t = ready[0]
full = next(i for i in items if i["id"] == t["id"])
st["active"] = {"id": t["id"], "started": C.now(), "attempts": 0, "status_at_start": t["status"],
                "missing": full.get("missing", ""), "rust": full.get("rust", []), "upstream": full.get("upstream", ""),
                "component": t["component"]}
C.save_state(st)
print(json.dumps({"selected": st["active"], "priority": t["priority"]}, indent=1))
