# rav1d 1.1.0, patched so a damaged AV1 tile does not abort the process

| | |
| --- | --- |
| upstream | `rav1d` 1.1.0 on crates.io, <https://github.com/memorysafety/rav1d>, BSD-2-Clause (see `COPYING`) |
| upstream `.crate` sha256 | `1932f060d5e7bd49dc9f8b272c1dc5e9ce0ffe141c28be900265d3989b36c9ed` |
| used by | `crates/x3d-runtime` (`rav1d`, optional feature `av1`), through `[patch.crates-io]` in the root `Cargo.toml` |
| a fork? | No. Nothing is published; this directory is a patched copy kept in this repository. |

## The bug

`rav1d` exposes only `extern "C"` functions. A Rust panic inside an `extern "C"` function cannot unwind, so the process
aborts (`panic in a function that cannot unwind`, SIGABRT), and `catch_unwind` around the call does not help.

Exact path (found with a fuzz of damaged streams, `crates/x3d-runtime/tests/av1_safety.rs`; 81 of 300 mutated files aborted):

1. `dav1d_send_data` (`src/lib.rs`) -> `rav1d_send_data` -> `gen_picture` -> `rav1d_parse_obus` (`src/obu.rs:2644`)
2. -> `rav1d_submit_frame` (`src/obu.rs:2626`, `src/decode.rs:4934`), single frame thread: `rav1d_decode_frame` fails on corrupt
   tile data. Its exit path (`rav1d_decode_frame_exit`) already clears the frame context's `frame_hdr`.
3. `rav1d_submit_frame` then calls its local `on_error` (`src/decode.rs:5343`), whose first statement
   `f.frame_hdr.as_ref().unwrap().refresh_context` (`src/decode.rs:4997`) unwraps that `None`: panic, abort.

It is a recoverable error (the function was about to `return Err`), not an overflow, an OOM, an explicit abort or an
`unreachable!`. No other panic or abort site was found: after the patch 6000 damaged files in a debug build and 3000 in a
release build all ended with a decode result or an error (see Check it).

## The change

One statement in `src/decode.rs` (`on_error`): `f.frame_hdr.as_ref().unwrap().refresh_context != 0` becomes
`f.frame_hdr.as_ref().is_none_or(|h| h.refresh_context != 0)`, with a comment. When the header is already gone the
output CDF is released too, which is the safe direction. Everything else is byte-identical to the crate, except what is
left out: `.github/`, `.gitlab-ci.yml`, `.gitignore` (upstream CI files that mean nothing here) and `.cargo-ok`.

## Check it

```
python3 -I tools/check_vendored_rav1d.py                 # downloads the crate, compares byte for byte
python3 -I tools/check_vendored_rav1d.py <file.crate>    # offline, from a copy of the .crate
cargo nextest run -p x3d-runtime --features av1 --test av1_safety   # child processes decode damaged files
AV1_SAFETY_SEEDS=6000 cargo nextest run -p x3d-runtime --features av1 --test av1_safety
```

`av1_safety` runs every damaged file in a CHILD process and fails if one dies by a signal, so it proves the property that
`catch_unwind` cannot.

## Maintenance

* Moving to a newer `rav1d`: take the new crate, look whether `on_error` still unwraps `frame_hdr` (if upstream fixed it,
  delete the patch), update the checksum here and in `tools/check_vendored_rav1d.py`, run both checks above with a large
  `AV1_SAFETY_SEEDS`, and read the new release notes for other `extern "C"` panics.
* Dropping this patch: when a rav1d release contains the fix (or exposes a Rust API that returns errors), delete
  `vendor/rav1d` and the `[patch.crates-io]` entry; keep `av1_safety.rs` as the regression test.
* Security fixes in upstream are not picked up automatically. Watch the crate's releases and advisories.
* A new kind of damaged stream could reach another unwrap. The fuzz is not a proof; keep `AV1_SAFETY_SEEDS` high when
  rav1d or the wrapper (`crates/x3d-runtime/src/av1.rs`, `Av1Decoder::check`) changes.
