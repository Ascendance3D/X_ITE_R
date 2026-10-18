# Port Factory backlog

Generated 2026-10-17T01:00:04-04:00 from `/home/ryan/Projects/cybertown/.reports/xite-rust-full-port-rea-completion/ledger/ledger.json` by `.port-factory/tools/sync.py`. Do not hand-edit; change statuses with the REA `set_status.py`.

Coverage: 133/346 COMPLETE = 38.4% (PARTIAL does not count).

## Ordering rule

1. Ready before blocked (blocked = depends on a missing engine subsystem such as `sub:sound.engine`, or has a recorded blocker).
2. Status: NOT_TESTED, then PARTIAL, then DIFFERENT, then MISSING (cheapest to finish first).
3. Components with fewer open items first (small wins, fast integration).
4. Nodes before subsystems, then id order.

Priority numbers below are global ranks. Active: none.

## Lighting (3 open)

- [ ] #1 `node:DirectionalLight` NOT_TESTED - see behavior
- [ ] #3 `node:PointLight` NOT_TESTED - see behavior
- [ ] #6 `node:SpotLight` NOT_TESTED - see behavior

## Layout (3 open)

- [ ] #2 `node:LayoutGroup` NOT_TESTED - runtime layout/viewport field change not compared
- [ ] #4 `node:ScreenFontStyle` NOT_TESTED - see behavior
- [ ] #5 `node:ScreenGroup` NOT_TESTED - see behavior

## Shape (4 open)

- [ ] #7 `node:Appearance` NOT_TESTED - see behavior
- [ ] #9 `node:Material` NOT_TESTED - see behavior
- [ ] #10 `node:Shape` NOT_TESTED - see behavior
- [ ] #92 `node:AcousticProperties` MISSING - see behavior

## Text (4 open)

- [ ] #8 `node:FontStyle` NOT_TESTED - see behavior
- [ ] #11 `node:Text` NOT_TESTED - see behavior
- [ ] #47 `sub:text.fonts` PARTIAL
- [ ] #93 `node:FontLibrary` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)

## EnvironmentalEffects (5 open)

- [ ] #12 `node:LocalFog` NOT_TESTED - see behavior
- [ ] #13 `node:TextureBackground` NOT_TESTED - see behavior
- [ ] #48 `node:Background` PARTIAL - only binding / static load / children paths oracle-tested here; render and navigation detail covered by GPU and navigation suites but not item-by-item; Collision collideTime path and Billboard orientation not exercised; Anchor click/load not exercised
- [ ] #49 `node:Fog` PARTIAL - only binding / static load / children paths oracle-tested here; render and navigation detail covered by GPU and navigation suites but not item-by-item; Collision collideTime path and Billboard orientation not exercised; Anchor click/load not exercised
- [ ] #50 `node:FogCoordinate` PARTIAL - renderer does not consume them yet: fog depth in the fog shader, tangents (normal maps), channels 1-3 (MultiTexture), generator modes, vertex attributes (needs ComposedShader)

## DIS (6 open)

- [ ] #14 `node:EspduTransform` NOT_TESTED - see behavior
- [ ] #100 `node:DISEntityManager` MISSING - see behavior
- [ ] #101 `node:DISEntityTypeMapping` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)
- [ ] #107 `node:ReceiverPdu` MISSING - see behavior
- [ ] #108 `node:SignalPdu` MISSING - see behavior
- [ ] #110 `node:TransmitterPdu` MISSING - see behavior

## Texturing3D (6 open)

- [ ] #15 `node:TextureCoordinate3D` NOT_TESTED - see behavior
- [ ] #16 `node:TextureCoordinate4D` NOT_TESTED - see behavior
- [ ] #99 `node:ComposedTexture3D` MISSING - see behavior
- [ ] #104 `node:ImageTexture3D` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)
- [ ] #105 `node:ImageTextureAtlas` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)
- [ ] #106 `node:PixelTexture3D` MISSING - see behavior

## HAnim (7 open)

- [ ] #17 `node:HAnimJoint` NOT_TESTED - see behavior
- [ ] #18 `node:HAnimSegment` NOT_TESTED - see behavior
- [ ] #19 `node:HAnimSite` NOT_TESTED - see behavior
- [ ] #116 `node:HAnimDisplacer` MISSING - see behavior
- [ ] #117 `node:HAnimHumanoid` MISSING - see behavior
- [ ] #118 `node:HAnimMotion` MISSING - see behavior
- [ ] #119 `node:HAnimPose` MISSING - see behavior

