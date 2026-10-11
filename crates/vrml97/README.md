# vrml97

Read and write VRML97 and X3D classic-encoding files (`.wrl`, `.x3dv`, gzip or plain) in Rust.

`vrml97` is a faithful port of the parser, field types and VRML writer of
[X_ITE](https://github.com/create3000/x_ite). Its goal is to give the same result as X_ITE:
the same scene, the same errors, and byte-identical output from `to_vrml_string()`,
including X_ITE's number formatting (7 significant digits for floats, 15 for doubles).

```rust
let scene = vrml97::load_file(std::path::Path::new("world.wrl"))?;
print!("{}", scene.to_vrml_string());
```

## Status

0.1 covers parsing, PROTO/EXTERNPROTO declarations, ROUTEs, IS mappings, all VRML97/X3D
field types and the classic-encoding writer. It does not run events, routes or Scripts,
does not fetch Inline/EXTERNPROTO URLs, and does not render; those parts exist in the
port but are not published yet.

Tested against X_ITE 16.5.0 on about 1,150 real-world VRML97 files, comparing the
re-serialized scene text byte for byte.

## License

MIT. Includes code ported from X_ITE (Copyright (c) 2009 Holger Seelig and other
contributors, MIT) and fdlibm (Sun Microsystems). See LICENSE.
