#!/usr/bin/env bash
# Identity gate self-test for the Rust port. Local only.
#
# It proves the gate accepts the approved candidate and refuses everything else.
# Every negative case uses disposable inputs. It does not modify the approved
# lock, the approved worktrees, or either reference checkout.
#
# Usage: tools/selftest-identity.sh
#
# Exit codes:
#   0  every case behaved as expected
#   1  at least one case failed
set -uo pipefail

HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd -- "$HERE/.." && pwd)
GATE="$HERE/verify-candidate.sh"
LOCK="$HERE/candidate.lock"
DIGEST="$HERE/reference-digest.py"

PASS=0
FAIL=0
TMPROOT=$(mktemp -d "${TMPDIR:-/tmp}/r01-selftest.XXXXXX")
trap 'rm -rf "$TMPROOT"' EXIT

ok ()   { PASS=$((PASS + 1)); printf '  PASS  %s\n' "$*"; }
bad ()  { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$*"; }

# expect <wanted exit> <label> <command...>
expect ()
{
   local want="$1" label="$2"; shift 2
   local out; out=$("$@" 2>&1); local got=$?
   if [ "$got" -eq "$want" ]; then ok "$label (exit $got)"
   else bad "$label (exit $got, wanted $want)"; printf '%s\n' "$out" | sed 's/^/          /'; fi
}

cd -- "$ROOT" || exit 1
echo "identity gate self-test"
echo "  worktree $ROOT"
echo "  scratch  $TMPROOT"
echo

# --- 1. the approved candidate must be accepted -------------------------------
echo "acceptance"
expect 0 "approved candidate is accepted" "$GATE"

# --- 2. wrong worktrees must be refused ---------------------------------------
echo
echo "wrong checkout"
expect 3 "private main is refused"        env -C /home/ryan/Projects/cybertown/X_ITE/scratch/rust-port "$GATE"
expect 3 "other research worktree refused" env -C /home/ryan/Projects/cybertown/.worktrees/rust-port-research-1 "$GATE"
expect 3 "detached checkout refused"       env -C /tmp/opencode/factor-lane "$GATE"

# --- 3. wrong reference pins must be refused ----------------------------------
# Disposable locks. The approved lock is only read, never written.
echo
echo "wrong reference pins (disposable locks)"
mk_lock () { sed "$1" "$LOCK" > "$TMPROOT/$2"; }

mk_lock 's/^X_ITE_PIN=.*/X_ITE_PIN=0000000000000000000000000000000000000000/' xite-pin.lock
expect 3 "wrong X_ITE commit pin refused" "$GATE" --lock "$TMPROOT/xite-pin.lock"

mk_lock 's/^CM5_PIN=.*/CM5_PIN=1111111111111111111111111111111111111111/' cm-pin.lock
expect 3 "wrong CodeMirror commit pin refused" "$GATE" --lock "$TMPROOT/cm-pin.lock"

mk_lock 's/^X_ITE_REF_DIGEST=.*/X_ITE_REF_DIGEST=deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef/' xite-digest.lock
expect 3 "wrong X_ITE content digest refused" "$GATE" --lock "$TMPROOT/xite-digest.lock"

mk_lock 's/^CM5_REF_DIGEST=.*/CM5_REF_DIGEST=cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe/' cm-digest.lock
expect 3 "wrong CodeMirror content digest refused" "$GATE" --lock "$TMPROOT/cm-digest.lock"

mk_lock 's/^X_ITE_VERSION=.*/X_ITE_VERSION=16.4.0/' xite-ver.lock
expect 3 "wrong X_ITE version refused" "$GATE" --lock "$TMPROOT/xite-ver.lock"

mk_lock 's/^CM5_VERSION=.*/CM5_VERSION=5.65.20/' cm-ver.lock
expect 3 "wrong CodeMirror version refused" "$GATE" --lock "$TMPROOT/cm-ver.lock"

# --- 4. the digest must react to content, not to file names alone --------------
echo
echo "reference digest sensitivity (disposable tree)"
mkdir -p "$TMPROOT/tree/sub"
printf 'a\n' > "$TMPROOT/tree/sub/a.js"
printf 'b\n' > "$TMPROOT/tree/sub/b.js"
D0=$(python3 "$DIGEST" "$TMPROOT/tree" --sub sub)
mv "$TMPROOT/tree/sub/a.js" "$TMPROOT/tree/sub/renamed.js"
D1=$(python3 "$DIGEST" "$TMPROOT/tree" --sub sub)
printf 'c\n' >> "$TMPROOT/tree/sub/b.js"
D2=$(python3 "$DIGEST" "$TMPROOT/tree" --sub sub)
echo 'new' > "$TMPROOT/tree/sub/c.js"
D3=$(python3 "$DIGEST" "$TMPROOT/tree" --sub sub)

if [ "$D0" != "$D1" ]; then ok "digest changes when a file is renamed"
else bad "digest did not change on rename"; fi
if [ "$D1" != "$D2" ]; then ok "digest changes when file bytes change"
else bad "digest did not change on a byte edit"; fi
if [ "${D3%% *}" = "3" ]; then ok "digest counts a newly added file"
else bad "digest did not count the added file ($D3)"; fi

# --- 5. a committed non-infrastructure change must be refused -----------------
echo
echo "committed non-infrastructure change"
REPO=$(git -C "$ROOT" rev-parse --git-common-dir)
BASE=$(git -C "$ROOT" rev-parse HEAD)
PROBE="$TMPROOT/wt"
if git -C "$REPO" worktree add -q --detach "$PROBE" "$BASE" 2>/dev/null; then
   printf '\n// unrelated probe\n' >> "$PROBE/crates/x3d-runtime/src/lib.rs"
   git -C "$PROBE" -c user.name=probe -c user.email=probe@example.invalid \
       commit -q -a -m "probe: unrelated runtime change"
   OUT=$(env -C "$PROBE" "$GATE" 2>&1); RC=$?
   if [ "$RC" -eq 3 ] && printf '%s' "$OUT" | grep -q 'crates/x3d-runtime/src/lib.rs'; then
      ok "committed crate change refused and named"
   else
      bad "committed crate change was not refused correctly (exit $RC)"
      printf '%s\n' "$OUT" | sed 's/^/          /'
   fi
   git -C "$REPO" worktree remove --force "$PROBE" >/dev/null 2>&1
else
   bad "could not create the probe worktree"
fi

# --- 6. a dirty tracked file must be refused ----------------------------------
echo
echo "dirty tracked file"
DIRTY_BEFORE=$(git -C "$ROOT" status --porcelain=v1 --untracked-files=no | wc -l)
if [ "$DIRTY_BEFORE" -eq 0 ]; then
   printf '\n' >> "$ROOT/PLAN.md"
   OUT=$("$GATE" 2>&1); RC=$?
   git -C "$ROOT" checkout -- PLAN.md
   if [ "$RC" -eq 3 ]; then ok "dirty tracked file refused and then restored"
   else bad "dirty tracked file was not refused (exit $RC)"; fi
else
   bad "tree was already dirty before the probe; skipped"
fi

echo
echo "self-test result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]