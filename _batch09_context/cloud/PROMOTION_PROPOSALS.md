# REA promotion proposals (cloud, checkpoint after 804802d)

No status is changed by this file. Each proposal needs the owner's decision by exact REA ID; the 33 nodes also need the desktop GPU
run first.

## Ready for decision now

| REA ID | proposed | evidence (all exact against pinned X_ITE, 3 identical recordings) |
| --- | --- | --- |
| `sub:proto.proto` | COMPLETE | corpus-exact; the SAI proto API the entry names as missing is ported and compared: createProto, add/update/removeProtoDeclaration, add/updateExternProtoDeclaration (scx-api, sai-context, ctxev-scene), declaration printing and getProtoDeclaration (scx-print). The entry's `missing` text predates them. |
| `sub:fields.math` | COMPLETE (owner call) | every standard/Math/Geometry and Algorithms operation the port has is bit-exact on 3567 cases (geom_math.rs); left: Triangle3.triangulatePolygon (libtess) compared only through Text geometry in x3d-text, toString methods not ported. |

## The 33 nodes (PARTIAL since this round: field and event evidence exact; visual evidence needed)

Cloud evidence: the node-* worlds (field definitions, created text, every settable field's events on a live node). GPU evidence: the
existing image cases below render the node and must pass on the RTX 3060 Ti at the tested commit; nodes without one need a world.

| REA ID | node-* world | existing image cases that render it |
| --- | --- | --- |
| `node:Appearance`, `node:Material`, `node:Shape` | node-shape | > 240 cases (every lit-shape world) |
| `node:DirectionalLight` | node-lighting | 137 |
| `node:PointLight` | node-lighting | 31 (ctx-runtime, disp-runtime ...) |
| `node:SpotLight` | node-lighting | 4 (pbr-runtime, pbr-shading-flat) |
| `node:Coordinate` | node-rendering | 52 |
| `node:IndexedLineSet` | node-rendering | 14 (picking-groups, picking-layers ...) |
| `node:PointSet` | node-rendering | 12 (shading-flat, shading-phong) |
| `node:Color`, `node:ColorRGBA`, `node:Normal`, `node:CoordinateDouble` | node-rendering | none found by name: a GPU world is needed |
| `node:ImageTexture` | node-texturing | 43 |
| `node:PixelTexture` | node-texturing | 29 |
| `node:TextureTransform` | node-texturing | 5 (multitex-runtime, pbr-shading-flat) |
| `node:MovieTexture` | node-texturing | none: a GPU world is needed |
| `node:TextureCoordinate3D`, `node:TextureCoordinate4D` | node-texturing3d | 2 (tex3d-composed) |
| `node:Text` | node-text, text-events, text-fontstyle, text-family | 63; open: warm-up font timing, ScreenText |
| `node:FontStyle` | node-text, text-* | 19 |
| `node:OrthoViewpoint` | node-navigation | 2 (viewpoint-runtime) |
| `node:LocalFog` | node-environment | 2 (fog-runtime) |
| `node:TextureBackground` | node-environment | 2 (background-runtime) |
| `node:HAnimJoint`, `node:HAnimSegment` | node-hanim | 6 (hanim-humanoid, hanim-sites-33) |
| `node:HAnimSite` | node-hanim | 4 (hanim-sites-33, -41) |
| `node:LayoutGroup`, `node:ScreenFontStyle`, `node:ScreenGroup` | node-layout | none: a GPU world is needed |
| `node:AudioClip`, `node:Sound` | node-sound | audio evidence rather than images (the audio suites) |
| `node:EspduTransform` | node-dis | none (network node: the UDP suite, desktop) |

Image-case counts come from the world files of every baseline with PNGs (319 cases); a node named only inside a PROTO or an Inline
is not counted.
