# Function coverage summary

Upstream: X_ITE 16.5.0 @ 34cdf9f97 (`oracle/x_ite-pin/src`). Generator: `ledger/functions/tools/` (extractor and builder; the rules are in the
builder's module documentation). This file is generated: do not edit it.

A status other than `UNRESOLVED_MAPPING` comes only from an inspected record of `ledger/functions/dispositions.jsonl`.
`UNRESOLVED_MAPPING` means that nobody has inspected the function. It does not mean that the implementation is missing.

## Counts

| item | count |
| --- | --- |
| JavaScript files read | 764 |
| files that did not parse | 0 |
| functions, all | 12314 |
| of these: inline (callbacks and arrow functions without a name; part of the function around them) | 1604 |
| of these: named functions | 10710 |
| applicable functions (named, X_ITE's own engine code) | 5939 |

## All functions by scope

| scope | functions | named |
| --- | --- | --- |
| engine | 7175 | 5939 |
| shader-source | 62 | 50 |
| site-or-harness | 34 | 10 |
| third-party | 5043 | 4711 |

Only `engine` and `component-module` functions are applicable. Third-party libraries (`lib/ammojs`, `lib/nurbs`, `lib/libtess.js`, `assets/lib`), shader source strings and the site files are listed in the JSON and CSV with the status `NOT_APPLICABLE`.

## Applicable functions by status

| status | functions | share |
| --- | --- | --- |
| IMPLEMENTED_AND_VERIFIED | 0 | 0.0% |
| IMPLEMENTED_NOT_VERIFIED | 0 | 0.0% |
| IMPLEMENTED_DIFFERENT | 0 | 0.0% |
| PARTIALLY_IMPLEMENTED | 0 | 0.0% |
| MISSING_IMPLEMENTATION | 0 | 0.0% |
| REPLACED_BY_RUST_DESIGN | 0 | 0.0% |
| PLATFORM_SPECIFIC | 0 | 0.0% |
| NOT_APPLICABLE | 0 | 0.0% |
| UNRESOLVED_MAPPING | 5939 | 100.0% |

This table counts inspected dispositions. It is not a measure of how much of X_ITE the Rust engine implements: the engine's behavior is measured by the 346-entry ledger and its tests.

## Applicable functions and the 346-entry ledger

| item | functions |
| --- | --- |
| linked to an entry by the entry's upstream file | 2732 |
| no entry lists the file | 3207 |

The 346-entry ledger (`ledger/ledger.json`, 346 entries) is read only here. Its statuses are not copied into this inventory.

## By area of the X_ITE source (applicable functions)

| area | functions | unresolved | verified | not verified | different | partial | missing | no Rust function needed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `lib` | 11 | 11 | 0 | 0 | 0 | 0 | 0 | 0 |
| `standard/Geospatial` | 9 | 9 | 0 | 0 | 0 | 0 | 0 | 0 |
| `standard/Math` | 452 | 452 | 0 | 0 | 0 | 0 | 0 | 0 |
| `standard/Time` | 4 | 4 | 0 | 0 | 0 | 0 | 0 | 0 |
| `standard/Utility` | 26 | 26 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite` | 25 | 25 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Base` | 295 | 295 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Core` | 124 | 124 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/CubeMapTexturing` | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/DOMIntegration.js` | 16 | 16 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/EnvironmentalEffects` | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Followers` | 16 | 16 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/GaussianSplats` | 24 | 24 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Geometry2D` | 26 | 26 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Geometry3D` | 65 | 65 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Geospatial` | 13 | 13 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Grouping` | 5 | 5 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Interpolation` | 21 | 21 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/KeyDeviceSensor` | 8 | 8 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Layering` | 3 | 3 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Layout` | 8 | 8 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Legacy.js` | 5 | 5 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Lighting` | 9 | 9 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/NURBS` | 9 | 9 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Navigation` | 103 | 103 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Networking` | 23 | 23 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/ParticleSystems` | 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Picking` | 18 | 18 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/PointingDeviceSensor` | 40 | 40 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Rendering` | 59 | 59 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Scripting` | 4 | 4 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Shaders` | 19 | 19 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Shape` | 12 | 12 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Sound` | 27 | 27 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Text` | 36 | 36 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Texturing` | 51 | 51 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Texturing3D` | 52 | 52 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/Time` | 5 | 5 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/VolumeRendering` | 10 | 10 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/WebXR` | 16 | 16 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/X3DBrowser.js` | 79 | 79 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/X3DBrowserContext.js` | 27 | 27 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Browser/X_ITE` | 10 | 10 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Annotation` | 17 | 17 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/CADGeometry` | 31 | 31 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Core` | 129 | 129 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/CubeMapTexturing` | 36 | 36 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/DIS` | 18 | 18 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/EnvironmentalEffects` | 52 | 52 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/EnvironmentalSensor` | 25 | 25 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/EventUtilities` | 30 | 30 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Followers` | 94 | 94 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/GaussianSplats` | 7 | 7 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Geometry2D` | 21 | 21 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Geometry3D` | 30 | 30 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Geospatial` | 79 | 79 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Grouping` | 71 | 71 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/HAnim` | 74 | 74 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Interpolation` | 56 | 56 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/KeyDeviceSensor` | 15 | 15 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Layering` | 54 | 54 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Layout` | 41 | 41 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Lighting` | 71 | 71 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/NURBS` | 94 | 94 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Navigation` | 130 | 130 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Networking` | 71 | 71 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/ParticleSystems` | 96 | 96 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Picking` | 40 | 40 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/PointingDeviceSensor` | 40 | 40 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Rendering` | 189 | 189 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/RigidBodyPhysics` | 132 | 132 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Scripting` | 24 | 24 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Shaders` | 93 | 93 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Shape` | 195 | 195 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Sound` | 175 | 175 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Text` | 31 | 31 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/TextureProjection` | 43 | 43 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Texturing` | 147 | 147 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Texturing3D` | 56 | 56 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/Time` | 42 | 42 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/VolumeRendering` | 113 | 113 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Components/X_ITE` | 162 | 162 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Configuration` | 27 | 27 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Execution` | 161 | 161 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Fields` | 310 | 310 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/InputOutput` | 80 | 80 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Parser` | 598 | 598 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Prototype` | 40 | 40 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Rendering` | 105 | 105 | 0 | 0 | 0 | 0 | 0 | 0 |
| `x_ite/Routing` | 23 | 23 | 0 | 0 | 0 | 0 | 0 | 0 |

## REA evidence

Static analysis of the whole pinned source (`analyze_javascript_application`, detail `summary`): parent `ev_4b19e25b68ef315c508675fd4b3f73b3b165891f5c683cafe92b06a6e1e87313`, summary `ev_304fc670a11afcb98229572336aae2872b97e3f40960f15a63a9d2945bd1873a`. REA parsed 765 JavaScript files (1793281 AST nodes, 0 parse failures); its graph is truncated and partial, it executed nothing, and it did not analyze Rust.

REA is the independent check of the parse (file count, no parse failure). The function list itself comes from the extractor in `tools/`, which is deterministic.
