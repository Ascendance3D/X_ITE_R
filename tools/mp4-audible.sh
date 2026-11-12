#!/usr/bin/env bash
# MP4 sound on the real output path (ADR 0015): tools/mp4-audible.sh
#
# Linux with PipeWire or PulseAudio. The script makes a null sink, makes it the default for the run, records its monitor
# with parec while the ignored test realtime_output_plays_the_movie_sound (crates/x3d-runtime/tests/mp4_gst.rs) plays
# h264-aac.mp4 through a Sound node, then restores the default sink and removes the null sink. The tones can still be
# heard on the speakers: on the first host (PipeWire 1.0.5) the owner heard them during the run, so the null sink does
# not isolate the stream there. The tones are 2 s long at half scale. tools/mp4_audible_check.py then finds the 8 tones
# of the file (300 to 1000 Hz, 0.25 s each) in the recording, in order. The run fails when a tone is missing or out of order.
set -euo pipefail
cd "$(dirname "$0")/.."

out="$(mktemp -d "${TMPDIR:-/tmp}/xiter-mp4-audible.XXXXXX")"
old="$(pactl get-default-sink)"
module="$(pactl load-module module-null-sink sink_name=xiter_mp4_audible)"
cleanup() {
  pactl set-default-sink "$old" || true
  pactl unload-module "$module" || true
  rm -rf "$out"
}
trap cleanup EXIT

jobs="${CARGO_BUILD_JOBS:-$(($(nproc) / 2))}"
# Build first, so that the recording holds the run only.
cargo nextest run -j "$jobs" -p x3d-runtime --features system-video-gst --test mp4_gst --no-run

pactl set-default-sink xiter_mp4_audible
parec --latency-msec=20 --device=xiter_mp4_audible.monitor --format=float32le --rate=48000 --channels=2 --raw >"$out/capture.f32" &
rec=$!
sleep 0.5

status=0
cargo nextest run -j "$jobs" -p x3d-runtime --features system-video-gst --test mp4_gst --run-ignored only \
  -E 'test(realtime_output_plays_the_movie_sound)' --no-capture || status=$?

# parec writes its buffer late: wait before it stops.
sleep 1.5
kill -INT "$rec"
wait "$rec" 2>/dev/null || true
echo "default sink during the run: xiter_mp4_audible (restored to $old)"
python3 -I -B tools/mp4_audible_check.py "$out/capture.f32" || status=$?
exit "$status"