## Texturing (8 open)

- [ ] #20 `node:ImageTexture` NOT_TESTED - see behavior
- [ ] #21 `node:MovieTexture` NOT_TESTED - see behavior
- [ ] #22 `node:PixelTexture` NOT_TESTED - see behavior
- [ ] #23 `node:TextureTransform` NOT_TESTED - see behavior
- [ ] #58 `node:MultiTextureCoordinate` PARTIAL - renderer does not consume them yet: fog depth in the fog shader, tangents (normal maps), channels 1-3 (MultiTexture), generator modes, vertex attributes (needs ComposedShader)
- [ ] #59 `node:TextureCoordinate` PARTIAL - mapping field (multi-texture mapping) not used by the renderer
- [ ] #60 `node:TextureCoordinateGenerator` PARTIAL - renderer does not consume them yet: fog depth in the fog shader, tangents (normal maps), channels 1-3 (MultiTexture), generator modes, vertex attributes (needs ComposedShader)
- [ ] #123 `node:RenderedTexture` MISSING - see behavior

## ParticleSystems (10 open)

- [ ] #24 `node:ParticleSystem` NOT_TESTED - see behavior
- [ ] #126 `node:BoundedPhysicsModel` MISSING - see behavior
- [ ] #127 `node:ConeEmitter` MISSING - see behavior
- [ ] #128 `node:ExplosionEmitter` MISSING - see behavior
- [ ] #129 `node:ForcePhysicsModel` MISSING - see behavior
- [ ] #130 `node:PointEmitter` MISSING - see behavior
- [ ] #131 `node:PolylineEmitter` MISSING - see behavior
- [ ] #132 `node:SurfaceEmitter` MISSING - see behavior
- [ ] #133 `node:VolumeEmitter` MISSING - see behavior
- [ ] #134 `node:WindPhysicsModel` MISSING - see behavior

## Navigation (12 open)

- [ ] #25 `node:OrthoViewpoint` NOT_TESTED - see behavior
- [ ] #61 `node:Billboard` PARTIAL - only binding / static load / children paths oracle-tested here; render and navigation detail covered by GPU and navigation suites but not item-by-item; Collision collideTime path and Billboard orientation not exercised; Anchor click/load not exercised
- [ ] #62 `node:Collision` PARTIAL - only binding / static load / children paths oracle-tested here; render and navigation detail covered by GPU and navigation suites but not item-by-item; Collision collideTime path and Billboard orientation not exercised; Anchor click/load not exercised
- [ ] #63 `node:Viewpoint` PARTIAL - only binding / static load / children paths oracle-tested here; render and navigation detail covered by GPU and navigation suites but not item-by-item; Collision collideTime path and Billboard orientation not exercised; Anchor click/load not exercised
- [ ] #64 `sub:nav.collision-gravity` PARTIAL
- [ ] #65 `sub:nav.examine` PARTIAL
- [ ] #66 `sub:nav.lookat` PARTIAL
- [ ] #67 `sub:nav.none` PARTIAL
- [ ] #68 `sub:nav.plane` PARTIAL
- [ ] #69 `sub:nav.transitions` PARTIAL
- [ ] #70 `sub:nav.walk-fly` PARTIAL
- [ ] #135 `node:ViewpointGroup` MISSING - see behavior

## Rendering (14 open)

- [ ] #26 `node:Color` NOT_TESTED - see behavior
- [ ] #27 `node:ColorRGBA` NOT_TESTED - see behavior
- [ ] #28 `node:Coordinate` NOT_TESTED - see behavior
- [ ] #29 `node:CoordinateDouble` NOT_TESTED - see behavior
- [ ] #30 `node:IndexedLineSet` NOT_TESTED - see behavior
- [ ] #31 `node:Normal` NOT_TESTED - see behavior
- [ ] #32 `node:PointSet` NOT_TESTED - see behavior
- [ ] #71 `node:Tangent` PARTIAL - renderer does not consume them yet: fog depth in the fog shader, tangents (normal maps), channels 1-3 (MultiTexture), generator modes, vertex attributes (needs ComposedShader)
- [ ] #72 `sub:render.core` PARTIAL
- [ ] #73 `sub:render.fog` PARTIAL - LocalFog, FogCoordinate
- [ ] #74 `sub:render.headlight` PARTIAL
- [ ] #75 `sub:render.lighting-model` PARTIAL
- [ ] #76 `sub:render.transparency-sorting` PARTIAL
- [ ] #176 `sub:render.shadows` MISSING

