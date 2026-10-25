#!/usr/bin/env bash
# Self-test of the reference and identity gate (tools/verify-candidate.sh).
#
# It proves that the gate accepts a clean checkout and refuses each kind of drift, and
# that each refusal names its own reason (a case passes only when the exit code AND the
# message match). Every case runs in a disposable sparse clone under $TMPDIR that this
# script creates and deletes. It never writes to the caller's checkout, its index, its
# worktree list, or the approved lock; it only reads the lock and the Git objects.
#
# Usage: tools/selftest-identity.sh
#
# Exit codes:
#   0  every case behaved as expected
#   1  at least one case failed
set -uo pipefail

HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
SRC=$(cd -- "$HERE/.." && pwd)
LOCK="$HERE/candidate.lock"
DIGEST="$HERE/reference-digest.py"

PASS=0
FAIL=0
TMPROOT=$(mktemp -d "${TMPDIR:-/tmp}/verify-candidate-selftest.XXXXXX")
trap 'rm -rf "$TMPROOT"' EXIT

ok ()   { PASS=$((PASS + 1)); printf '  PASS  %s\n' "$*"; }
bad ()  { FAIL=$((FAIL + 1)); printf '  FAIL  %s\n' "$*"; }

# expect <wanted exit> <message regex> <label> <command...>
# The case passes only when the exit code is right AND the output matches the regex,
# so a refusal for some other reason cannot pass a negative case.
expect ()
{
   local want="$1" re="$2" label="$3"; shift 3
   local out; out=$("$@" 2>&1); local got=$?
   if [ "$got" -eq "$want" ] && printf '%s' "$out" | grep -Eq -- "$re"; then
      ok "$label (exit $got)"
      printf '%s\n' "$out" | grep -E -m1 -- "$re" | sed 's/^/          /'
   else
      bad "$label (exit $got, wanted $want and /$re/)"
      printf '%s\n' "$out" | sed 's/^/          /'
   fi
}

# --- the disposable clone -----------------------------------------------------
# --shared reads the source objects through alternates and writes nothing to the source.
# The sparse checkout holds only what the gate reads, plus one crate file for a probe.
BASE=$(git -C "$SRC" rev-parse HEAD)
C="$TMPROOT/clone"
git clone -q --shared --no-checkout "$SRC" "$C" || { echo "cannot clone $SRC"; exit 1; }
git -C "$C" sparse-checkout set --no-cone /tools/ /oracle/x_ite-pin/ /oracle/cm5-build/ \
   /crates/x3d-runtime/src/lib.rs >/dev/null || { echo "cannot set sparse checkout"; exit 1; }
git -C "$C" checkout -q --detach "$BASE" || { echo "cannot check out $BASE"; exit 1; }
gitc () { git -C "$C" -c user.name=selftest -c user.email=selftest@example.invalid "$@"; }
# The gate under test is the one in the clone, at the same commit as the caller's.
GATE="$C/tools/verify-candidate.sh"
# A lock whose CM5_LIVE_DIR does not exist, so the result does not depend on the machine.
sed 's|^CM5_LIVE_DIR=.*|CM5_LIVE_DIR=/nonexistent/codemirror5|' "$LOCK" > "$TMPROOT/base.lock"
mk_lock () { sed "$1" "$TMPROOT/base.lock" > "$TMPROOT/$2"; }
g () { env -C "$C" "$GATE" --lock "$TMPROOT/base.lock" "$@"; }

echo "verify-candidate self-test"
echo "  source   $SRC at $BASE"
echo "  clone    $C"

# --- 1. acceptance ------------------------------------------------------------
echo
echo "acceptance"
expect 0 'not checked \(no --expect\)'  "reference-only run accepts the clean pins" g
expect 0 'identity ACCEPTED'          "--expect HEAD accepts a clean clone" g --expect "$BASE"
expect 0 'gated-command-ran'          "--expect HEAD -- <command> runs the command" g --expect "$BASE" -- echo gated-command-ran

# --- 2. usage refusals --------------------------------------------------------
echo
echo "usage"
expect 2 'full 40-character'          "--expect with a branch name is a usage error" g --expect master
expect 2 "not ''"                     "an empty --expect= is a usage error" g --expect=
expect 2 'needs --expect'             "a gated command without --expect is a usage error" g -- true
expect 2 'needs --expect'             "--allow-dirty without --expect is a usage error" g --allow-dirty
expect 2 'needs --expect'             "--expect-worktree without --expect is a usage error" g --expect-worktree "$C"

