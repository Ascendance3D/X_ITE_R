# synthetic-rea-formats

Parser oracle inputs for the Rust port, run by `oracle/x_ite-formats.mjs`.

- `xml/`: X3D XML (`.x3d`), syntax, DEF/USE, fields, PROTO, ROUTE, IMPORT/EXPORT.
- `json/`: X3D JSON (`.json`), plus one non-X3D file expected to fail.
- `stl/`, `ply/`, `obj/`: ASCII and binary STL, PLY, and OBJ with MTL.
- `formats-baseline.json`: recorded X_ITE output for these inputs.

Every input is newly authored synthetic data; no third-party models or copied assets.
