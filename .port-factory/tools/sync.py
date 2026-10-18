#!/usr/bin/env python3
"""sync.py [--no-rebuild]: rebuild REA ledger.json, regenerate backlog.md, merge into state.json.
Only this tool, next.py and record.py write state.json. Run with python3 -I."""
import sys, os, importlib.util
spec = importlib.util.spec_from_file_location("common", os.path.join(os.path.dirname(os.path.abspath(__file__)), "common.py"))
C = importlib.util.module_from_spec(spec); spec.loader.exec_module(C)

SCHEMA = {
 "name": "port-factory-state", "version": 1,
 "description": "Machine-readable state for the private X_ITE 16.5.0 -> Rust port factory. Plain JSON, no tool-specific format. "
   "Gap authority is the REA ledger (ledger_path); this file only tracks factory progress. Written only by .port-factory/tools/{sync,next,record}.py.",
 "fields": {
   "counts": "ledger status counts (COMPLETE, PARTIAL, MISSING, DIFFERENT, NOT_TESTED) and total; only COMPLETE counts toward coverage",
   "coverage_pct": "COMPLETE / total * 100",
   "active": "task currently being worked: {id, started, attempts, status_at_start, missing} or null",
   "completed": "[{id, date, source ('pre-factory' or 'factory'), evidence (paths), note}]",
   "test_results": "[{id, time, verdict PASS|FAIL|BLOCKED, kind focused|integration, command, evidence, note}] newest last",
   "blockers": "[{id, date, note, resolved}]; a BLOCKED oracle is not a pass",
   "owner_decisions": "[{date, decision}] recorded verbatim; never deleted",
   "backlog_top": "top 20 of the ranked open backlog (see backlog.md for the full list)"}}

def merge(items, st):
    st["schema"] = SCHEMA
    st["ledger_path"] = C.LEDGER
    st["worktree"] = C.WORKTREE
    for k, v in (("active", None), ("completed", []), ("test_results", []), ("blockers", []), ("owner_decisions", [])):
        st.setdefault(k, v)
    counts = {"total": len(items)}
    for i in items:
        counts[i["status"]] = counts.get(i["status"], 0) + 1
    st["counts"] = counts
    st["coverage_pct"] = round(100.0 * counts.get("COMPLETE", 0) / max(1, len(items)), 1)
    done = {c["id"] for c in st["completed"]}
    for i in items:
        if i["status"] == "COMPLETE" and i["id"] not in done:
            src = "factory" if st["active"] and st["active"]["id"] == i["id"] else "pre-factory"
            st["completed"].append({"id": i["id"], "date": C.today(), "source": src,
                "evidence": i.get("tests", []), "note": i.get("oracle", "")})
            if src == "factory":
                st["active"] = None
    bl = C.ranked_backlog(items, st)
    st["backlog_top"] = bl[:20]
    return bl

def backlog_md(bl, st):
    L = ["# Port Factory backlog", "",
         f"Generated {C.now()} from `{C.LEDGER}` by `.port-factory/tools/sync.py`. Do not hand-edit; change statuses with the REA `set_status.py`.", "",
         f"Coverage: {st['counts'].get('COMPLETE',0)}/{st['counts']['total']} COMPLETE = {st['coverage_pct']}% (PARTIAL does not count).", "",
         "## Ordering rule", "",
         "1. Ready before blocked (blocked = depends on a missing engine subsystem such as `sub:sound.engine`, or has a recorded blocker).",
         "2. Status: NOT_TESTED, then PARTIAL, then DIFFERENT, then MISSING (cheapest to finish first).",
         "3. Components with fewer open items first (small wins, fast integration).",
         "4. Nodes before subsystems, then id order.", "",
         f"Priority numbers below are global ranks. Active: {st['active']['id'] if st['active'] else 'none'}.", ""]
    comps = {}
    for x in bl:
        comps.setdefault(x["component"], []).append(x)
    for comp, xs in sorted(comps.items(), key=lambda kv: min(x["priority"] for x in kv[1])):
        L += [f"## {comp} ({len(xs)} open)", ""]
        for x in xs:
            b = f" **BLOCKED: {x['blocked_by']}**" if x["blocked_by"] else ""
            m = f" - {x['missing']}" if x["missing"] else ""
            L.append(f"- [ ] #{x['priority']} `{x['id']}` {x['status']}{b}{m}")
        L.append("")
    return "\n".join(L)

def main():
    if "--no-rebuild" not in sys.argv:
        rc, out = C.rebuild_ledger()
        print("build_ledger rc=%d %s" % (rc, out.splitlines()[-1] if out else ""))
    led = C.load_json(C.LEDGER)
    items = led["items"]
    st = C.load_json(C.STATE, {})
    bl = merge(items, st)
    C.write_atomic(C.BACKLOG, backlog_md(bl, st))
    C.save_state(st)
    print("synced: %s open=%d coverage=%s%%" % (st["counts"], len(bl), st["coverage_pct"]))

if __name__ == "__main__":
    main()
