#!/usr/bin/env bash
# Fail-closed identity gate for the Rust port. Local only. Nothing here publishes or pushes.
#
# It answers one question before any test runs: "is this the approved candidate?"
# If the answer is no, it refuses. It never switches worktrees and never picks a branch
# for the caller. It also never changes the working tree.
#
# Usage:
#   tools/verify-candidate.sh                       # report identity only, exit 0/3
#   tools/verify-candidate.sh --expect <sha>        # gate on an explicit commit instead of the lock
#   tools/verify-candidate.sh --allow-dirty         # report dirty files but do not refuse
#   tools/verify-candidate.sh -- <command> [args..] # gate, then run <command> in this worktree
#
# Exit codes:
#   0  identity accepted
#   2  usage error
#   3  REFUSED. The commit, branch, or worktree is not the approved candidate.
#      A test runner that sees 3 must stop and must not retry elsewhere.
set -uo pipefail

HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# The worktree under test is the CALLER's current directory, not the directory
# that happens to hold this script. Using the script location would make the gate
# always accept, because it would always inspect the approved worktree.
ROOT=$(cd -- "${PWD}" 2>/dev/null && pwd -P)
# The lock ships next to the script so one approved pin serves every worktree.
LOCK="$HERE/candidate.lock"
DIGEST="$HERE/reference-digest.py"

die_usage () { echo "verify-candidate: $*" >&2; exit 2; }
refuse () { echo "verify-candidate: REFUSED: $*" >&2; exit 3; }

# --- read the arguments ------------------------------------------------------
# Arguments are parsed BEFORE the lock is sourced, because --lock chooses which
# lock file to read. An earlier draft sourced first, so --lock was silently
# ignored and every negative test appeared to pass.
EXPECT_COMMIT=""
ALLOW_DIRTY=0
SKIP_REFERENCE=0
RUN=()

while [ $# -gt 0 ]; do
   case "$1" in
      --expect) [ $# -ge 2 ] || die_usage "--expect needs a commit"; EXPECT_COMMIT="$2"; shift 2 ;;
      --expect=*) EXPECT_COMMIT="${1#--expect=}"; shift ;;
      --root) [ $# -ge 2 ] || die_usage "--root needs a directory"; ROOT="$(cd -- "$2" && pwd -P)"; shift 2 ;;
      --allow-dirty) ALLOW_DIRTY=1; shift ;;
      --lock) [ $# -ge 2 ] || die_usage "--lock needs a file"; LOCK="$2"; shift 2 ;;
      --no-reference-check) SKIP_REFERENCE=1; shift ;;
      --) shift; RUN=("$@"); break ;;
      -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
      *) die_usage "unknown argument: $1" ;;
   esac
done

# --- read the lock -----------------------------------------------------------
[ -f "$LOCK" ] || die_usage "missing pin file: $LOCK"
# shellcheck disable=SC1090
. "$LOCK"
: "${CANDIDATE_COMMIT:?CANDIDATE_COMMIT missing from lock}"
: "${CANDIDATE_BRANCH:?CANDIDATE_BRANCH missing from lock}"
: "${CANDIDATE_WORKTREE:?CANDIDATE_WORKTREE missing from lock}"
: "${X_ITE_PIN:?X_ITE_PIN missing from lock}"
: "${X_ITE_VERSION:?X_ITE_VERSION missing from lock}"
: "${X_ITE_REF_DIR:?X_ITE_REF_DIR missing from lock}"
: "${X_ITE_REF_DIGEST:?X_ITE_REF_DIGEST missing from lock}"
: "${CM5_PIN:?CM5_PIN missing from lock}"
: "${CM5_VERSION:?CM5_VERSION missing from lock}"
: "${CM5_REF_DIR:?CM5_REF_DIR missing from lock}"
: "${CM5_REF_DIGEST:?CM5_REF_DIGEST missing from lock}"
CM5_SUBS="${CM5_SUBS:-}"
ALLOWED_INFRA_FILES="${ALLOWED_INFRA_FILES:-}"

[ -n "$EXPECT_COMMIT" ] || EXPECT_COMMIT="$CANDIDATE_COMMIT"

# --- gather the facts --------------------------------------------------------
command -v git >/dev/null || die_usage "git not found"
git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1 || die_usage "$ROOT is not a git checkout"

