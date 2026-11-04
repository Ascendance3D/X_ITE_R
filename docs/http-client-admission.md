# HTTP client admission record (crate `x3d-http`)

Task: FileLoader needs a `Fetcher` that answers with status, headers, final URL and body, follows redirects, and speaks HTTPS (issue sub:io.file-loader,
owner decision 2026-11-04). The rule: a maintained client, no HTTP stack of our own, no new system package.

## Choice

`ureq` 3.5.1 (MIT OR Apache-2.0), default features: `rustls` with the `ring` provider and `webpki-roots`, and `gzip`.

| Candidate | Verdict |
| --- | --- |
| `ureq` 3 | Chosen. Blocking, no async runtime, pure Rust apart from `ring`, small tree (17 new packages). Redirects can be turned off (`max_redirects (0)`), so `x3d-http` checks each hop. |
| `reqwest` (blocking) | Not chosen. It brings `tokio`, `hyper` and a thread for its runtime for one blocking GET. |
| `native-tls` / OpenSSL | Not chosen. It needs system packages on Linux, and the repository uses no OpenSSL. |
| `minreq`, `attohttpc` | Not chosen. Less used, and less control of redirects. |

## Licenses of the new packages (`cargo metadata`, 2026-11-04)

MIT OR Apache-2.0: ureq, ureq-proto, http, httparse, base64, getrandom, rustls-pki-types, utf8-zero, zeroize, wasi (with LLVM exception).
Others: `rustls` (Apache-2.0 OR ISC OR MIT), `ring` (Apache-2.0 AND ISC), `rustls-webpki` (ISC), `untrusted` (ISC), `subtle` (BSD-3-Clause),
`webpki-roots` (CDLA-Permissive-2.0, the Mozilla root list as data).

The decoder rule BR1.7 (`docs/decoder-admission.md`) names MIT, Apache-2.0 and BSD and is about decoders. This record is not a decoder admission. ISC is a
permissive license of the BSD family, and CDLA-Permissive-2.0 is a permissive data license; neither is on the BR1.7 list, and no other rule or `deny.toml`
covers them. The owner can replace `webpki-roots` by the roots of the operating system (`platform-verifier`) at the cost of a system dependency.

## Platforms

No package outside the Rust crates is needed. `ring` compiles C and assembly with the C compiler that the CI jobs already have (Linux, macOS, Windows). The
crate is a dependency of `x3d-viewer` through the `script` feature only, so the wasm build of `x3d-web` does not contain it (`cargo tree -p x3d-web -i ureq`
finds nothing).
