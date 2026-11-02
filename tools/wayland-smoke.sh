#!/usr/bin/env bash
# Wayland window smoke run (issue #88): tools/wayland-smoke.sh
#
# Starts a private headless Weston (pixman renderer, its own socket and runtime directory), then runs x3d-view with DISPLAY
# unset so winit takes its Wayland backend, in the deterministic scripted mode (3 frames, one PNG). It checks that the window
# is created on a Wayland compositor, the wgpu surface works, and frames are presented. A control run with a Wayland socket
# that does not exist must fail, which shows the first run used Wayland.
#
# Not covered: key delivery. A headless Weston has no wl_seat, so no keyboard event reaches the window. The key path from
# winit key events to KeySensor / StringSensor is checked without a window by crates/x3d-viewer/tests/native_keys.rs; the
# compositor-to-winit part (real keymap, layout, repeat, modifiers) and a hardware keyboard stay unverified, as does macOS.
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in weston; do
  command -v "$tool" >/dev/null || { echo "wayland-smoke: $tool is missing: not verified" >&2; exit 3; }
done

cargo build --release -p x3d-viewer --features gpu --bin x3d-view
view="${CARGO_TARGET_DIR:-target}/release/x3d-view"

work="$(mktemp -d)"
chmod 700 "$work"
weston_pid=""
cleanup() { [ -n "$weston_pid" ] && kill "$weston_pid" 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

cat > "$work/world.wrl" <<'W'
#VRML V2.0 utf8
Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box {} }
W
echo '{"frames":3,"events":[]}' > "$work/script.json"

export XDG_RUNTIME_DIR="$work"
weston --backend=headless --renderer=pixman --socket=xr88-wl --no-config --idle-time=0 > "$work/weston.log" 2>&1 &
weston_pid=$!
for _ in $(seq 50); do [ -S "$work/xr88-wl" ] && break; sleep 0.1; done
[ -S "$work/xr88-wl" ] || { echo "wayland-smoke: Weston did not start:" >&2; cat "$work/weston.log" >&2; exit 1; }

out="$(env -u DISPLAY WAYLAND_DISPLAY=xr88-wl timeout 60 "$view" "$work/world.wrl" --size 320x240 \
  --script "$work/script.json" --png "$work/out.png" 2>&1)"
echo "$out" | grep -E "presented|final pose" || true
echo "$out" | grep -q "presented 3 frames" || { echo "wayland-smoke: FAIL: 3 frames were not presented" >&2; echo "$out" >&2; exit 1; }
[ -s "$work/out.png" ] || { echo "wayland-smoke: FAIL: no PNG" >&2; exit 1; }

if env -u DISPLAY WAYLAND_DISPLAY=xr88-nonexistent timeout 60 "$view" "$work/world.wrl" --size 320x240 \
    --script "$work/script.json" --png "$work/control.png" >/dev/null 2>&1; then
  echo "wayland-smoke: FAIL: the control run with no Wayland socket succeeded, so the first run proves nothing" >&2
  exit 1
fi

echo "wayland-smoke: PASS: window on Weston headless (Wayland), 3 frames, PNG written; control without a socket failed."
echo "wayland-smoke: key delivery, hardware keyboard and macOS are not covered."