HEAD_SHA=$(git -C "$ROOT" rev-parse HEAD 2>/dev/null)
BRANCH=$(git -C "$ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null)
WORKTREE=$(cd -- "$ROOT" && pwd -P)
GIT_COMMON=$(cd -- "$(git -C "$ROOT" rev-parse --git-common-dir)" 2>/dev/null && pwd -P || echo unknown)

# Tracked modifications only. Untracked build output and gitignored baselines are artifacts.
DIRTY_TRACKED=$(git -C "$ROOT" status --porcelain=v1 --untracked-files=no 2>/dev/null)
DIRTY_LIST=$(git -C "$ROOT" status --porcelain=v1 2>/dev/null | sed -n 's/^?? //p')
DIRTY_COUNT=$(printf '%s' "$DIRTY_TRACKED" | grep -c . )

# --- the report --------------------------------------------------------------
echo "candidate identity"
echo "  worktree           $WORKTREE"
echo "  git common dir     $GIT_COMMON"
echo "  branch             $BRANCH"
echo "  HEAD               $HEAD_SHA"
echo "  expected commit    $EXPECT_COMMIT"
echo "  dirty tracked files ${DIRTY_COUNT}"

if [ "$DIRTY_COUNT" -gt 0 ]; then
   printf '%s\n' "$DIRTY_TRACKED" | sed 's/^/      /'
fi

UNTRACKED_COUNT=$(printf '%s' "$DIRTY_LIST" | grep -c . )
echo "  untracked entries  $UNTRACKED_COUNT (build output and gitignored baselines)"

# --- reference sources, checked by content -----------------------------------
# A stored commit string proves only what someone wrote down. These checks read
# the actual bytes of the reference trees and refuse when they drift.
REF_PROBLEMS=()

# digest <dir> <subs...> -> "<file_count> <sha256>" or empty when it cannot run
digest ()
{
   local dir="$1"; shift
   local args=()
   for s in "$@"; do args+=(--sub "$s"); done
   python3 "$DIGEST" "$dir" "${args[@]}" 2>/dev/null
}

if [ "$SKIP_REFERENCE" -eq 0 ]; then
   # -- X_ITE -------------------------------------------------------------
   XI_DIR="$ROOT/$X_ITE_REF_DIR"
   if [ ! -d "$XI_DIR" ]; then
      REF_PROBLEMS+=("X_ITE reference tree is missing: $X_ITE_REF_DIR")
   else
      if [ -f "$ROOT/$X_ITE_PIN_FILE" ]; then
         XI_FILE_PIN=$(tr -d '[:space:]' < "$ROOT/$X_ITE_PIN_FILE")
         # PINNED_COMMIT stores the 9-character prefix that upstream's docs use.
         [ "$XI_FILE_PIN" = "${X_ITE_PIN:0:${#XI_FILE_PIN}}" ] || \
            REF_PROBLEMS+=("X_ITE PINNED_COMMIT says $XI_FILE_PIN, lock says ${X_ITE_PIN:0:8}")
      fi
      if [ -f "$ROOT/$X_ITE_VERSION_FILE" ]; then
         XI_VER=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("version",""))' "$ROOT/$X_ITE_VERSION_FILE" 2>/dev/null)
         [ "$XI_VER" = "$X_ITE_VERSION" ] || \
            REF_PROBLEMS+=("X_ITE version is $XI_VER, lock says $X_ITE_VERSION")
      fi
      XI_GOT=$(digest "$XI_DIR")
      XI_WANT="${X_ITE_REF_FILES} ${X_ITE_REF_DIGEST}"
      [ "$XI_GOT" = "$XI_WANT" ] || \
         REF_PROBLEMS+=("X_ITE source digest is '$XI_GOT', lock says '$XI_WANT'")
   fi

   # -- CodeMirror 5 ------------------------------------------------------
   CM_DIR="$ROOT/$CM5_REF_DIR"
   if [ ! -d "$CM_DIR" ]; then
      REF_PROBLEMS+=("CodeMirror reference tree is missing: $CM5_REF_DIR")
   else
      if [ -f "$ROOT/$CM5_VERSION_FILE" ]; then
         CM_VER=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("version",""))' "$ROOT/$CM5_VERSION_FILE" 2>/dev/null)
         [ "$CM_VER" = "$CM5_VERSION" ] || \
            REF_PROBLEMS+=("CodeMirror version is $CM_VER, lock says $CM5_VERSION")
      fi
      # shellcheck disable=SC2086
      CM_GOT=$(digest "$CM_DIR" $CM5_SUBS)
      CM_WANT="${CM5_REF_FILES} ${CM5_REF_DIGEST}"
      [ "$CM_GOT" = "$CM_WANT" ] || \
         REF_PROBLEMS+=("CodeMirror source digest is '$CM_GOT', lock says '$CM_WANT'")
      if [ -d "$CM5_LIVE_DIR/.git" ]; then
         CM_LIVE_SHA=$(git -C "$CM5_LIVE_DIR" rev-parse HEAD 2>/dev/null)
         [ "$CM_LIVE_SHA" = "$CM5_PIN" ] || \
            REF_PROBLEMS+=("CodeMirror checkout is at ${CM_LIVE_SHA:0:8}, lock says ${CM5_PIN:0:8}")
      fi
   fi
