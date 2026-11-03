# Third-party notices

Code and data in this repository that derive from third-party works, with the notices their licenses require.

## pl_mpeg

`crates/x3d-runtime/src/mpeg1/` (the MPEG-1 video decoder, `video.rs`, and the code tables, `tables.rs`) is a Rust port
of the MPEG-1 video decoder of pl_mpeg, with changes listed in the header of `video.rs` and in
`docs/mpeg1-implementation.md`.

- Upstream: https://github.com/phoboslab/pl_mpeg, commit `c871f2be022ece7ef4f64230b4fb8e1fb9eb6023`
  ("Fix corrupt slice check; close #64", 2025-12-30)
- `pl_mpeg.h` SHA-256 at that commit: `3a8cb30c83c2a1147719c30fe0c8b93da2987aa43140077c575b39aaa75fc2c9`
- Author: Dominic Szablewski (https://phoboslab.org)
- License: MIT. The source states it on its line 3, `SPDX-License-Identifier: MIT`, and the repository README calls it a
  "Single-file MIT licensed library". The repository has no separate license file at this commit (the tree holds
  `README.md`, `makefile`, `pl_mpeg.h` and three example programs), so the upstream copyright line is not available. The text
  below is the standard MIT text; the holder is named as the header's author line names him. Whether to ask the author
  for a license file is a decision for the owner.
- The header of `pl_mpeg.h` says its video decoder is "inspired by Java MPEG-1 Video Decoder and Player by Zoltan Korandi"
  (https://sourceforge.net/projects/javampeg1video/). Its audio decoder, based on kjmp2 by Martin J. Fiedler, is not used.

MIT License

Copyright (c) Dominic Szablewski

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
