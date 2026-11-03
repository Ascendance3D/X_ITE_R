# web-audio-api 1.7.0, patched to build without an AAC decoder

| | |
| --- | --- |
| upstream | `web-audio-api` 1.7.0 on crates.io, <https://github.com/orottier/web-audio-api-rs>, MIT (see `LICENSE`) |
| upstream `.crate` sha256 | `013903031fb735e7fd9aee7c159f3440a19c6969dc81365aec8c64caddcf39bc` (the checksum `Cargo.lock` recorded before this patch) |
| used by | `crates/x3d-runtime` (`web-audio-api = "1.7.0"`), through `[patch.crates-io]` in the root `Cargo.toml` |
| why | Owner direction 2026-11-02 (issue #90): keep AAC out of native and wasm release builds. Upstream has no way to do that: its `aac`, `alac`, `flac`, `m4a`, `mp3`, `ogg` and `wav` features are empty (`aac = []`), and the manifest turns on every Symphonia and Creek codec unconditionally. |
| a fork? | No. Nothing is published; this directory is a patched copy kept in this repository. |

## The change

Everything under `src/` and `resources/`, `LICENSE` and `README.md` is byte-identical to the upstream crate. Only `Cargo.toml` differs, in three ways:

1. `[dependencies.creek]`: `decode-all` is replaced by the same list without `decode-aac`.
2. `[dependencies.symphonia]`: `all` is replaced by the same list without `aac` (`all` is `all-codecs`, `all-formats` and `all-meta`; `isomp4`, `alac`, MP1/MP2/MP3, ADPCM and the rest stay).
3. The `[dev-dependencies.*]` tables are removed, so `Cargo.lock` does not gain test-only crates. The library builds the same; the upstream tests, benches and examples are not part of this copy.

Result: `symphonia-codec-aac` 0.5.5 and 0.6.1 are no longer in `Cargo.lock` at all. `.m4a` files still open (the `isomp4` demuxer stays) and fail with `unsupported audio codec`.

## Check it

```
python3 -I tools/check_vendored_web_audio_api.py            # downloads the crate, compares byte for byte
python3 -I tools/check_vendored_web_audio_api.py <file.crate> # offline, from a copy of the .crate
python3 -I tools/check_no_aac.py                             # Cargo.lock has no AAC decoder (also run by the test suite)
cargo tree --locked -e normal,build,dev --target all -i symphonia-codec-aac@0.6.1   # must say: did not match any packages
```

## Maintenance

* Moving to a newer `web-audio-api`: take the new crate, redo the three `Cargo.toml` changes, update the checksum here and in `tools/check_vendored_web_audio_api.py`, and rerun the checks above. `check_no_aac.py` and the `cargo_lock_has_no_aac_decoder` test fail if the new version brings an AAC decoder back; a different new codec in `all` is not noticed, so read the new manifest's feature lists.
* Dropping this patch: when upstream (or a release of it) lets a build leave AAC out, delete this directory and the `[patch.crates-io]` entry, turn that option off in `crates/x3d-runtime/Cargo.toml`, and keep `check_no_aac.py`.
* Security fixes in upstream are not picked up automatically. Watch the crate's releases and advisories.
