#!/usr/bin/env bash
# Fail-closed reference and identity gate for the Rust port. Nothing here publishes or pushes.
#
# The gate has two parts:
#
#   reference  Always on. The vendored X_ITE and CodeMirror sources must match the pins in
#              tools/candidate.lock by version string and by content digest. When the live
#              CodeMirror checkout (CM5_LIVE_DIR) exists, its HEAD must equal CM5_PIN.
#              CI runs this part on every pull request.
#   identity   Only on request, with --expect <full sha>. HEAD must be that commit, or that
#              commit plus ALLOWED_INFRA_FILES only, and no tracked file may be modified.
#              There is no default identity: no commit is an approved candidate unless the
#              caller names it. Master is not an approved release candidate.
#
# It never switches worktrees, never picks a branch for the caller, and never changes the
# working tree.
#
# Usage:
#   tools/verify-candidate.sh                              # reference checks only, exit 0/3
#   tools/verify-candidate.sh --expect <sha>               # reference + identity checks
#   tools/verify-candidate.sh --expect <sha> --expect-worktree <dir>
#                                                          # also require this worktree path
#   tools/verify-candidate.sh --expect <sha> --allow-dirty # report modified files, do not refuse
#   tools/verify-candidate.sh --expect <sha> -- <command> [args..]
#                                                          # gate, then run <command> in the worktree
#   Other options: --root <dir> (the worktree to check, default: the current directory),
#                  --lock <file> (default: tools/candidate.lock next to this script).
#
# Exit codes:
#   0  accepted
#   2  usage error
#   3  REFUSED. A test runner that sees 3 must stop and must not retry elsewhere.
set -uo pipefail

HERE=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# The worktree under test is the CALLER's current directory, not the directory
# that holds this script. Using the script location would make the gate always
# inspect one fixed worktree.
ROOT=$(cd -- "${PWD}" 2>/dev/null && pwd -P)
# The lock ships next to the script so one pin serves every worktree.
LOCK="$HERE/candidate.lock"
DIGEST="$HERE/reference-digest.py"

die_usage () { echo "verify-candidate: $*" >&2; exit 2; }
refuse () { echo "verify-candidate: REFUSED: $*" >&2; exit 3; }

# --- read the arguments ------------------------------------------------------
# Arguments are parsed BEFORE the lock is sourced, because --lock chooses which
# lock file to read.
EXPECT_COMMIT=""
EXPECT_GIVEN=0
EXPECT_WORKTREE=""
ALLOW_DIRTY=0
RUN=()
HAVE_RUN=0

while [ $# -gt 0 ]; do
   case "$1" in
      --expect) [ $# -ge 2 ] || die_usage "--expect needs a commit"; EXPECT_COMMIT="$2"; EXPECT_GIVEN=1; shift 2 ;;
      --expect=*) EXPECT_COMMIT="${1#--expect=}"; EXPECT_GIVEN=1; shift ;;
      --expect-worktree) [ $# -ge 2 ] || die_usage "--expect-worktree needs a directory"; EXPECT_WORKTREE="$2"; shift 2 ;;
      --root) [ $# -ge 2 ] || die_usage "--root needs a directory"; ROOT="$(cd -- "$2" && pwd -P)" || die_usage "no such directory: $2"; shift 2 ;;
      --allow-dirty) ALLOW_DIRTY=1; shift ;;
      --lock) [ $# -ge 2 ] || die_usage "--lock needs a file"; LOCK="$2"; shift 2 ;;
      --) shift; RUN=("$@"); HAVE_RUN=1; break ;;
      -h|--help) sed -n '2,32p' "${BASH_SOURCE[0]}"; exit 0 ;;
      *) die_usage "unknown argument: $1" ;;
   esac
done

# An empty --expect= is an error, not a silent reference-only run.
if [ "$EXPECT_GIVEN" -eq 1 ] && ! [[ "$EXPECT_COMMIT" =~ ^[0-9a-f]{40}$ ]]; then
   die_usage "--expect needs a full 40-character commit SHA, not '$EXPECT_COMMIT'"
fi
if [ -z "$EXPECT_COMMIT" ]; then
   [ "$HAVE_RUN" -eq 0 ] || die_usage "a gated command (--) needs --expect <sha>"
   [ -z "$EXPECT_WORKTREE" ] || die_usage "--expect-worktree needs --expect <sha>"
   [ "$ALLOW_DIRTY" -eq 0 ] || die_usage "--allow-dirty needs --expect <sha>"
fi

# --- read the lock -----------------------------------------------------------
[ -f "$LOCK" ] || die_usage "missing pin file: $LOCK"
# shellcheck disable=SC1090
. "$LOCK"
: "${X_ITE_PIN:?X_ITE_PIN missing from lock}"
: "${X_ITE_VERSION:?X_ITE_VERSION missing from lock}"
: "${X_ITE_REF_DIR:?X_ITE_REF_DIR missing from lock}"
: "${X_ITE_REF_DIGEST:?X_ITE_REF_DIGEST missing from lock}"
: "${CM5_PIN:?CM5_PIN missing from lock}"
: "${CM5_VERSION:?CM5_VERSION missing from lock}"
: "${CM5_REF_DIR:?CM5_REF_DIR missing from lock}"
: "${CM5_REF_DIGEST:?CM5_REF_DIGEST missing from lock}"
CM5_SUBS="${CM5_SUBS:-}"
CM5_LIVE_DIR="${CM5_LIVE_DIR:-}"
ALLOWED_INFRA_FILES="${ALLOWED_INFRA_FILES:-}"

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