## Core (1 open)

- [ ] #33 `node:MetadataSet` PARTIAL - X3DNode getMetaData/setMetaData helpers (X_ITE internal metadata API)

## Scripting (1 open)

- [ ] #34 `node:Script` PARTIAL - SCRIPT_SHUTDOWN_ON_REMOVAL, JS_ONLY_NODE_REFERENCE_LIFETIME, addUserDefinedField/removeUserDefinedField reload

## Configuration (1 open)

- [ ] #35 `sub:config.components-profiles` PARTIAL - ComponentInfo/ProfileInfo title and providerURL (data not in trimmed node table)

## InputOutput (1 open)

- [ ] #36 `sub:io.file-loader` PARTIAL - data: URLs, non-VRML formats, fetch error messages

## Geospatial (2 open)

- [ ] #37 `node:GeoTouchSensor` PARTIAL - Own behavior done (hitGeoCoord_changed = getGeoCoord(hitPoint), GD+UTM+GeoOrigin). Oracle still fails from 2 inherited pointer.picking gaps, same with plain TouchSensor: hit point differs ~1e-5 (float precision), and X_ITE re-sends hitPoint/hitNormal/hitTexCoord on mousedown and mouseup at the same position; Rust does not.
- [ ] #85 `node:GeoMetadata` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)

## Fields (2 open)

- [ ] #38 `sub:fields.math` PARTIAL - Box2/Box3/Line3/Plane3/Triangle/ViewVolume/Spline numerics not all ported
- [ ] #39 `sub:fields.types` PARTIAL - value model complete for parse/print; JS field object API per type not fully oracle-tested

## Prototype (2 open)

- [ ] #40 `sub:proto.externproto` PARTIAL - cached EXTERNPROTO live-state parity; disposed-context cancel is a deliberate difference
- [ ] #41 `sub:proto.proto` PARTIAL - corpus-exact; SAI proto API (createProto, field updates on declaration) not ported

## Networking (3 open)

- [ ] #42 `node:Anchor` PARTIAL - only binding / static load / children paths oracle-tested here; render and navigation detail covered by GPU and navigation suites but not item-by-item; Collision collideTime path and Billboard orientation not exercised; Anchor click/load not exercised
- [ ] #43 `node:Inline` PARTIAL - known differences: replaced PROTO body closes nested Inlines; EmbeddedViewer rule
- [ ] #88 `node:InlineGeometry` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)

## Execution (3 open)

- [ ] #44 `sub:exec.bindable-stacks` PARTIAL - isBound/bindTime events per layer verified only for Viewpoint/Background corpus
- [ ] #45 `sub:exec.context-api` PARTIAL - addImportedNode/updateImportedNode/removeImportedNode, createX3DFromString returns the current context, change events (namedNodes_changed etc.)
- [ ] #46 `sub:exec.import-export` PARTIAL - SAI addImportedNode/updateImportedNode/removeImportedNode via Script not oracle-tested

## Browser (5 open)

- [ ] #51 `sub:browser.sai-api` PARTIAL - createX3DFromString, createX3DFromURL, loadURL, replaceWorld, importDocument, importJS, getBrowserOption/setBrowserOption/getBrowserProperty, currentSpeed/currentFrameRate, changeViewpoint, getRenderingProperty, addBrowserCallback, beginUpdate/endUpdate, viewAll
- [ ] #83 `sub:browser.dom-integration` DIFFERENT
- [ ] #94 `sub:browser.keyboard-ui` MISSING
- [ ] #95 `sub:browser.multi-viewer-thread` MISSING - several EmbeddedViewer instances on one thread
- [ ] #96 `sub:browser.webxr` MISSING

## PointingDeviceSensor (5 open)

