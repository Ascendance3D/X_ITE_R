#!/usr/bin/env bash
# Audio output smoke run (issue #88): tools/audio-smoke.sh
#
# Plays a quiet 440 Hz tone (gain 0.05) through the native AudioContext on the default output device and checks that the
# device clock runs and the tone reaches an analyser (crates/x3d-runtime/tests/audio_device_smoke.rs). It prints the
# session type and the audio server first. Without an output device the run fails with "evidence no-audio-device": it
# never reports a pass for a host that could not play. Set X3D_EVIDENCE=ci and X3D_EVIDENCE_ALLOW=no-audio-device to
# turn that failure into a named skip in X3D_EVIDENCE_LOG.
#
# Not covered: that sound leaves the speakers, and macOS (CoreAudio).
set -euo pipefail
cd "$(dirname "$0")/.."

echo "session: ${XDG_SESSION_TYPE:-unknown} (WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-none}, DISPLAY=${DISPLAY:-none})"
if command -v pactl >/dev/null 2>&1; then
  echo "audio server: $(pactl info 2>/dev/null | sed -n 's/^Server Name: //p' | head -1)"
  echo "default sink: $(pactl get-default-sink 2>/dev/null || echo none)"
fi

# Run the one ignored test. -j and CARGO_BUILD_JOBS are the caller's choice; the default is half the CPUs.
jobs="${CARGO_BUILD_JOBS:-$(( $(nproc) / 2 ))}"
exec cargo nextest run -j "$jobs" -p x3d-runtime --test audio_device_smoke --run-ignored only --no-capture
