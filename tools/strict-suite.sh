#!/usr/bin/env bash
# The local strict Rust suite (issue #185): every unit, integration and GPU test with cargo-nextest, then the
# doctests with `cargo test --doc`. Both steps must pass. It runs strict (X3D_EVIDENCE unset), so a missing answer
# key or GPU adapter fails, as the command in the header of .github/workflows/rust-tests.yml does.
#
#   X3D_ORACLE_KEYS=<evidence store> tools/strict-suite.sh [extra nextest arguments]
#
# Settings (environment):
#   XITE_R_TEST_JOBS    concurrent tests, default 8, at least 6
#   XITE_R_BUILD_JOBS   compiler jobs, default 12
#   XITE_R_SUITE_DIR    build and output directory, default /tmp/xiter-suite-<worktree name>
#   XITE_R_MEMORY_MAX   memory limit of the run (systemd scope), default 16G
#
# Build isolation: the workspace .cargo/config.toml of ~/Projects/cybertown sets build.build-dir to a cache that
# every worktree shares, and CARGO_TARGET_DIR does not move build.build-dir. This script sets both
# CARGO_BUILD_BUILD_DIR and CARGO_TARGET_DIR to XITE_R_SUITE_DIR and holds a lock on that directory, so a second
# strict-suite run cannot share it. The private directory is the protection against other builds: a cargo command
# started by hand with the same CARGO_BUILD_BUILD_DIR is not locked out. A scan of running cargo processes at the start
# catches that case only when it is already running (advisory).
#
# Prerequisites: a systemd user session (`systemd-run --user`, for the memory limit), flock, and GNU time at
# /usr/bin/time. The memory peak comes from cgroup v2 (memory.peak); the summary says so when it is not available.
#
# NVIDIA's shader disk cache has a size limit; when it is full, a changed shader is compiled again in every test
# process. __GL_SHADER_DISK_CACHE_SIZE raises the limit for this run only (it changes no file and no profile).
set -euo pipefail

# The absolute path of this script, also when it is called through PATH or a symlink.
self=$(readlink -f "${BASH_SOURCE[0]}")
root=$(cd "$(dirname "$self")/.." && pwd -P)
jobs=${XITE_R_TEST_JOBS:-8}
build_jobs=${XITE_R_BUILD_JOBS:-12}
suite_dir=${XITE_R_SUITE_DIR:-/tmp/xiter-suite-$(basename "$root")}
memory_max=${XITE_R_MEMORY_MAX:-16G}

fail() {
    echo "strict-suite: $*" >&2
    exit 2
}

[[ $jobs =~ ^[0-9]+$ ]] || fail "XITE_R_TEST_JOBS must be a number: $jobs"
((jobs >= 6)) || fail "XITE_R_TEST_JOBS=$jobs: at least 6 concurrent tests are required"
[[ $build_jobs =~ ^[0-9]+$ ]] || fail "XITE_R_BUILD_JOBS must be a number: $build_jobs"
[[ -n ${X3D_ORACLE_KEYS:-} ]] || fail "X3D_ORACLE_KEYS is not set (the strict suite needs the answer keys)"
[[ -z ${X3D_EVIDENCE:-} ]] || fail "X3D_EVIDENCE=$X3D_EVIDENCE: the local suite runs strict, unset it"
cargo nextest --version >/dev/null 2>&1 || fail "cargo-nextest is not installed"
command -v systemd-run >/dev/null 2>&1 || fail "systemd-run is not available (a systemd user session is required)"
command -v flock >/dev/null 2>&1 || fail "flock is not installed"
[[ -x /usr/bin/time ]] || fail "GNU time (/usr/bin/time) is not installed"

# Run the rest inside a memory-limited scope, as the rust-tests.yml header command does.
if [[ -z ${XITE_R_IN_SCOPE:-} ]]; then
    exec systemd-run --user --scope --quiet -p "MemoryMax=$memory_max" -p MemorySwapMax=0 \
        env XITE_R_IN_SCOPE=1 "$self" "$@"
fi

mkdir -p "$suite_dir"
exec 9>"$suite_dir/.lock"
flock -n 9 || fail "another strict-suite run holds $suite_dir/.lock"

