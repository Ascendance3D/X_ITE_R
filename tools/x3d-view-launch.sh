#!/usr/bin/env bash
# Launch the native X_ITE_R viewer (x3d-view) on a VRML97 / X3D world.
#
# Usage:
#   x3d-view-launch.sh [world.wrl|world.wrz|world.x3d|world.x3dv] [x3d-view options...]
#
# With no world it opens the bundled sample (samples/desktop-demo/demo.wrl).
# It finds x3d-view next to this script (an unpacked package) or in the repository's
# target/release (or $CARGO_TARGET_DIR/release). Options after the world go to x3d-view
# unchanged: --size WxH, --msaa 0|1|2|4|8|16, --depth standard|log|reversed.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bin=""

for c in "$here/x3d-view" "${CARGO_TARGET_DIR:-$here/../target}/release/x3d-view"; do
    if [ -x "$c" ]; then
        bin=$c
        break
    fi
done

if [ -z "$bin" ]; then
    echo "x3d-view not found. Build it: cargo build --release -p x3d-viewer --bin x3d-view" >&2
    exit 2
fi

if [ $# -eq 0 ] || [[ $1 == --* ]]; then
    for s in "$here/samples/desktop-demo/demo.wrl" "$here/../crates/x3d-viewer/samples/desktop-demo/demo.wrl"; do
        if [ -f "$s" ]; then
            set -- "$s" "$@"
            break
        fi
    done
fi

if [ ! -f "$1" ]; then
    echo "world not found: $1" >&2
    exit 2
fi

exec "$bin" "$(realpath "$1")" "${@:2}"