- [ ] #52 `sub:pointer.picking` PARTIAL - hit point/normal/texCoord differ ~1e-5 from X_ITE; X_ITE re-sends hit events on mousedown/mouseup at the same position (Rust skips them). Shown by oracle/synthetic-rea/geo-touch.wrl
- [ ] #79 `node:CylinderSensor` DIFFERENT
- [ ] #80 `node:PlaneSensor` DIFFERENT
- [ ] #81 `node:SphereSensor` DIFFERENT
- [ ] #82 `node:TouchSensor` DIFFERENT

## Parser (6 open)

- [ ] #53 `sub:parse.golden-gate` PARTIAL - dispatches VRML only
- [ ] #54 `sub:parse.vrml-classic` PARTIAL - corpus-exact; X3D classic edge cases not separately oracle-tested
- [ ] #112 `sub:parse.glb2` MISSING
- [ ] #113 `sub:parse.gltf2` MISSING
- [ ] #114 `sub:parse.html-support` MISSING
- [ ] #115 `sub:parse.svg` MISSING

## Shaders (8 open)

- [ ] #55 `node:FloatVertexAttribute` PARTIAL - renderer does not consume them yet: fog depth in the fog shader, tangents (normal maps), channels 1-3 (MultiTexture), generator modes, vertex attributes (needs ComposedShader)
- [ ] #56 `node:Matrix3VertexAttribute` PARTIAL - renderer does not consume them yet: fog depth in the fog shader, tangents (normal maps), channels 1-3 (MultiTexture), generator modes, vertex attributes (needs ComposedShader)
- [ ] #57 `node:Matrix4VertexAttribute` PARTIAL - renderer does not consume them yet: fog depth in the fog shader, tangents (normal maps), channels 1-3 (MultiTexture), generator modes, vertex attributes (needs ComposedShader)
- [ ] #120 `node:ComposedShader` MISSING - see behavior
- [ ] #121 `node:PackagedShader` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)
- [ ] #122 `node:ProgramShader` MISSING - see behavior
- [ ] #124 `node:ShaderPart` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)
- [ ] #125 `node:ShaderProgram` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)

## Interpolation (2 open)

- [ ] #77 `node:SplinePositionInterpolator` DIFFERENT
- [ ] #78 `node:SplinePositionInterpolator2D` DIFFERENT

## GaussianSplats (1 open)

- [ ] #84 `node:GaussianSplats` MISSING - see behavior

## TextureProjection (2 open)

- [ ] #86 `node:TextureProjector` MISSING - see behavior
- [ ] #87 `node:TextureProjectorParallel` MISSING - see behavior

## Picking (3 open)

- [ ] #89 `node:PointPickSensor` MISSING - needs a physics engine (Ammo in X_ITE); not ported
- [ ] #90 `node:PrimitivePickSensor` MISSING - needs a physics engine (Ammo in X_ITE); not ported
- [ ] #91 `node:VolumePickSensor` MISSING - needs a physics engine (Ammo in X_ITE); not ported

## Annotation (6 open)

- [ ] #97 `node:AnnotationLayer` MISSING - see behavior
- [ ] #98 `node:AnnotationTarget` MISSING - see behavior
- [ ] #102 `node:GroupAnnotation` MISSING - see behavior
- [ ] #103 `node:IconAnnotation` MISSING - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)
- [ ] #109 `node:TextAnnotation` MISSING - see behavior
- [ ] #111 `node:URLAnnotation` MISSING - see behavior

## VolumeRendering (13 open)

- [ ] #136 `node:BlendedVolumeStyle` MISSING - see behavior
- [ ] #137 `node:BoundaryEnhancementVolumeStyle` MISSING - see behavior
- [ ] #138 `node:CartoonVolumeStyle` MISSING - see behavior
- [ ] #139 `node:ComposedVolumeStyle` MISSING - see behavior
- [ ] #142 `node:EdgeEnhancementVolumeStyle` MISSING - see behavior
- [ ] #143 `node:IsoSurfaceVolumeData` MISSING - see behavior
- [ ] #155 `node:OpacityMapVolumeStyle` MISSING - see behavior
- [ ] #156 `node:ProjectionVolumeStyle` MISSING - see behavior
- [ ] #157 `node:SegmentedVolumeData` MISSING - see behavior
- [ ] #158 `node:ShadedVolumeStyle` MISSING - see behavior
- [ ] #159 `node:SilhouetteEnhancementVolumeStyle` MISSING - see behavior
- [ ] #160 `node:ToneMappedVolumeStyle` MISSING - see behavior
- [ ] #161 `node:VolumeData` MISSING - see behavior

