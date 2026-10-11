# x3d

X3D and VRML97 node reference for Rust, plus reading and writing the classic encoding.

```rust
let t = x3d::node("Transform").unwrap();
for f in t.fields() {
    println!("{} {} {} {}", f.access(), f.type_name(), f.name(), f.default_vrml());
}
```

- All 263 node types X_ITE 16.5 supports, VRML97 through X3D 4.1.
- Each node's component, containerField and specification range.
- Each field's access type, field type and default value.
- Components and profiles.
- `load_file` / `parse_bytes` read `.wrl` / `.x3dv` (gzip or plain) through
  [`vrml97`](https://crates.io/crates/vrml97), a faithful port of X_ITE's parser.

The data is taken from [X_ITE](https://github.com/create3000/x_ite)'s node definitions.

## Coming next

`x3d` is the home of a Rust port of the X_ITE X3D browser. Already working in the
(not yet published) port, each checked against X_ITE output:

- events, routes, PROTO instancing, Inline loading and Scripts (QuickJS)
- a wgpu renderer (shapes, lighting, textures, text, fog, backgrounds)
- an interactive viewer with EXAMINE / WALK / FLY navigation and pointing sensors

These will be published as they reach parity with X_ITE.

## License

MIT. Includes data and code ported from X_ITE (Copyright (c) 2009 Holger Seelig and
other contributors, MIT). See LICENSE.
