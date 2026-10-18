"""Shared helpers for the Port Factory tools (stdlib only; run with python3 -I)."""
import json, os, datetime, subprocess

TOOLS = os.path.dirname(os.path.abspath(__file__))
PF_DIR = os.environ.get("PF_DIR") or os.path.dirname(TOOLS)
WORKTREE = os.path.dirname(os.path.dirname(TOOLS))
REA = os.environ.get("PF_REA", "/home/ryan/Projects/cybertown/.reports/xite-rust-full-port-rea-completion")
LEDGER = os.path.join(REA, "ledger", "ledger.json")
STATE = os.path.join(PF_DIR, "state.json")
BACKLOG = os.path.join(PF_DIR, "backlog.md")

STATUS_RANK = {"NOT_TESTED": 0, "PARTIAL": 1, "DIFFERENT": 2, "MISSING": 3, "COMPLETE": 9}
# component -> subsystem item that must exist first (blocked while that item is not COMPLETE)
DEPENDS = {
    "Sound": "sub:sound.engine",
    "Picking": "sub:picking.engine",
    "RigidBodyPhysics": "sub:physics.engine",
}

def now():
    return datetime.datetime.now().astimezone().isoformat(timespec="seconds")

def today():
    return datetime.date.today().isoformat()

def load_json(p, default=None):
    try:
        with open(p) as f:
            return json.load(f)
    except FileNotFoundError:
        return default

def write_atomic(p, text):
    tmp = p + ".tmp"
    with open(tmp, "w") as f:
        f.write(text)
    os.replace(tmp, p)

def save_state(s):
    s["updated"] = now()
    write_atomic(STATE, json.dumps(s, indent=1) + "\n")

def rebuild_ledger():
    """Rebuild ledger.json via the REA lane's build_ledger.py (python only, cheap)."""
    r = subprocess.run(["python3", "-I", os.path.join(REA, "tools", "build_ledger.py"), WORKTREE],
                       capture_output=True, text=True)
    return r.returncode, (r.stdout + r.stderr).strip()

def ranked_backlog(items, state):
    """Open items -> list of dicts with rank info. Order: ready before blocked, then
    status rank (NOT_TESTED < PARTIAL < DIFFERENT < MISSING), then small components first,
    then nodes before subsystems, then id."""
    by_id = {i["id"]: i for i in items}
    open_items = [i for i in items if i["status"] != "COMPLETE"]
    comp_size = {}
    for i in open_items:
        comp_size[i["component"]] = comp_size.get(i["component"], 0) + 1
    parked = {b["id"] for b in state.get("blockers", []) if not b.get("resolved")}
    out = []
    for i in open_items:
        reason = None
        dep = DEPENDS.get(i["component"])
        if i["kind"] == "node" and dep and dep in by_id and by_id[dep]["status"] != "COMPLETE":
            reason = "needs " + dep
        if i["id"] in parked:
            reason = "owner/blocker recorded"
        key = (reason is not None, STATUS_RANK.get(i["status"], 5), comp_size[i["component"]],
               0 if i["kind"] == "node" else 1, i["id"])
        out.append({"id": i["id"], "kind": i["kind"], "component": i["component"], "status": i["status"],
                    "missing": i.get("missing", ""), "blocked_by": reason, "component_open": comp_size[i["component"]], "_k": key})
    out.sort(key=lambda x: x["_k"])
    for n, x in enumerate(out, 1):
        x["priority"] = n
        del x["_k"]
    return out
