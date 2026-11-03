# MPEG-1 test fixtures

Newly authored, synthetic (ffmpeg `lavfi` sources `testsrc2` and `sine`), no third-party or historical content. They are
inputs of `crates/x3d-runtime/tests/mpeg1_decode.rs`. The commands are in `docs/mpeg1-implementation.md` and in the
doc comment of that test; the reference planes (`*.yuv.gz`, gzip -9n) are what ffmpeg 6.1.1 (`mpeg1video` decoder) decodes from each
stream with `-f rawvideo -pix_fmt yuv420p`.
