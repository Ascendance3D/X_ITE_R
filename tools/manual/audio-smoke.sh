#!/usr/bin/env bash
# Scripted native audio smoke run (issue #88): tools/manual/audio-smoke.sh [seconds]
#
# Records the monitor of the default sink (parec), plays tools/manual/audio-tone.x3dv (a quiet 880 Hz sine, gain 0.05)
# in x3d-view through the real cpal output, then checks the recording: RMS above a floor and the strongest frequency
# at 880 Hz (Goertzel). It does not change any audio setting. Needs a graphical session (x3d-view opens a window).
#
# Exit codes: 0 PASS, 1 FAIL, 77 SKIPPED (no audio server, no sink, or no display).
# Env: X3D_VIEW=<path to a built x3d-view> skips the build.
# Not covered: macOS (CoreAudio), keyboard input, and that sound leaves the speakers (the monitor taps the sink).
set -u
skip() { echo "SKIPPED: $*"; exit 77; }
fail() { echo "FAIL: $*"; exit 1; }
secs="${1:-6}"
[[ "$secs" =~ ^[1-9][0-9]?$ ]] || fail "seconds must be an integer from 1 to 99 (got '$secs')"
# A relative X3D_VIEW is relative to the caller's directory, so resolve it before the cd.
if [ -n "${X3D_VIEW:-}" ]; then
  X3D_VIEW="$(realpath -- "$X3D_VIEW")" || fail "X3D_VIEW does not resolve: $X3D_VIEW"
fi
cd "$(dirname "$0")/../.." || exit 1
world="tools/manual/audio-tone.x3dv"
tmp="$(mktemp -d /tmp/audio-smoke.XXXXXX)"
trap 'kill "${rec:-}" "${view:-}" 2>/dev/null; rm -rf "$tmp"' EXIT

echo "session: ${XDG_SESSION_TYPE:-unknown} (WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-none}, DISPLAY=${DISPLAY:-none})"
for tool in timeout python3 pactl parec; do
  command -v "$tool" >/dev/null 2>&1 || skip "$tool not installed"
done
sink="$(timeout 10 pactl get-default-sink 2>/dev/null)" || skip "no PulseAudio/PipeWire server"
[ -n "$sink" ] || skip "no default sink"
[ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ] || skip "no display for x3d-view"
echo "audio server: $(timeout 10 pactl info | sed -n 's/^Server Name: //p')"
echo "default sink: $sink"
timeout 10 pactl list short sources 2>/dev/null | cut -f2 | grep -qxF "$sink.monitor" || skip "the default sink has no monitor source ($sink.monitor)"

view_bin="${X3D_VIEW:-}"
if [ -z "$view_bin" ]; then
  timeout 900 cargo build --release -p x3d-viewer --bin x3d-view >&2 || { echo "FAIL: build"; exit 1; }
  view_bin="$(cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')/release/x3d-view"
fi
[ -x "$view_bin" ] || { echo "FAIL: no x3d-view binary at $view_bin"; exit 1; }

rate=48000
timeout -k 5 $((secs + 20)) parec -d "$sink.monitor" --rate=$rate --format=s16le --channels=1 --raw >"$tmp/cap.raw" 2>"$tmp/rec.err" &
rec=$!
sleep 1
timeout -k 5 "$secs" "$view_bin" "$world" >"$tmp/view.out" 2>&1 &
view=$!
wait "$view" 2>/dev/null
view_status=$?
sleep 0.5
kill "$rec" 2>/dev/null; wait "$rec" 2>/dev/null
rec=""; view=""
# 124: timeout ended the viewer, as planned. Any other status is the viewer's own failure.
if [ "$view_status" -ne 0 ] && [ "$view_status" -ne 124 ]; then
  echo "FAIL: x3d-view exited with status $view_status"
  sed -n '1,60p' "$tmp/view.out"
  exit 1
fi

python3 -I - "$tmp/cap.raw" "$rate" 880 "$sink" <<'PY'
import array, math, sys
path, rate, want, sink = sys.argv[1], int(sys.argv[2]), float(sys.argv[3]), sys.argv[4]
a = array.array("h")
with open(path, "rb") as f:
    data = f.read()
a.frombytes(data[: len(data) // 2 * 2])
x = [v / 32768.0 for v in a]
if len(x) < rate:
    print(f"FAIL: captured only {len(x)} samples"); sys.exit(1)
# Loudest 1 s window: the capture also holds silence before the tone starts.
win = rate
best = max(range(0, len(x) - win + 1, rate // 4), key=lambda s: sum(v * v for v in x[s:s + win]))
w = x[best:best + win]
rms = math.sqrt(sum(v * v for v in w) / len(w))
peak = max(abs(v) for v in w)
def goertzel(f):
    k = 2 * math.cos(2 * math.pi * f / rate); s1 = s2 = 0.0
    for v in w:
        s1, s2 = v + k * s1 - s2, s1
    return math.sqrt(max(s1 * s1 + s2 * s2 - k * s1 * s2, 0.0)) * 2 / len(w)
freqs = [f / 2 for f in range(100, 4000)]
best_f = max(freqs, key=goertzel)
amp = goertzel(best_f)
print(f"sink={sink} rate={rate} Hz window=1 s rms={rms:.5f} peak={peak:.5f} peak_freq={best_f:.1f} Hz amplitude={amp:.5f}")
ok = rms > 0.005 and abs(best_f - want) <= 5 and amp > 0.01
print("PASS" if ok else "FAIL", f"(need rms>0.005, peak_freq within 5 Hz of {want:.0f}, amplitude>0.01)")
sys.exit(0 if ok else 1)
PY
exit $?