# --- 3. wrong reference pins (disposable locks) -------------------------------
echo
echo "wrong reference pins (disposable locks)"
mk_lock 's/^X_ITE_PIN=.*/X_ITE_PIN=0000000000000000000000000000000000000000/' xite-pin.lock
expect 3 'PINNED_COMMIT says'         "wrong X_ITE commit pin refused" env -C "$C" "$GATE" --lock "$TMPROOT/xite-pin.lock"
mk_lock 's/^X_ITE_VERSION=.*/X_ITE_VERSION=16.4.0/' xite-ver.lock
expect 3 'X_ITE version is'           "wrong X_ITE version refused" env -C "$C" "$GATE" --lock "$TMPROOT/xite-ver.lock"
mk_lock 's/^X_ITE_REF_DIGEST=.*/X_ITE_REF_DIGEST=deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef/' xite-digest.lock
expect 3 'X_ITE source digest is'     "wrong X_ITE content digest refused" env -C "$C" "$GATE" --lock "$TMPROOT/xite-digest.lock"
mk_lock 's/^CM5_VERSION=.*/CM5_VERSION=5.65.20/' cm-ver.lock
expect 3 'CodeMirror version is'      "wrong CodeMirror version refused" env -C "$C" "$GATE" --lock "$TMPROOT/cm-ver.lock"
mk_lock 's/^CM5_REF_DIGEST=.*/CM5_REF_DIGEST=cafebabecafebabecafebabecafebabecafebabecafebabecafebabecafebabe/' cm-digest.lock
expect 3 'CodeMirror source digest is' "wrong CodeMirror content digest refused" env -C "$C" "$GATE" --lock "$TMPROOT/cm-digest.lock"
# A disposable one-commit repository stands in for the live CodeMirror checkout.
git init -q "$TMPROOT/cm-live"
git -C "$TMPROOT/cm-live" -c user.name=selftest -c user.email=selftest@example.invalid commit -q --allow-empty -m live
mk_lock "s|^CM5_LIVE_DIR=.*|CM5_LIVE_DIR=$TMPROOT/cm-live|" cm-live.lock
expect 3 'CodeMirror checkout .* is at' "live CodeMirror checkout at the wrong commit refused" env -C "$C" "$GATE" --lock "$TMPROOT/cm-live.lock"

# --- 4. a one-byte edit in the reference tree ---------------------------------
echo
echo "reference content (disposable clone)"
XF=$(git -C "$C" ls-files oracle/x_ite-pin/src | grep -m1 '\.js$')
printf ' ' >> "$C/$XF"
expect 3 'X_ITE source digest is'     "one-byte edit in $XF refused" g
git -C "$C" restore --source=HEAD -- "$XF"
expect 0 'reference ACCEPTED'         "restored clone accepted again" g

# --- 5. the digest reacts to content and names --------------------------------
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
if [ "$D0" != "$D1" ]; then ok "digest changes when a file is renamed"; else bad "digest did not change on rename"; fi
if [ "$D1" != "$D2" ]; then ok "digest changes when file bytes change"; else bad "digest did not change on a byte edit"; fi
if [ "${D3%% *}" = "3" ]; then ok "digest counts a newly added file"; else bad "digest did not count the added file ($D3)"; fi

# --- 6. identity: commits in the disposable clone -----------------------------
echo
echo "identity (commits in the disposable clone)"
expect 3 'is not a commit'            "--expect of an absent commit refused" g --expect 0000000000000000000000000000000000000001

gitc commit -q --allow-empty -m "selftest: same tree"
expect 0 'same tree as'               "a commit with the same tree is accepted and named as such" g --expect "$BASE"

printf '\n# selftest infrastructure probe\n' >> "$C/tools/candidate.lock"
gitc commit -q -a -m "selftest: infrastructure only"
expect 0 'approved infrastructure only' "an infrastructure-only commit is accepted" g --expect "$BASE"

printf '\n// selftest probe\n' >> "$C/crates/x3d-runtime/src/lib.rs"
mkdir -p "$C/crates/x3d-runtime/src/sp ace"
printf '// probe\n' > "$C/crates/x3d-runtime/src/sp ace/a b.rs"
gitc add --sparse -A crates
gitc commit -q -m "selftest: crate change"
expect 3 'in 2 non-infrastructure file'  "a crate change is refused with every offender counted" g --expect "$BASE"
expect 3 'src/sp ace/a b.rs'          "an offender path with spaces is named whole" g --expect "$BASE"
expect 3 'src/lib.rs'                 "the second offender is listed too" g --expect "$BASE"

HEAD2=$(git -C "$C" rev-parse HEAD)
expect 3 'not the expected worktree'  "a wrong --expect-worktree is refused" g --expect "$HEAD2" --expect-worktree "$TMPROOT"
expect 0 'identity ACCEPTED'          "the right --expect-worktree is accepted" g --expect "$HEAD2" --expect-worktree "$C"

# --- 7. a dirty tracked file --------------------------------------------------
echo
echo "dirty tracked file (disposable clone)"
printf '\n' >> "$C/crates/x3d-runtime/src/lib.rs"
expect 3 'tracked file\(s\) are modified' "a modified tracked file is refused" g --expect "$HEAD2"
expect 3 'not the expected worktree'  "--allow-dirty does not waive the worktree check" g --expect "$HEAD2" --allow-dirty --expect-worktree "$TMPROOT"
expect 0 'NOT a clean candidate'      "--allow-dirty reports the tree as not clean" g --expect "$HEAD2" --allow-dirty

echo
echo "self-test result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