# Advisory: a cargo process (outside this lock) that already builds into the same directory would overwrite this
# run's files. This finds only a process that is running now; the private build directory is the real protection.
for env_file in /proc/[0-9]*/environ; do
    pid=${env_file#/proc/}
    pid=${pid%/environ}
    [[ $pid != "$$" ]] || continue
    comm=$(cat "/proc/$pid/comm" 2>/dev/null) || continue
    [[ $comm == cargo ]] || continue
    if tr '\0' '\n' <"$env_file" 2>/dev/null | grep -qxF "CARGO_BUILD_BUILD_DIR=$suite_dir/build"; then
        fail "cargo process $pid already builds into $suite_dir/build"
    fi
done

export CARGO_BUILD_BUILD_DIR="$suite_dir/build"
export CARGO_TARGET_DIR="$suite_dir/target"
export CARGO_BUILD_JOBS="$build_jobs"
export __EGL_VENDOR_LIBRARY_FILENAMES=${__EGL_VENDOR_LIBRARY_FILENAMES:-/usr/share/glvnd/egl_vendor.d/50_mesa.json}
export __GL_SHADER_DISK_CACHE_SIZE=${__GL_SHADER_DISK_CACHE_SIZE:-10000000000}
export NEXTEST_HIDE_PROGRESS_BAR=1
cd "$root"

summary="$suite_dir/summary.txt"
{
    echo "commit            $(git rev-parse HEAD)$(git diff --quiet HEAD -- || echo ' (dirty)')"
    echo "build dir         $CARGO_BUILD_BUILD_DIR"
    echo "target dir        $CARGO_TARGET_DIR"
    echo "test jobs         $jobs"
    echo "build jobs        $build_jobs"
    echo "memory max        $memory_max"
    echo "shader cache size $__GL_SHADER_DISK_CACHE_SIZE"
} | tee "$summary"

# Runs one step, records its wall-clock and CPU time, and returns its exit status.
step() {
    local name=$1 log=$2
    shift 2
    local start end status
    start=$(date +%s%N)
    set +e
    /usr/bin/time -f '%U %S %P %M' -o "$suite_dir/$name.time" "$@" 2>&1 | tee "$log"
    status=${PIPESTATUS[0]}
    set -e
    end=$(date +%s%N)
    read -r user sys cpu rss < <(tail -1 "$suite_dir/$name.time") || true
    local tenths=$(((end - start) / 100000000))
    printf '%-9s exit %d, wall %d.%d s, user %s s, sys %s s, cpu %s, max rss %s KiB\n' \
        "$name" "$status" $((tenths / 10)) $((tenths % 10)) "${user:-?}" "${sys:-?}" "${cpu:-?}" "${rss:-?}" |
        tee -a "$summary"
    return "$status"
}

build_status=0
step build "$suite_dir/build.log" \
    cargo nextest run --workspace --release --no-run --build-jobs "$build_jobs" || build_status=$?

if ((build_status != 0)); then
    echo "FAILED: the build (see $suite_dir/build.log)" | tee -a "$summary"
    exit "$build_status"
fi

nextest_status=0
step nextest "$suite_dir/nextest.log" \
    cargo nextest run --workspace --release --build-jobs "$build_jobs" --test-threads "$jobs" --no-fail-fast "$@" ||
    nextest_status=$?

doc_status=0
step doctests "$suite_dir/doctests.log" \
    cargo test --workspace --release --doc -j "$build_jobs" --no-fail-fast || doc_status=$?

# The counts are information only: the pass or fail result comes from the exit status of each step, below.
{
    nextest_summary=$(grep -E '^ *Summary \[' "$suite_dir/nextest.log" | tail -1 | sed 's/^ *//') || true
    echo "nextest:  ${nextest_summary:-WARNING: no nextest Summary line in $suite_dir/nextest.log (exit status $nextest_status)}"
    grep '^test result' "$suite_dir/doctests.log" |
        awk '{n++; p+=$4; f+=$6; i+=$8} END {if (n) printf "doctests: %d passed, %d failed, %d ignored\n", p, f, i; else print "doctests: WARNING: no test result line"}'
    cg=$(cut -d: -f3 /proc/self/cgroup)
    if [[ -r /sys/fs/cgroup$cg/memory.peak ]]; then
        echo "memory peak $(($(cat "/sys/fs/cgroup$cg/memory.peak") / 1048576)) MiB"
    else
        echo "memory peak unavailable (no cgroup v2 memory.peak for this scope)"
    fi
} | tee -a "$summary"

if ((nextest_status != 0)); then
    echo "FAILED: nextest (exit $nextest_status); failed tests:" | tee -a "$summary"
    grep -E '^ +(FAIL|SIGSEGV|SIGABRT|SIGKILL|TIMEOUT|LEAK-FAIL) ' "$suite_dir/nextest.log" | sort -u | tee -a "$summary" || true
fi

if ((doc_status != 0)); then
    echo "FAILED: doctests (exit $doc_status)" | tee -a "$summary"
    grep -E '^test .* FAILED$' "$suite_dir/doctests.log" | tee -a "$summary" || true
fi

((nextest_status == 0 && doc_status == 0)) || exit 1
echo "PASSED: nextest and doctests ($summary)" | tee -a "$summary"