## NURBS (13 open)

- [ ] #140 `node:Contour2D` MISSING - see behavior
- [ ] #141 `node:ContourPolyline2D` MISSING - see behavior
- [ ] #144 `node:NurbsCurve` MISSING - see behavior
- [ ] #145 `node:NurbsCurve2D` MISSING - see behavior
- [ ] #146 `node:NurbsOrientationInterpolator` MISSING - see behavior
- [ ] #147 `node:NurbsPatchSurface` MISSING - see behavior
- [ ] #148 `node:NurbsPositionInterpolator` MISSING - see behavior
- [ ] #149 `node:NurbsSet` MISSING - see behavior
- [ ] #150 `node:NurbsSurfaceInterpolator` MISSING - see behavior
- [ ] #151 `node:NurbsSweptSurface` MISSING - see behavior
- [ ] #152 `node:NurbsSwungSurface` MISSING - see behavior
- [ ] #153 `node:NurbsTextureCoordinate` MISSING - see behavior
- [ ] #154 `node:NurbsTrimmedSurface` MISSING - see behavior

## X_ITE (14 open)

- [ ] #162 `node:AnisotropyMaterialExtension` MISSING - see behavior
- [ ] #163 `node:ClearcoatMaterialExtension` MISSING - see behavior
- [ ] #164 `node:DiffuseTransmissionMaterialExtension` MISSING - see behavior
- [ ] #165 `node:DispersionMaterialExtension` MISSING - see behavior
- [ ] #166 `node:EmissiveStrengthMaterialExtension` MISSING - see behavior
- [ ] #167 `node:IORMaterialExtension` MISSING - see behavior
- [ ] #168 `node:InstancedShape` MISSING - see behavior
- [ ] #169 `node:IridescenceMaterialExtension` MISSING - see behavior
- [ ] #170 `node:RetroreflectionMaterialExtension` MISSING - see behavior
- [ ] #171 `node:SheenMaterialExtension` MISSING - see behavior
- [ ] #172 `node:SpecularMaterialExtension` MISSING - see behavior
- [ ] #173 `node:TransmissionMaterialExtension` MISSING - see behavior
- [ ] #174 `node:VolumeMaterialExtension` MISSING - see behavior
- [ ] #175 `node:VolumeScatterMaterialExtension` MISSING - see behavior

## RigidBodyPhysics (15 open)

- [ ] #177 `sub:physics.engine` MISSING
- [ ] #181 `node:BallJoint` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #182 `node:CollidableOffset` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #183 `node:CollidableShape` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #184 `node:CollisionCollection` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #185 `node:CollisionSensor` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #186 `node:CollisionSpace` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #187 `node:Contact` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #188 `node:DoubleAxisHingeJoint` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #189 `node:MotorJoint` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #190 `node:RigidBody` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #191 `node:RigidBodyCollection` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #192 `node:SingleAxisHingeJoint` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #193 `node:SliderJoint` MISSING **BLOCKED: needs sub:physics.engine** - see behavior
- [ ] #194 `node:UniversalJoint` MISSING **BLOCKED: needs sub:physics.engine** - see behavior

## Sound (22 open)

- [ ] #178 `sub:sound.engine` MISSING - no audio output in Rust
- [ ] #179 `node:AudioClip` NOT_TESTED **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #180 `node:Sound` NOT_TESTED **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #195 `node:Analyser` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #196 `node:AudioDestination` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #197 `node:BiquadFilter` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #198 `node:BufferAudioSource` MISSING **BLOCKED: needs sub:sound.engine** - not implemented; only named in the X3DUrlObject type list of nodes/load_sensor.rs (X3DCast)
- [ ] #199 `node:ChannelMerger` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #200 `node:ChannelSelector` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #201 `node:ChannelSplitter` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #202 `node:Convolver` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #203 `node:Delay` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #204 `node:DynamicsCompressor` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #205 `node:Gain` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #206 `node:ListenerPointSource` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #207 `node:MicrophoneSource` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #208 `node:OscillatorSource` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #209 `node:PeriodicWave` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #210 `node:SpatialSound` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #211 `node:StreamAudioDestination` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #212 `node:StreamAudioSource` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
- [ ] #213 `node:WaveShaper` MISSING **BLOCKED: needs sub:sound.engine** - see behavior