fi

echo "  reference X_ITE    $X_ITE_PIN (version $X_ITE_VERSION)"
echo "  reference CodeMirror $CM5_PIN (version $CM5_VERSION)"

if [ "${#REF_PROBLEMS[@]}" -gt 0 ]; then
   for p in "${REF_PROBLEMS[@]}"; do echo "  reference problem   $p"; done
fi

# --- the gate ----------------------------------------------------------------
# The branch check is a warning, not a refusal. A detached HEAD at the right
# commit is the same candidate. A different branch at the same commit is fine.
# The commit is the identity that matters.
if [ "$HEAD_SHA" != "$EXPECT_COMMIT" ]; then
   # A commit on top of the candidate is acceptable only when its whole diff is
   # approved infrastructure. Anything that touches a crate refuses.
   DELTA=$(git -C "$ROOT" diff --name-only "$EXPECT_COMMIT" "$HEAD_SHA" 2>/dev/null)
   if [ -z "$DELTA" ]; then
      refuse "HEAD $HEAD_SHA and expected $EXPECT_COMMIT are unrelated commits"
   fi
   OFFENDER=""
   for f in $DELTA; do
      case " $ALLOWED_INFRA_FILES " in
         *" $f "*) ;;
         *) OFFENDER="$f"; break ;;
      esac
   done
   if [ -n "$OFFENDER" ]; then
      refuse "HEAD $HEAD_SHA differs from $EXPECT_COMMIT in non-infrastructure file: $OFFENDER"
   fi
   echo "  note               HEAD is $EXPECT_COMMIT plus approved infrastructure only"
fi

if [ "$BRANCH" != "$CANDIDATE_BRANCH" ] && [ "$BRANCH" != "HEAD" ]; then
   echo "  note               branch is $BRANCH, lock names $CANDIDATE_BRANCH" >&2
fi

if [ "$WORKTREE" != "$CANDIDATE_WORKTREE" ]; then
   # Running the right commit from another worktree is only refused on request,
   # because the approved path is the one QA is told to reproduce.
   if [ "$ALLOW_DIRTY" -eq 0 ]; then
      refuse "worktree $WORKTREE is not the approved path $CANDIDATE_WORKTREE"
   fi
   echo "  note               running outside the approved worktree (allowed)" >&2
fi

if [ "$DIRTY_COUNT" -gt 0 ] && [ "$ALLOW_DIRTY" -eq 0 ]; then
   refuse "$DIRTY_COUNT tracked file(s) are modified; the candidate must be clean"
fi

# Reference drift refuses even when the working tree is clean, because a test run
# against the wrong reference source proves nothing.
if [ "${#REF_PROBLEMS[@]}" -gt 0 ]; then
   refuse "reference source does not match the pin: ${REF_PROBLEMS[*]}"
fi

echo "  result             ACCEPTED"

# --- optionally run the gated command ----------------------------------------
if [ "${#RUN[@]}" -gt 0 ]; then
   echo "  running            ${RUN[*]}"
   cd -- "$ROOT" || refuse "cannot enter $ROOT"
   exec "${RUN[@]}"
fi
exit 0