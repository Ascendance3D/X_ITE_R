#!/usr/bin/env bash
# Build x3d-view in release mode and pack it with the launcher and the sample world.
# Local only: nothing here publishes or pushes.
#
# Usage: tools/package-desktop.sh <output-dir>
#   CARGO_TARGET_DIR / CARGO_BUILD_BUILD_DIR are honoured (use isolated directories).
#
# Result: <output-dir>/x3d-view-linux-x86_64/ and <output-dir>/x3d-view-linux-x86_64.tar.gz
set -euo pipefail

out=${1:?usage: tools/package-desktop.sh <output-dir>}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}

(cd "$root" && cargo build --release -p x3d-viewer --bin x3d-view)

dir="$out/x3d-view-linux-x86_64"
rm -rf "$dir"
mkdir -p "$dir/samples"
install -m 755 "$target/release/x3d-view" "$dir/x3d-view"
install -m 755 "$root/tools/x3d-view-launch.sh" "$dir/x3d-view-launch.sh"
cp -r "$root/crates/x3d-viewer/samples/desktop-demo" "$dir/samples/"

cat > "$dir/README.txt" <<'TXT'
X_ITE_R x3d-view: native VRML97 / X3D viewer (Linux x86_64)

Run:
  ./x3d-view-launch.sh                      opens samples/desktop-demo/demo.wrl
  ./x3d-view-launch.sh /path/world.wrl      any .wrl, .wrz (gzip), .x3d, .x3dv

Keys: 1 EXAMINE, 2 WALK, 3 FLY, 4 NONE, PageUp / PageDown previous / next viewpoint,
Home / End first / last viewpoint, l list viewpoints, r reload, s screenshot to /tmp,
m multisampling, Esc or q quit. Mouse: left drag moves the camera (EXAMINE rotates,
WALK / FLY move), middle drag pans, wheel zooms; click TouchSensor objects.
The window title shows the world, the navigation type, the viewpoint and the last message.

Needs (Ubuntu 24.04 package names):
  libvulkan1 and a Vulkan driver (NVIDIA proprietary driver, or mesa-vulkan-drivers)
  libasound2t64 (sound; with PipeWire or PulseAudio the ALSA default device is used)
  X11: libx11-6 libx11-xcb1 libxcursor1 libxi6 libxrandr2 libxkbcommon-x11-0
  Wayland: libwayland-client0 libwayland-cursor0 libxkbcommon0
No Rust toolchain or build directory is needed.
TXT

tar -C "$out" -czf "$out/x3d-view-linux-x86_64.tar.gz" x3d-view-linux-x86_64
echo "$dir"
