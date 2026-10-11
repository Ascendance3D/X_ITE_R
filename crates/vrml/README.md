# vrml

A toolkit and command-line tool for VRML / X3D classic-encoding files (`.wrl`, `.x3dv`,
gzip or plain).

```sh
cargo install vrml
vrml check world.wrl other.wrl   # parse, list warnings, exit 1 on errors
vrml fmt world.wrl > clean.x3dv  # re-write with X_ITE's writer (outputs X3D classic encoding)
vrml info world.wrl              # header, size, node/PROTO/ROUTE counts, top node types
```

As a library: `vrml::detect` reads the declared version (VRML 1.0, VRML97, X3D x.y),
`vrml::summarize` counts a scene's contents, and `vrml::load_file` / `parse_bytes`
parse through [`vrml97`](https://crates.io/crates/vrml97), a faithful Rust port of
X_ITE's parser and writer.

## Coming next

`vrml` will grow into the front door of a Rust VRML/X3D toolchain. Already working in
the (not yet published) port, each checked against X_ITE output:

- running worlds: events, routes, PROTOs, Inlines, Scripts
- a wgpu renderer and an interactive viewer (`vrml view`)
- a native VRML-aware code editor based on a CodeMirror 5 port

These will be published as they reach parity.

## License

MIT. Includes code ported from X_ITE (Copyright (c) 2009 Holger Seelig and other
contributors, MIT). See LICENSE.