json_version () { python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("version",""))' "$1" 2>/dev/null; }

# -- X_ITE ---------------------------------------------------------------
XI_DIR="$ROOT/$X_ITE_REF_DIR"
if [ ! -d "$XI_DIR" ]; then
   REF_PROBLEMS+=("X_ITE reference tree is missing: $X_ITE_REF_DIR")
else
   if [ -f "$ROOT/$X_ITE_PIN_FILE" ]; then
      XI_FILE_PIN=$(tr -d '[:space:]' < "$ROOT/$X_ITE_PIN_FILE")
      # PINNED_COMMIT stores the 9-character prefix that upstream's docs use.
      [ -n "$XI_FILE_PIN" ] && [ "$XI_FILE_PIN" = "${X_ITE_PIN:0:${#XI_FILE_PIN}}" ] || \
         REF_PROBLEMS+=("X_ITE PINNED_COMMIT says '$XI_FILE_PIN', lock says ${X_ITE_PIN:0:9}")
   else
      REF_PROBLEMS+=("X_ITE pin file is missing: $X_ITE_PIN_FILE")
   fi
   if [ -f "$ROOT/$X_ITE_VERSION_FILE" ]; then
      XI_VER=$(json_version "$ROOT/$X_ITE_VERSION_FILE")
      [ "$XI_VER" = "$X_ITE_VERSION" ] || \
         REF_PROBLEMS+=("X_ITE version is '$XI_VER', lock says $X_ITE_VERSION")
   else
      REF_PROBLEMS+=("X_ITE version file is missing: $X_ITE_VERSION_FILE")
   fi
   XI_GOT=$(digest "$XI_DIR")
   XI_WANT="${X_ITE_REF_FILES} ${X_ITE_REF_DIGEST}"
   [ "$XI_GOT" = "$XI_WANT" ] || \
      REF_PROBLEMS+=("X_ITE source digest is '$XI_GOT', lock says '$XI_WANT'")
fi

# -- CodeMirror 5 --------------------------------------------------------
CM_DIR="$ROOT/$CM5_REF_DIR"
if [ ! -d "$CM_DIR" ]; then
   REF_PROBLEMS+=("CodeMirror reference tree is missing: $CM5_REF_DIR")
else
   if [ -f "$ROOT/$CM5_VERSION_FILE" ]; then
      CM_VER=$(json_version "$ROOT/$CM5_VERSION_FILE")
      [ "$CM_VER" = "$CM5_VERSION" ] || \
         REF_PROBLEMS+=("CodeMirror version is '$CM_VER', lock says $CM5_VERSION")
   else
      REF_PROBLEMS+=("CodeMirror version file is missing: $CM5_VERSION_FILE")
   fi
   # shellcheck disable=SC2086
   CM_GOT=$(digest "$CM_DIR" $CM5_SUBS)
   CM_WANT="${CM5_REF_FILES} ${CM5_REF_DIGEST}"
   [ "$CM_GOT" = "$CM_WANT" ] || \
      REF_PROBLEMS+=("CodeMirror source digest is '$CM_GOT', lock says '$CM_WANT'")
fi
# The live checkout exists only on the owner's machine (not in CI), so this check is
# conditional. Its lock path resolves through X_ITE/scratch/rust-port (a symlink to X_ITE_R)
# into the ignored upstream/ directory of the main checkout.
CM_LIVE="not present (not checked)"
if [ -n "$CM5_LIVE_DIR" ] && [ -e "$CM5_LIVE_DIR/.git" ]; then
   CM_LIVE_SHA=$(git -C "$CM5_LIVE_DIR" rev-parse HEAD 2>/dev/null)
   CM_LIVE="$CM_LIVE_SHA"
   [ "$CM_LIVE_SHA" = "$CM5_PIN" ] || \
      REF_PROBLEMS+=("CodeMirror checkout $CM5_LIVE_DIR is at '${CM_LIVE_SHA:0:8}', lock says ${CM5_PIN:0:8}")
fi

echo "reference"
echo "  root               $ROOT"
echo "  X_ITE              $X_ITE_PIN (version $X_ITE_VERSION, ${X_ITE_REF_FILES} files)"
echo "  CodeMirror         $CM5_PIN (version $CM5_VERSION, ${CM5_REF_FILES} files)"
echo "  CodeMirror live    $CM_LIVE"
if [ "${#REF_PROBLEMS[@]}" -gt 0 ]; then
   for p in "${REF_PROBLEMS[@]}"; do echo "  reference problem  $p"; done
   # Reference drift refuses before any identity check, because a test run against
   # the wrong reference source proves nothing.
   refuse "reference source does not match the pin: ${REF_PROBLEMS[*]}"
