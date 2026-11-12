# Function coverage summary

Upstream: X_ITE 16.5.0 @ 34cdf9f97 (`oracle/x_ite-pin/src`). Generator: `ledger/functions/tools/` (extractor and builder; the rules are in the
builder's module documentation). This file is generated: do not edit it.

`mapping` is code presence only: `cited` (a Rust source line names the X_ITE function), `name` (a Rust function of the same name
in a file or `impl` of the same class; a candidate), `class` (a constructor whose class has Rust files in the 346-entry ledger), `none` (no symbol found by these rules; not proof of absence). `tests` is
separate: `direct` (a Rust test names the function), `entry` (its entry of the 346-entry ledger is COMPLETE with tests), `none`.

## Counts

| item | count |
| --- | --- |
| JavaScript files read | 764 |
| files that did not parse | 0 |
| functions, all | 12314 |
| of these: inline (callbacks and arrow functions without a name; part of the function around them) | 1604 |
| of these: named functions | 10710 |
| applicable functions (named, X_ITE's own engine code) | 5939 |
| Rust functions indexed (crates/*/src, test modules left out) | 6735 |

## All functions by scope

| scope | functions | named |
| --- | --- | --- |
| engine | 7175 | 5939 |
| shader-source | 62 | 50 |
| site-or-harness | 34 | 10 |
| third-party | 5043 | 4711 |

Only `engine` and `component-module` functions are applicable. Third-party libraries (`lib/ammojs`, `lib/nurbs`, `lib/libtess.js`, `assets/lib`), shader source strings and the site files are listed in the JSON and CSV, and are not mapped.

## Applicable functions: code presence

| mapping | functions | share |
| --- | --- | --- |
| cited | 1303 | 21.9% |
| name | 406 | 6.8% |
| class | 195 | 3.3% |
| none | 4035 | 67.9% |

`cited` is the only verified mapping. `name` is a candidate from the name and the class, and `class` says only that the class has Rust files. `none` is not a statement that the behavior is absent.

## Applicable functions: tested behavior

| tests | functions | share |
| --- | --- | --- |
| direct | 108 | 1.8% |
| entry | 2008 | 33.8% |
| none | 3823 | 64.4% |

## Applicable functions and the 346-entry ledger

| item | functions |
| --- | --- |
| linked to an entry by the entry's upstream file | 2732 |
| no entry lists the file | 3207 |
| linked entry is DIFFERENT (approved difference) | 123 |
| linked entry is PARTIAL (open gap) | 571 |

The 346-entry ledger itself: 289 COMPLETE / 36 PARTIAL / 21 DIFFERENT / 0 MISSING — total 346. It is read only here.

## By area of the X_ITE source (applicable functions)

| area | functions | cited | name | class | none | tests direct | tests entry | tests none |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `lib` | 11 | 0 | 0 | 0 | 11 | 0 | 0 | 11 |
| `standard/Geospatial` | 9 | 0 | 5 | 0 | 4 | 0 | 0 | 9 |
| `standard/Math` | 452 | 112 | 88 | 0 | 252 | 4 | 0 | 448 |
| `standard/Time` | 4 | 2 | 0 | 0 | 2 | 0 | 0 | 4 |
| `standard/Utility` | 26 | 0 | 0 | 0 | 26 | 0 | 0 | 26 |
| `x_ite` | 25 | 0 | 0 | 0 | 25 | 0 | 0 | 25 |
| `x_ite/Base` | 295 | 69 | 5 | 0 | 221 | 12 | 0 | 283 |
| `x_ite/Browser/Core` | 124 | 19 | 4 | 1 | 100 | 2 | 27 | 95 |
| `x_ite/Browser/CubeMapTexturing` | 2 | 0 | 0 | 0 | 2 | 0 | 0 | 2 |
| `x_ite/Browser/DOMIntegration.js` | 16 | 3 | 0 | 0 | 13 | 0 | 0 | 16 |
| `x_ite/Browser/EnvironmentalEffects` | 2 | 0 | 0 | 0 | 2 | 0 | 0 | 2 |
| `x_ite/Browser/Followers` | 16 | 2 | 0 | 0 | 14 | 0 | 0 | 16 |
| `x_ite/Browser/GaussianSplats` | 24 | 4 | 0 | 0 | 20 | 2 | 0 | 22 |
| `x_ite/Browser/Geometry2D` | 26 | 2 | 0 | 0 | 24 | 0 | 0 | 26 |
| `x_ite/Browser/Geometry3D` | 65 | 10 | 0 | 0 | 55 | 0 | 0 | 65 |
| `x_ite/Browser/Geospatial` | 13 | 2 | 0 | 0 | 11 | 0 | 0 | 13 |
| `x_ite/Browser/Grouping` | 5 | 4 | 0 | 0 | 1 | 0 | 0 | 5 |
| `x_ite/Browser/Interpolation` | 21 | 2 | 0 | 0 | 19 | 0 | 0 | 21 |
| `x_ite/Browser/KeyDeviceSensor` | 8 | 0 | 0 | 0 | 8 | 0 | 0 | 8 |
| `x_ite/Browser/Layering` | 3 | 0 | 0 | 0 | 3 | 0 | 0 | 3 |
| `x_ite/Browser/Layout` | 8 | 5 | 0 | 0 | 3 | 0 | 0 | 8 |
| `x_ite/Browser/Legacy.js` | 5 | 0 | 0 | 0 | 5 | 0 | 0 | 5 |
| `x_ite/Browser/Lighting` | 9 | 3 | 0 | 0 | 6 | 0 | 0 | 9 |
| `x_ite/Browser/NURBS` | 9 | 4 | 1 | 0 | 4 | 0 | 0 | 9 |
| `x_ite/Browser/Navigation` | 103 | 23 | 11 | 0 | 69 | 2 | 0 | 101 |
| `x_ite/Browser/Networking` | 23 | 3 | 0 | 0 | 20 | 0 | 0 | 23 |
| `x_ite/Browser/ParticleSystems` | 2 | 0 | 0 | 0 | 2 | 0 | 0 | 2 |
| `x_ite/Browser/Picking` | 18 | 6 | 0 | 0 | 12 | 2 | 0 | 16 |
| `x_ite/Browser/PointingDeviceSensor` | 40 | 16 | 0 | 0 | 24 | 3 | 0 | 37 |
| `x_ite/Browser/Rendering` | 59 | 4 | 0 | 0 | 55 | 1 | 0 | 58 |
| `x_ite/Browser/Scripting` | 4 | 0 | 0 | 0 | 4 | 0 | 0 | 4 |
| `x_ite/Browser/Shaders` | 19 | 6 | 0 | 0 | 13 | 2 | 0 | 17 |
| `x_ite/Browser/Shape` | 12 | 4 | 0 | 0 | 8 | 0 | 0 | 12 |
| `x_ite/Browser/Sound` | 27 | 7 | 4 | 0 | 16 | 0 | 0 | 27 |
| `x_ite/Browser/Text` | 36 | 16 | 0 | 0 | 20 | 0 | 0 | 36 |
| `x_ite/Browser/Texturing` | 51 | 3 | 0 | 0 | 48 | 0 | 0 | 51 |
| `x_ite/Browser/Texturing3D` | 52 | 3 | 0 | 0 | 49 | 0 | 0 | 52 |
| `x_ite/Browser/Time` | 5 | 2 | 0 | 0 | 3 | 0 | 0 | 5 |
| `x_ite/Browser/VolumeRendering` | 10 | 3 | 0 | 0 | 7 | 0 | 0 | 10 |
| `x_ite/Browser/WebXR` | 16 | 7 | 0 | 0 | 9 | 2 | 0 | 14 |
| `x_ite/Browser/X3DBrowser.js` | 79 | 25 | 0 | 0 | 54 | 3 | 76 | 0 |
| `x_ite/Browser/X3DBrowserContext.js` | 27 | 2 | 0 | 0 | 25 | 0 | 0 | 27 |
| `x_ite/Browser/X_ITE` | 10 | 3 | 0 | 0 | 7 | 0 | 0 | 10 |
| `x_ite/Components/Annotation` | 17 | 0 | 0 | 6 | 11 | 0 | 0 | 17 |
| `x_ite/Components/CADGeometry` | 31 | 3 | 2 | 5 | 21 | 0 | 30 | 1 |
| `x_ite/Components/Core` | 129 | 38 | 0 | 6 | 85 | 0 | 31 | 98 |
| `x_ite/Components/CubeMapTexturing` | 36 | 15 | 2 | 0 | 19 | 0 | 26 | 10 |
| `x_ite/Components/DIS` | 18 | 0 | 0 | 6 | 12 | 0 | 18 | 0 |
| `x_ite/Components/EnvironmentalEffects` | 52 | 14 | 1 | 3 | 34 | 0 | 22 | 30 |
| `x_ite/Components/EnvironmentalSensor` | 25 | 10 | 1 | 0 | 14 | 0 | 17 | 8 |
| `x_ite/Components/EventUtilities` | 30 | 0 | 0 | 7 | 23 | 0 | 23 | 7 |
| `x_ite/Components/Followers` | 94 | 10 | 17 | 14 | 53 | 0 | 58 | 36 |
| `x_ite/Components/GaussianSplats` | 7 | 2 | 0 | 0 | 5 | 0 | 0 | 7 |
| `x_ite/Components/Geometry2D` | 21 | 11 | 0 | 3 | 7 | 0 | 21 | 0 |
| `x_ite/Components/Geometry3D` | 30 | 9 | 0 | 6 | 15 | 0 | 30 | 0 |
| `x_ite/Components/Geospatial` | 79 | 13 | 1 | 7 | 58 | 0 | 59 | 20 |
| `x_ite/Components/Grouping` | 71 | 13 | 4 | 2 | 52 | 0 | 20 | 51 |
| `x_ite/Components/HAnim` | 74 | 12 | 5 | 4 | 53 | 0 | 74 | 0 |
| `x_ite/Components/Interpolation` | 56 | 7 | 7 | 12 | 30 | 0 | 39 | 17 |
| `x_ite/Components/KeyDeviceSensor` | 15 | 4 | 0 | 2 | 9 | 0 | 6 | 9 |
| `x_ite/Components/Layering` | 54 | 21 | 3 | 1 | 29 | 2 | 18 | 34 |
| `x_ite/Components/Layout` | 41 | 9 | 2 | 1 | 29 | 0 | 40 | 1 |
| `x_ite/Components/Lighting` | 71 | 14 | 3 | 2 | 52 | 0 | 52 | 19 |
| `x_ite/Components/NURBS` | 94 | 19 | 4 | 7 | 64 | 0 | 9 | 85 |
| `x_ite/Components/Navigation` | 130 | 51 | 9 | 4 | 66 | 2 | 83 | 45 |
| `x_ite/Components/Networking` | 71 | 35 | 2 | 2 | 32 | 6 | 32 | 33 |
| `x_ite/Components/ParticleSystems` | 96 | 19 | 3 | 7 | 67 | 5 | 24 | 67 |
| `x_ite/Components/Picking` | 40 | 7 | 3 | 4 | 26 | 0 | 17 | 23 |
| `x_ite/Components/PointingDeviceSensor` | 40 | 4 | 7 | 4 | 25 | 0 | 24 | 16 |
| `x_ite/Components/Rendering` | 189 | 50 | 6 | 8 | 125 | 2 | 81 | 106 |
| `x_ite/Components/RigidBodyPhysics` | 132 | 8 | 1 | 12 | 111 | 4 | 81 | 47 |
| `x_ite/Components/Scripting` | 24 | 9 | 2 | 1 | 12 | 1 | 0 | 23 |
| `x_ite/Components/Shaders` | 93 | 17 | 0 | 6 | 70 | 4 | 35 | 54 |
| `x_ite/Components/Shape` | 195 | 54 | 7 | 3 | 131 | 5 | 126 | 64 |
| `x_ite/Components/Sound` | 175 | 50 | 8 | 13 | 104 | 6 | 70 | 99 |
| `x_ite/Components/Text` | 31 | 16 | 0 | 2 | 13 | 2 | 14 | 15 |
| `x_ite/Components/TextureProjection` | 43 | 5 | 0 | 2 | 36 | 0 | 26 | 17 |
| `x_ite/Components/Texturing` | 147 | 45 | 3 | 5 | 94 | 2 | 74 | 71 |
| `x_ite/Components/Texturing3D` | 56 | 18 | 1 | 4 | 33 | 0 | 43 | 13 |
| `x_ite/Components/Time` | 42 | 22 | 1 | 0 | 19 | 0 | 13 | 29 |
| `x_ite/Components/VolumeRendering` | 113 | 10 | 11 | 11 | 81 | 0 | 67 | 46 |
| `x_ite/Components/X_ITE` | 162 | 17 | 2 | 11 | 132 | 3 | 155 | 4 |
| `x_ite/Configuration` | 27 | 6 | 0 | 0 | 21 | 0 | 0 | 27 |
| `x_ite/Execution` | 161 | 89 | 1 | 0 | 71 | 2 | 47 | 112 |
| `x_ite/Fields` | 310 | 17 | 1 | 0 | 292 | 0 | 0 | 310 |
| `x_ite/InputOutput` | 80 | 37 | 13 | 0 | 30 | 9 | 0 | 71 |
| `x_ite/Parser` | 598 | 72 | 155 | 1 | 370 | 11 | 300 | 287 |
| `x_ite/Prototype` | 40 | 16 | 0 | 0 | 24 | 2 | 0 | 38 |
| `x_ite/Rendering` | 105 | 19 | 0 | 0 | 86 | 5 | 0 | 100 |
| `x_ite/Routing` | 23 | 10 | 0 | 0 | 13 | 0 | 0 | 23 |

## REA evidence

Static analysis of the whole pinned source (`analyze_javascript_application`, detail `summary`): parent `ev_4b19e25b68ef315c508675fd4b3f73b3b165891f5c683cafe92b06a6e1e87313`, summary `ev_304fc670a11afcb98229572336aae2872b97e3f40960f15a63a9d2945bd1873a`. REA parsed 765 JavaScript files (1793281 AST nodes, 0 parse failures); its graph is truncated and partial, it executed nothing, and it did not analyze Rust.

REA is the independent check of the parse (file count, no parse failure). The function list itself comes from the extractor in `tools/`, which is deterministic; the Rust mapping comes from the Rust sources and is not a REA result.