fi
echo "  result             reference ACCEPTED"

if [ -z "$EXPECT_COMMIT" ]; then
   echo "identity"
   echo "  result             not checked (no --expect); this is not a candidate approval"
   exit 0
fi

# --- identity ----------------------------------------------------------------
command -v git >/dev/null || die_usage "git not found"
git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1 || die_usage "$ROOT is not a git checkout"
git -C "$ROOT" cat-file -e "$EXPECT_COMMIT^{commit}" 2>/dev/null || \
   refuse "expected $EXPECT_COMMIT is not a commit in this repository"

HEAD_SHA=$(git -C "$ROOT" rev-parse --verify HEAD 2>/dev/null) || refuse "cannot read HEAD in $ROOT"
BRANCH=$(git -C "$ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null)
WORKTREE=$(cd -- "$ROOT" && pwd -P)
# Tracked modifications only. Untracked build output and gitignored baselines are artifacts.
DIRTY_TRACKED=$(git -C "$ROOT" status --porcelain=v1 --untracked-files=no 2>/dev/null)
DIRTY_COUNT=$(printf '%s' "$DIRTY_TRACKED" | grep -c . )

echo "identity"
echo "  worktree           $WORKTREE"
echo "  branch             $BRANCH"
echo "  HEAD               $HEAD_SHA"
echo "  expected commit    $EXPECT_COMMIT"
echo "  dirty tracked files $DIRTY_COUNT"
[ "$DIRTY_COUNT" -eq 0 ] || printf '%s\n' "$DIRTY_TRACKED" | sed 's/^/      /'

if [ "$HEAD_SHA" != "$EXPECT_COMMIT" ]; then
   # A commit on top of the expected one is accepted only when its whole diff is
   # approved infrastructure. Anything that touches a crate refuses.
   # -z keeps a path with spaces or newlines as one name.
   OFFENDERS=()
   DELTA_COUNT=0
   # A failed diff must refuse, never read as an empty (accepted) delta.
   DELTA_FILE=$(mktemp) || refuse "cannot create a temporary file"
   if ! git -C "$ROOT" diff -z --name-only "$EXPECT_COMMIT" "$HEAD_SHA" > "$DELTA_FILE"; then
      rm -f "$DELTA_FILE"
      refuse "git diff $EXPECT_COMMIT $HEAD_SHA failed"
   fi
   while IFS= read -r -d '' f; do
      DELTA_COUNT=$((DELTA_COUNT + 1))
      case " $ALLOWED_INFRA_FILES " in
         *" $f "*) ;;
         *) OFFENDERS+=("$f") ;;
      esac
   done < "$DELTA_FILE"
   rm -f "$DELTA_FILE"
   if [ "$DELTA_COUNT" -eq 0 ]; then
      # Same tree, different commit (for example an empty commit): the content is identical.
      # Prove it from the tree IDs, not only from the empty diff.
      [ "$(git -C "$ROOT" rev-parse "$HEAD_SHA^{tree}")" = "$(git -C "$ROOT" rev-parse "$EXPECT_COMMIT^{tree}")" ] || \
         refuse "empty diff, but HEAD and $EXPECT_COMMIT have different trees"
      echo "  note               HEAD has the same tree as $EXPECT_COMMIT"
   elif [ "${#OFFENDERS[@]}" -gt 0 ]; then
      echo "  non-infrastructure ${#OFFENDERS[@]} of $DELTA_COUNT changed file(s):"
      printf '      %s\n' "${OFFENDERS[@]:0:50}"
      [ "${#OFFENDERS[@]}" -le 50 ] || echo "      ... and $(( ${#OFFENDERS[@]} - 50 )) more"
      refuse "HEAD $HEAD_SHA differs from $EXPECT_COMMIT in ${#OFFENDERS[@]} non-infrastructure file(s), first: ${OFFENDERS[0]}"
   else
      echo "  note               HEAD is $EXPECT_COMMIT plus approved infrastructure only"
   fi
fi

if [ -n "$EXPECT_WORKTREE" ]; then
   WANT_WT=$(cd -- "$EXPECT_WORKTREE" 2>/dev/null && pwd -P || echo "$EXPECT_WORKTREE")
   [ "$WORKTREE" = "$WANT_WT" ] || refuse "worktree $WORKTREE is not the expected worktree $WANT_WT"
fi

if [ "$DIRTY_COUNT" -gt 0 ]; then
   [ "$ALLOW_DIRTY" -eq 1 ] || refuse "$DIRTY_COUNT tracked file(s) are modified; the candidate must be clean"
   echo "  note               $DIRTY_COUNT modified tracked file(s) allowed by --allow-dirty: NOT a clean candidate" >&2
fi

echo "  result             identity ACCEPTED"

# --- optionally run the gated command ----------------------------------------
if [ "${#RUN[@]}" -gt 0 ]; then
   echo "  running            ${RUN[*]}"
   cd -- "$ROOT" || refuse "cannot enter $ROOT"
   exec "${RUN[@]}"
fi
exit 0
