# Unmapped functions

Upstream: X_ITE 16.5.0 @ 34cdf9f97 (`oracle/x_ite-pin/src`). Generator: `ledger/functions/tools/` (extractor and builder; the rules are in the
builder's module documentation). This file is generated: do not edit it.

`mapping` is code presence only: `cited` (a Rust source line names the X_ITE function), `name` (a Rust function of the same name
in a file or `impl` of the same class; a candidate), `class` (a constructor whose class has Rust files in the 346-entry ledger), `none` (no symbol found by these rules; not proof of absence). `tests` is
separate: `direct` (a Rust test names the function), `entry` (its entry of the 346-entry ledger is COMPLETE with tests), `none`.

Applicable functions with no Rust symbol found: **4035** of 5939. Each is a function to map by hand (a Rust function holds it without a citation), to port, or to mark as not needed in Rust with a reason.

## `src/lib/helper.js` (11; no ledger entry)

`on` (4), `off` (16), `isEmptyObject` (36), `parseXML` (43), `decodeText` (54), `sleep` (61), `toLowerCaseFirst` (65), `toUpperCaseFirst` (69), `try` (73), `gunzip` (91), `isGzip` (113)

## `src/standard/Geospatial/Geodetic.js` (1; no ledger entry)

`Geodetic` (6)

## `src/standard/Geospatial/UniversalTransverseMercator.js` (3; no ledger entry)

`UniversalTransverseMercator` (9), `convert` (41), `apply` (104)

## `src/standard/Math/Algorithm.js` (10; no ledger entry)

`degrees` (8), `random` (13), `project` (36), `roundToMultiple` (40), `lerp` (44), `isPowerOfTwo` (82), `bitCount` (95), `less` (101), `greater` (105), `lowerBound` (109)

## `src/standard/Math/Algorithms/Bezier.js` (1; no ledger entry)

`get_angle` (137)

## `src/standard/Math/Algorithms/MergeSort.js` (5; no ledger entry)

`MergeSort` (1), `compare` (12), `sort` (16), `mergeSort` (20), `merge` (30)

## `src/standard/Math/Algorithms/QuickSort.js` (4; no ledger entry)

`QuickSort` (1), `compare` (11), `sort` (15), `quicksort` (20)

## `src/standard/Math/Algorithms/SAT.js` (4; no ledger entry)

`SAT` (8), `project` (29), `overlaps` (51), `is_between` (57)

## `src/standard/Math/Algorithms/eigen_decomposition.js` (1; no ledger entry)

`eigen_decomposition` (7)

## `src/standard/Math/Geometry/Box2.js` (1; no ledger entry)

`Box2` (4)

## `src/standard/Math/Geometry/Box3.js` (4; no ledger entry)

`Box3` (5), `copy` (14), `assign` (22), `getMatrix` (32)

## `src/standard/Math/Geometry/Cylinder3.js` (1; no ledger entry)

`Cylinder3` (6)

## `src/standard/Math/Geometry/Line2.js` (1; no ledger entry)

`Line2` (3)

## `src/standard/Math/Geometry/Line3.js` (1; no ledger entry)

`Line3` (3)

## `src/standard/Math/Geometry/Plane3.js` (1; no ledger entry)

`Plane3` (8)

## `src/standard/Math/Geometry/Sphere3.js` (1; no ledger entry)

`Sphere3` (3)

## `src/standard/Math/Geometry/Spheroid3.js` (3; no ledger entry)

`Spheroid3` (1), `copy` (8), `assign` (17)

## `src/standard/Math/Geometry/Triangle3.js` (2; no ledger entry)

`vertexCallback` (76), `combineCallback` (83)

## `src/standard/Math/Geometry/ViewVolume.js` (1; no ledger entry)

`getViewport` (88)

## `src/standard/Math/Numbers/Color3.js` (8; no ledger entry)

`Color3` (14), `[Symbol.iterator]` (23), `copy` (29), `toString` (153), `get` (165), `get` (170), `get` (175), `HSV` (194)

## `src/standard/Math/Numbers/Color4.js` (11; no ledger entry)

`Color4` (16), `[Symbol.iterator]` (26), `copy` (33), `sRGBToLinear` (89), `toString` (97), `get` (110), `get` (115), `get` (120), `get` (125), `HSVA` (147), `lerp` (153)

## `src/standard/Math/Numbers/Complex.js` (15; no ledger entry)

`Complex` (14), `conjugate` (25), `copy` (30), `divComp` (38), `inverse` (50), `setPolar` (61), `multComp` (69), `get` (106), `set` (107), `get` (111), `set` (112), `magnitude` (124), `magnitude` (138), `angle` (145), `angle` (146)

## `src/standard/Math/Numbers/Matrix2.js` (20; no ledger entry)

`Matrix2` (3), `[Symbol.iterator]` (13), `copy` (18), `assign` (27), `equals` (34), `set1` (44), `get1` (48), `set` (52), `determinant1` (59), `determinant` (63), `transpose` (69), `inverse` (78), `multLeft` (97), `multRight` (113), `multVecMatrix` (129), `multMatrixVec` (149), `toString` (169), `xAxis` (202), `origin` (206), `submatrix` (210)

## `src/standard/Math/Numbers/Matrix3.js` (13; no ledger entry)

`Matrix3` (6), `[Symbol.iterator]` (23), `copy` (28), `assign` (37), `set1` (54), `get1` (60), `set` (64), `determinant2` (203), `skewX` (411), `skewY` (421), `toString` (431), `fromMatrix2` (518), `fromTransform` (534)

## `src/standard/Math/Numbers/Matrix4.js` (9; no ledger entry)

`Matrix4` (7), `[Symbol.iterator]` (32), `copy` (37), `assign` (46), `set1` (63), `get1` (69), `multMatrixVec` (415), `toString` (493), `fromSubMatrix` (610)

## `src/standard/Math/Numbers/Quaternion.js` (13; no ledger entry)

`Quaternion` (20), `copy` (40), `getEuler` (73), `getMatrix` (189), `pow` (348), `0` (501), `0` (502), `1` (506), `1` (507), `2` (511), `2` (512), `3` (516), `3` (517)

## `src/standard/Math/Numbers/Rotation4.js` (14; no ledger entry)

`Rotation4` (15), `[Symbol.iterator]` (24), `copy` (53), `getEuler` (117), `setAxis` (201), `setAxisAngle` (208), `toString` (352), `get` (368), `get` (385), `get` (402), `get` (419), `fromEuler` (451), `fromMatrix` (455), `fromQuaternion` (459)

## `src/standard/Math/Numbers/Vector2.js` (25; no ledger entry)

`Vector2` (3), `[Symbol.iterator]` (11), `copy` (16), `assign` (23), `set` (29), `equals` (35), `abs` (40), `clamp` (54), `distance` (60), `divide` (65), `divVec` (71), `dot` (77), `inverse` (82), `max` (94), `min` (100), `multVec` (112), `negate` (118), `normalize` (128), `reflect` (140), `squaredNorm` (149), `toString` (162), `0` (177), `0` (178), `1` (182), `1` (183)

## `src/standard/Math/Numbers/Vector3.js` (24; no ledger entry)

`Vector3` (3), `[Symbol.iterator]` (12), `copy` (18), `set` (33), `abs` (46), `clamp` (62), `distance` (79), `divide` (85), `divVec` (92), `dot` (99), `inverse` (105), `max` (119), `min` (126), `multVec` (140), `negate` (147), `reflect` (171), `squaredNorm` (190), `toString` (205), `0` (221), `0` (222), `1` (226), `1` (227), `2` (231), `2` (232)

## `src/standard/Math/Numbers/Vector4.js` (34; no ledger entry)

`Vector4` (3), `[Symbol.iterator]` (13), `copy` (20), `assign` (29), `set` (37), `equals` (45), `abs` (52), `add` (62), `clamp` (70), `distance` (78), `divide` (85), `divVec` (93), `dot` (101), `inverse` (108), `lerp` (116), `max` (124), `min` (132), `multiply` (140), `multVec` (148), `negate` (156), `norm` (164), `normalize` (168), `reflect` (182), `squaredNorm` (193), `subtract` (202), `toString` (210), `0` (227), `0` (228), `1` (232), `1` (233), `2` (237), `2` (238), `3` (242), `3` (243)

## `src/standard/Math/Utility/BVH.js` (11; no ledger entry)

`SortComparator` (25), `Triangle` (34), `intersectsLine` (45), `toArray` (85), `Node` (95), `intersectsLine` (169), `intersectsBBox` (177), `toArray` (255), `BVH` (272), `intersectsLine` (307), `toArray` (319)

## `src/standard/Math/Utility/MatrixStack.js` (14; no ledger entry)

`constructor` (5), `set` (12), `get` (17), `push` (22), `pop` (32), `clear` (37), `size` (42), `identity` (47), `inverse` (52), `multRight` (57), `multLeft` (62), `translate` (67), `rotate` (72), `scale` (77)

## `src/standard/Time/StopWatch.js` (2; no ledger entry)

`start` (8), `reset` (23)

## `src/standard/Utility/BitSet.js` (11; no ledger entry)

`BitSet` (5), `[Symbol.iterator]` (12), `get` (27), `set` (33), `add` (40), `remove` (45), `clear` (50), `entries` (54), `valueOf` (59), `toString` (63), `get` (74)

## `src/standard/Utility/DataStorage.js` (9; no ledger entry)

`get` (8), `set` (22), `DataStorage` (34), `getStorage` (47), `getNameSpace` (51), `addNameSpace` (55), `setDefaultValues` (59), `getDefaultValue` (63), `clear` (69)

## `src/standard/Utility/Lock.js` (1; no ledger entry)

`acquire` (5)

## `src/standard/Utility/ObjectCache.js` (4; no ledger entry)

`ObjectCache` (1), `pop` (6), `push` (13), `clear` (17)

## `src/standard/Utility/Shuffle.js` (1; no ledger entry)

`shuffle` (1)

## `src/x_ite/Base/Events.js` (2; no ledger entry)

`create` (3), `copy` (12)

## `src/x_ite/Base/FieldArray.js` (1; no ledger entry)

`FieldArray` (4)

## `src/x_ite/Base/FieldDefinitionArray.js` (1; no ledger entry)

`FieldDefinitionArray` (4)

## `src/x_ite/Base/IterableWeakSet.js` (12; no ledger entry)

`constructor` (12), `[Symbol.iterator]` (21), `size` (26), `add` (31), `clear` (37), `delete` (45), `forEach` (51), `has` (57), `entries` (62), `keys` (68), `values` (73), `constructor` (87)

## `src/x_ite/Base/X3DArrayField.js` (11; no ledger entry)

`X3DArrayField` (5), `filter` (39), `map` (43), `slice` (47), `toReversed` (51), `toSorted` (55), `toSpliced` (59), `with` (67), `addStaticProperties` (84), `from` (92), `fromArray` (106)

## `src/x_ite/Base/X3DBaseNode.js` (28; no ledger entry)

`setName` (52), `getBrowser` (58), `getScene` (62), `addType` (97), `getType` (101), `initialize` (126), `isInitialized` (128), `getInnerNode` (132), `checkLiveState` (186), `[_set_live__]` (201), `get` (239), `set` (240), `getFieldDefinition` (246), `getFields` (269), `addPredefinedField` (273), `get` (289), `set` (290), `changeField` (301), `get` (318), `set` (319), `removePredefinedField` (327), `getPredefinedFields` (349), `canUserDefinedFields` (353), `getUserDefinedFields` (396), `convertFields` (444), `name_changed` (599), `typeName_changed` (607), `parents_changed` (615)

## `src/x_ite/Base/X3DCast.js` (1; no ledger entry)

`X3DCast` (4)

## `src/x_ite/Base/X3DChildObject.js` (16; no ledger entry)

`isInitializable` (23), `isInput` (27), `isOutput` (31), `setModificationTime` (35), `getModificationTime` (39), `setTainted` (43), `isTainted` (47), `addEventObject` (56), `isPrivate` (66), `setPrivate` (70), `collectCloneCount` (74), `addParent` (88), `removeParent` (94), `getParents` (100), `parentsChanged` (104), `dispose` (106)

## `src/x_ite/Base/X3DConstants.js` (1; no ledger entry)

`value` (10)

## `src/x_ite/Base/X3DField.js` (40; no ledger entry)

`create` (45), `equals` (49), `assign` (53), `getType` (74), `setAccessType` (78), `getAccessType` (82), `isInitializable` (86), `isInput` (90), `isOutput` (94), `isReadable` (98), `isWritable` (102), `removeReference` (150), `addReferencesCallback` (179), `removeReferencesCallback` (185), `getReferencesCallbacks` (189), `processReferencesCallbacks` (193), `removeFieldInterest` (204), `getFieldInterests` (208), `getFieldCallbacks` (222), `addInputRoute` (226), `removeInputRoute` (231), `getInputRoutes` (237), `addOutputRoute` (241), `removeOutputRoute` (246), `getOutputRoutes` (252), `addRouteCallback` (256), `removeRouteCallback` (264), `getRouteCallbacks` (268), `processRouteCallbacks` (272), `getAppInfo` (320), `setAppInfo` (324), `setDocumentation` (328), `getDocumentation` (332), `fromString` (336), `fromJSONString` (348), `dispose` (352), `addStaticProperties` (382), `fromValue` (398), `fromString` (409), `fromJSONString` (442)

## `src/x_ite/Base/X3DFieldDefinition.js` (10; no ledger entry)

`X3DFieldDefinition` (8), `getAccessType` (17), `getDataType` (21), `getName` (25), `getValue` (29), `getAppInfo` (33), `setAppInfo` (37), `getDocumentation` (41), `setDocumentation` (45), `value` (73)

## `src/x_ite/Base/X3DInfoArray.js` (17; no ledger entry)

`set` (22), `ownKeys` (46), `getOwnPropertyDescriptor` (50), `[Symbol.iterator]` (100), `copy` (105), `clear` (114), `assign` (121), `equals` (132), `alias` (175), `filter` (252), `map` (256), `slice` (260), `toReversed` (264), `toSorted` (268), `toSpliced` (272), `with` (280), `toXMLStream` (310)

## `src/x_ite/Base/X3DObject.js` (16; no ledger entry)

`getId` (26), `getTypeName` (30), `getName` (34), `setName` (38), `getDisplayName` (42), `hasInterest` (46), `removeInterest` (71), `getInterests` (75), `getUserData` (94), `setUserData` (98), `removeUserData` (104), `toStream` (172), `dispose` (176), `[Symbol.toStringTag]` (190), `[Symbol.dispose]` (198), `getInterestId` (253)

## `src/x_ite/Base/X3DObjectArrayField.js` (29; no ledger entry)

`get` (12), `has` (52), `ownKeys` (64), `getOwnPropertyDescriptor` (68), `get` (87), `[Symbol.iterator]` (113), `getTarget` (122), `copy` (126), `equals` (136), `isDefaultValue` (158), `setValue` (174), `unshift` (181), `shift` (200), `push` (218), `pop` (237), `splice` (255), `[_insert]` (275), `[_erase]` (293), `resize` (310), `addChildObject` (347), `removeChildObject` (351), `shrinkToFit` (355), `concat` (359), `reverse` (373), `sort` (382), `valueOf` (391), `toVRMLStream` (443), `toXMLStream` (447), `toJSONStream` (469)

## `src/x_ite/Base/X3DTypedArrayField.js` (36; no ledger entry)

`get` (17), `has` (86), `ownKeys` (98), `getOwnPropertyDescriptor` (109), `get` (128), `[Symbol.iterator]` (166), `getTarget` (195), `copy` (199), `equals` (210), `assign` (236), `isDefaultValue` (277), `unshift` (295), `shift` (332), `push` (370), `pop` (405), `splice` (442), `[_insert]` (477), `[_erase]` (510), `resize` (535), `[_grow]` (572), `shrinkToFit` (606), `concat` (622), `flat` (644), `flatMap` (657), `[_fill]` (661), `fill` (727), `reverse` (737), `sort` (779), `valueOf` (827), `toVRMLStream` (925), `toXMLStream` (929), `toJSONStream` (979), `dispose` (1052), `createValue` (1063), `getValue` (1087), `addEvent` (1097)

## `src/x_ite/Browser/Core/BrowserOptions.js` (19; `sub:browser.options` COMPLETE)

`getPrimitiveQuality` (158), `getQualityWhenMoving` (162), `getTextureQuality` (168), `getShading` (172), `getTextCompression` (176), `set_Rubberband__` (186), `set_Antialiased__` (190), `set_PrimitiveQuality__` (194), `set_TextureQuality__` (206), `set_AutoUpdate__` (233), `checkUpdateListener` (251), `checkUpdate` (264), `set_ContentScale__` (295), `updateContentScale` (308), `removeUpdateContentScale` (317), `set_DisplayColorSpace` (328), `set_Exposure__` (338), `set_Timings__` (389), `set_XRSessionMode__` (393)

## `src/x_ite/Browser/Core/BrowserTimings.js` (9; no ledger entry)

`set_type__` (148), `build` (188), `getSpeed` (240), `getSpeedUnit` (247), `getPrimitives` (254), `getOpaqueShapes` (339), `getTransparentShapes` (343), `countShapes` (347), `format` (372)

## `src/x_ite/Browser/Core/Context.js` (2; no ledger entry)

`create` (7), `getVersion` (14)

## `src/x_ite/Browser/Core/ContextMenu.js` (34; no ledger entry)

`initialize` (19), `getUserMenu` (27), `setUserMenu` (31), `createUserMenu` (35), `createItem` (177), `addEvents` (270), `offset` (289), `callback` (334), `callback` (350), `callback` (372), `callback` (383), `callback` (394), `callback` (411), `callback` (422), `callback` (433), `callback` (450), `callback` (461), `callback` (472), `callback` (483), `callback` (494), `callback` (515), `callback` (523), `callback` (535), `linkify` (559), `callback` (695), `getViewpoints` (735), `callback` (757), `getAvailableViewers` (766), `callback` (782), `getViewerName` (792), `createAnimationsMenu` (810), `callback` (838), `callback` (858), `callback` (870)

## `src/x_ite/Browser/Core/Fonts.js` (1; no ledger entry)

`add` (3)

## `src/x_ite/Browser/Core/Notification.js` (2; no ledger entry)

`Notification` (5), `initialize` (14)

## `src/x_ite/Browser/Core/RenderingProperties.js` (1; no ledger entry)

`set_pixelsPerPoint__` (35)

## `src/x_ite/Browser/Core/X3DCoreContext.js` (32; `sub:browser.keyboard-ui` PARTIAL)

`initialize` (129), `src` (155), `src` (159), `url` (167), `url` (171), `getInstanceId` (206), `getElement` (210), `getShadow` (214), `getSurface` (218), `getSplashScreen` (222), `getCanvas` (226), `getContext` (230), `getLocalStorage` (234), `getBrowserTimings` (238), `getBrowserOptions` (242), `getBrowserProperties` (246), `getNotification` (254), `getContextMenu` (258), `getPrivateScene` (262), `checkLiveState` (270), `connectedCallback` (277), `disconnectedCallback` (283), `parseBooleanAttribute` (446), `parseUrlAttribute` (454), `getShiftKey` (498), `getControlKey` (502), `getAltKey` (506), `getCommandKey` (510), `getAltGrKey` (514), `[_keydown]` (518), `[_keyup]` (764), `dispose` (797)

## `src/x_ite/Browser/CubeMapTexturing/X3DCubeMapTexturingContext.js` (2; no ledger entry)

`X3DCubeMapTexturingContext` (5), `getPanoramaShader` (9)

## `src/x_ite/Browser/DOMIntegration.js` (13; `sub:browser.dom-integration` DIFFERENT)

`constructor` (37), `isPrivate` (52), `processCanvasMutation` (58), `processRootElement` (65), `processMutation` (127), `processInlineElements` (259), `processInlineElement` (268), `appendInlineChildElement` (280), `addEventDispatchersAll` (346), `addEventDispatchers` (354), `bindFieldCallback` (373), `fieldCallback` (381), `dispose` (403)

## `src/x_ite/Browser/EnvironmentalEffects/X3DEnvironmentalEffectsContext.js` (2; no ledger entry)

`X3DEnvironmentalEffectsContext` (5), `getBackgroundTextureProperties` (9)

## `src/x_ite/Browser/Followers/X3DArrayChaserTemplate.js` (3; no ledger entry)

`X3DArrayChaserTemplate` (3), `X3DArrayChaserObject` (7), `step` (17)

## `src/x_ite/Browser/Followers/X3DArrayFollowerTemplate.js` (11; no ledger entry)

`X3DArrayFollowerObject` (3), `getArray` (11), `assign` (15), `getValue` (45), `getDestination` (49), `getInitialValue` (53), `getInitialDestination` (57), `setValue` (61), `duplicate` (79), `equals` (87), `interpolate` (101)

## `src/x_ite/Browser/GaussianSplats/GaussianSplatsShape.js` (20; no ledger entry)

`initialize` (66), `getShapeKey` (118), `getGeometryContext` (122), `getGeometryType` (126), `getNumInstances` (130), `set_key__` (134), `set_geometry__` (164), `set_transparent__` (166), `createTexture` (171), `requestRebuild` (189), `displaySimple` (293), `display` (340), `getShader` (418), `createShader` (431), `createPointingShader` (489), `createDepthShader` (524), `initSortWorker` (528), `onmessage` (554), `onerror` (586), `loading` (593)

## `src/x_ite/Browser/Geometry2D/Arc2DOptions.js` (1; no ledger entry)

`Arc2DOptions` (5)

## `src/x_ite/Browser/Geometry2D/ArcClose2DOptions.js` (1; no ledger entry)

`ArcClose2DOptions` (5)

## `src/x_ite/Browser/Geometry2D/Circle2DOptions.js` (7; no ledger entry)

`Circle2DOptions` (8), `initialize` (17), `getGeometry` (23), `createCoordIndex` (30), `createPoints` (41), `build` (55), `eventsProcessed` (70)

## `src/x_ite/Browser/Geometry2D/Disk2DOptions.js` (6; no ledger entry)

`Disk2DOptions` (7), `initialize` (20), `getDiskTexCoords` (26), `getDiskNormals` (33), `getDiskVertices` (40), `eventsProcessed` (94)

## `src/x_ite/Browser/Geometry2D/Rectangle2DOptions.js` (2; no ledger entry)

`Rectangle2DOptions` (6), `getGeometry` (13)

## `src/x_ite/Browser/Geometry2D/X3DGeometry2DContext.js` (7; no ledger entry)

`initialize` (19), `getArc2DOptions` (23), `getArcClose2DOptions` (27), `getCircle2DOptions` (31), `getDisk2DOptions` (35), `getRectangle2DOptions` (39), `getOptionNode` (81)

## `src/x_ite/Browser/Geometry3D/BoxOptions.js` (1; no ledger entry)

`initialize` (13)

## `src/x_ite/Browser/Geometry3D/ConeOptions.js` (10; no ledger entry)

`initialize` (21), `getSideGeometry` (27), `getBottomGeometry` (34), `createTexCoordIndex` (41), `createTexCoord` (60), `createNormalIndex` (96), `createNormal` (115), `createCoordIndex` (150), `createPoints` (169), `eventsProcessed` (219)

## `src/x_ite/Browser/Geometry3D/CylinderOptions.js` (11; no ledger entry)

`initialize` (21), `getSideGeometry` (27), `getTopGeometry` (34), `getBottomGeometry` (41), `createTexCoordIndex` (48), `createTexCoord` (75), `createNormalIndex` (123), `createNormal` (150), `createCoordIndex` (176), `createPoints` (203), `eventsProcessed` (272)

## `src/x_ite/Browser/Geometry3D/IcoSphere.js` (15; no ledger entry)

`IcoSphere` (5), `getCoordIndex` (16), `getPoint` (21), `getTexCoordIndex` (26), `getTexPoint` (31), `coord` (36), `octahedron` (52), `icosahedron` (77), `refineTriangles` (136), `addPoint` (164), `addTriangle` (171), `getMiddlePoint` (179), `sphericalMapping` (205), `resolveOverlap` (324), `applyRadius` (348)

## `src/x_ite/Browser/Geometry3D/IcoSphereOptions.js` (5; no ledger entry)

`IcoSphereOptions` (9), `initialize` (19), `getGeometry` (25), `build` (32), `eventsProcessed` (55)

## `src/x_ite/Browser/Geometry3D/QuadSphereOptions.js` (7; no ledger entry)

`initialize` (21), `getGeometry` (27), `createTexCoordIndex` (34), `createTexCoord` (77), `createCoordIndex` (118), `createPoints` (181), `eventsProcessed` (229)

## `src/x_ite/Browser/Geometry3D/X3DGeometry3DContext.js` (6; no ledger entry)

`initialize` (17), `getBoxOptions` (21), `getConeOptions` (25), `getCylinderOptions` (29), `getSphereOptions` (33), `getOptionNode` (74)

## `src/x_ite/Browser/Geospatial/Geocentric.js` (3; no ledger entry)

`Geocentric` (3), `convert` (7), `apply` (11)

## `src/x_ite/Browser/Geospatial/GeospatialObject.js` (8; no ledger entry)

`getElevationFrame` (54), `getCoordinateSystem` (58), `getEllipsoid` (70), `isStandardOrder` (94), `getLatitudeFirst` (114), `getNorthingFirst` (124), `getZone` (134), `getNorthernHemisphere` (146)

## `src/x_ite/Browser/Grouping/X3DBBoxNode.js` (1; no ledger entry)

`getShapes` (44)

## `src/x_ite/Browser/Interpolation/CatmullRomSplineInterpolator.js` (1; no ledger entry)

`CatmullRomSplineInterpolator` (4)

## `src/x_ite/Browser/Interpolation/CatmullRomSplineInterpolator1.js` (9; no ledger entry)

`CatmullRomSplineInterpolator1` (3), `create` (10), `copy` (14), `subtract` (18), `multiply` (22), `divide` (26), `magnitude` (30), `distance` (34), `dot` (38)

## `src/x_ite/Browser/Interpolation/CatmullRomSplineInterpolatorTemplate.js` (9; no ledger entry)

`CatmullRomSplineInterpolatorTemplate` (3), `CatmullRomSplineInterpolatorInstance` (5), `create` (13), `copy` (17), `subtract` (21), `multiply` (25), `divide` (29), `magnitude` (33), `distance` (37)

## `src/x_ite/Browser/KeyDeviceSensor/X3DKeyDeviceSensorContext.js` (8; no ledger entry)

`X3DKeyDeviceSensorContext` (7), `initialize` (14), `addKeyDeviceSensorNode` (21), `removeKeyDeviceSensorNode` (25), `getKeyDeviceSensorNodes` (29), `[_keydown]` (33), `[_keyup]` (46), `dispose` (59)

## `src/x_ite/Browser/Layering/X3DLayeringContext.js` (3; no ledger entry)

`X3DLayeringContext` (5), `initialize` (12), `getDefaultViewport` (16)

## `src/x_ite/Browser/Layout/ScreenText.js` (3; no ledger entry)

`getTextureNode` (25), `drawGlyph` (280), `traverseAfter` (349)

## `src/x_ite/Browser/Legacy.js` (5; no ledger entry)

`elements` (3), `properties` (13), `set` (24), `browser` (34), `error` (48)

## `src/x_ite/Browser/Lighting/X3DLightingContext.js` (6; no ledger entry)

`getMaxEnvironmentLights` (31), `getMaxLights` (35), `getLibraryTexture` (39), `popShadowBuffer` (66), `pushShadowBuffer` (90), `getEnvironmentTextureShader` (95)

## `src/x_ite/Browser/NURBS/NURBS.js` (4; no ledger entry)

`getUVWeights` (238), `getControlPoints2D` (259), `getControlPoints` (295), `getTexControlPoints` (379)

## `src/x_ite/Browser/Navigation/ExamineViewer.js` (4; `sub:nav.examine` PARTIAL)

`startPan` (667), `set_positionOffset__` (841), `set_centerOfRotationOffset__` (847), `dispose` (868)

## `src/x_ite/Browser/Navigation/FlyViewer.js` (4; no ledger entry)

`FlyViewer` (4), `isCollidable` (11), `getFlyDirection` (15), `constrainPanDirection` (28)

## `src/x_ite/Browser/Navigation/LookAtViewer.js` (13; no ledger entry)

`LookAtViewer` (16), `initialize` (35), `mousedown` (64), `mouseup` (99), `dblclick` (118), `mousemove` (137), `touchstart` (198), `touchend` (244), `set_positionOffset__` (331), `set_centerOfRotationOffset__` (337), `set_orientationOffset__` (343), `disconnect` (441), `dispose` (447)

## `src/x_ite/Browser/Navigation/NoneViewer.js` (1; no ledger entry)

`NoneViewer` (3)

## `src/x_ite/Browser/Navigation/PlaneViewer.js` (8; no ledger entry)

`initialize` (32), `mousedown` (48), `mouseup` (105), `mousemove` (148), `wheel` (180), `touchstart` (221), `touchend` (266), `dispose` (392)

## `src/x_ite/Browser/Navigation/WalkViewer.js` (5; no ledger entry)

`WalkViewer` (5), `isCollidable` (12), `getStraightenHorizon` (16), `getFlyDirection` (20), `constrainPanDirection` (46)

## `src/x_ite/Browser/Navigation/X3DFlyViewer.js` (10; no ledger entry)

`initialize` (42), `mousedown` (64), `mouseup` (156), `mousemove` (180), `wheel` (236), `set_orientationOffset__` (457), `addFly` (463), `addPan` (475), `disconnect` (609), `dispose` (623)

## `src/x_ite/Browser/Navigation/X3DNavigationContext.js` (16; no ledger entry)

`initialize` (28), `createHeadlight` (38), `getHeadlight` (51), `getActiveLayer` (55), `getActiveNavigationInfo` (59), `getActiveViewpoint` (63), `getCurrentViewer` (67), `getViewer` (71), `addCollision` (75), `removeCollision` (79), `getCollisionCount` (83), `remove_world__` (87), `set_world__` (93), `set_activeLayer__` (99), `set_activeViewpoint__` (138), `dispose` (179)

## `src/x_ite/Browser/Navigation/X3DViewer.js` (8; no ledger entry)

`isActive` (28), `getActiveLayer` (32), `getNavigationInfo` (40), `getStraightenHorizon` (48), `getDistanceToCenter` (105), `touch` (151), `disconnect` (155), `tbProjectToSphere` (159)

## `src/x_ite/Browser/Networking/URLs.js` (5; no ledger entry)

`getProviderURL` (37), `getFontsURL` (46), `getLinetypeURL` (50), `getHatchingURL` (54), `getLibraryURL` (58)

## `src/x_ite/Browser/Networking/X3DNetworkingContext.js` (15; no ledger entry)

`getBaseURI` (20), `initialize` (45), `getScriptURL` (49), `getProviderURL` (53), `getBaseURL` (57), `setBaseURL` (61), `getBrowserLoading` (69), `setBrowserLoading` (73), `loading` (109), `getLoadingObjects` (127), `addLoadingObject` (131), `removeLoadingObject` (143), `setLoadingFractions` (168), `[_set_loadCount]` (177), `getDefaultScene` (221)

## `src/x_ite/Browser/ParticleSystems/X3DParticleSystemsContext.js` (2; no ledger entry)

`X3DParticleSystemsContext` (5), `getDefaultEmitter` (9)

## `src/x_ite/Browser/Picking/VolumePicker.js` (3; no ledger entry)

`setChildShape1Components` (38), `setChildShape2Components` (42), `setChildShapeComponents` (70)

## `src/x_ite/Browser/Picking/X3DPickingContext.js` (9; no ledger entry)

`addTransformSensor` (22), `removeTransformSensor` (27), `addPickSensor` (32), `removePickSensor` (37), `getPickSensors` (42), `getPickingHierarchy` (46), `getPickable` (50), `enablePicking` (54), `getPickingTime` (79)

## `src/x_ite/Browser/PointingDeviceSensor/PointingDevice.js` (6; no ledger entry)

`initialize` (17), `dblclick` (97), `mousemove` (112), `onmouseout` (218), `onverifymotion` (222), `showContextMenu` (232)

## `src/x_ite/Browser/PointingDeviceSensor/X3DPointingDeviceSensorContext.js` (18; no ledger entry)

`copy` (56), `initialize` (79), `getPointingTime` (85), `addPointingDeviceSensor` (89), `removePointingDeviceSensor` (93), `setCursors` (97), `copyEvent` (120), `getPointer` (133), `getPointingInputSource` (154), `getHit` (162), `removeHit` (166), `addSensor` (174), `addPointingShape` (178), `buttonReleaseEvent` (219), `leaveNotifyEvent` (262), `motion` (349), `getPointingShader` (390), `createPointingShader` (420)

## `src/x_ite/Browser/Rendering/GeometryContext.js` (3; no ledger entry)

`GeometryContext` (4), `getTextureCoordinateMapping` (26), `getTextureCoordinate` (30)

## `src/x_ite/Browser/Rendering/MikkTSpace.js` (16; no ledger entry)

`initialize` (9), `__wbindgen_string_new` (17), `__wbindgen_rethrow` (23), `isInitialized` (40), `load` (45), `generateTangents` (78), `getUint8Memory0` (112), `getStringFromWasm0` (120), `addHeapObject` (128), `getObject` (141), `dropObject` (146), `takeObject` (156), `getFloat32Memory0` (167), `passArrayF32ToWasm0` (177), `getInt32Memory0` (190), `getArrayF32FromWasm0` (198)

## `src/x_ite/Browser/Rendering/ScreenLine.js` (3; no ledger entry)

`ScreenLine` (10), `setColor` (92), `dispose` (182)

## `src/x_ite/Browser/Rendering/ScreenPoint.js` (2; no ledger entry)

`ScreenPoint` (7), `dispose` (157)

## `src/x_ite/Browser/Rendering/X3DRenderingContext.js` (31; no ledger entry)

`initialize` (41), `getRenderer` (83), `getVendor` (97), `getWebGLVersion` (111), `getMaxSamples` (117), `getMaxClipPlanes` (123), `getDepthSize` (127), `getColorDepth` (133), `getViewport` (142), `getLocalObjects` (146), `getFramebuffers` (150), `getDefaultFramebuffer` (154), `setDefaultFramebuffer` (158), `addTextureBuffer` (169), `getFullscreenVertexArrayObject` (173), `getOITComposeShader` (195), `getDepthShader` (203), `createDepthShader` (233), `resize` (285), `test` (293), `reshapeTextureBuffer` (402), `resetShapeId` (416), `getShapeId` (420), `onfullscreen` (424), `xrCheckSupport` (433), `xrUpdateButton` (449), `xrFrame` (469), `getSession` (471), `setSession` (475), `getPose` (479), `dispose` (483)

## `src/x_ite/Browser/Scripting/X3DScriptingContext.js` (3; no ledger entry)

`X3DScriptingContext` (1), `isExternal` (5), `getScriptNode` (9)

## `src/x_ite/Browser/Scripting/evaluate.js` (1; no ledger entry)

`evaluate` (1)

## `src/x_ite/Browser/Shaders/ShaderRegistry.js` (3; no ledger entry)

`addVertexShader` (97), `addFragmentShader` (101), `addIncludeFile` (105)

## `src/x_ite/Browser/Shaders/X3DShadersContext.js` (10; no ledger entry)

`initialize` (20), `getShadingLanguageVersion` (24), `getMaxVertexUniformVectors` (30), `getMaxFragmentUniformVectors` (36), `getMaxVertexAttribs` (42), `getMaxVaryingVectors` (48), `getPrimitiveMode` (54), `getShaders` (58), `getWireframe` (118), `createShader` (122)

## `src/x_ite/Browser/Shape/X3DShapeContext.js` (8; no ledger entry)

`getDefaultAppearance` (29), `getDefaultPointProperties` (40), `getDefaultLineProperties` (51), `getDefaultMaterial` (64), `getLineTransformShader` (117), `getLineTransformInstancedShader` (121), `createLineTransformShader` (125), `getLineTransformFeedback` (170)

## `src/x_ite/Browser/Sound/AudioElement.js` (10; no ledger entry)

`detune` (21), `detune` (25), `playbackRate` (33), `playbackRate` (37), `loopStart` (45), `loopStart` (49), `loopEnd` (57), `loopEnd` (61), `loop` (69), `loop` (73)

## `src/x_ite/Browser/Sound/X3DSoundContext.js` (6; no ledger entry)

`initialize` (21), `removeSoundDestination` (63), `getSoundDestinations` (69), `stopAudioElement` (92), `toggleNoSoundButton` (103), `getDefaultPeriodicWave` (136)

## `src/x_ite/Browser/Text/PolygonText.js` (4; no ledger entry)

`getTextureNode` (18), `getGlyph` (145), `vertexCallback` (274), `combineCallback` (279)

## `src/x_ite/Browser/Text/X3DTextContext.js` (3; no ledger entry)

`isWoff2` (128), `getWoff2Decoder` (139), `loadWoff2Decoder` (143)

## `src/x_ite/Browser/Text/X3DTextGeometry.js` (13; no ledger entry)

`getBrowser` (46), `getText` (50), `getFontStyle` (54), `getGlyphs` (58), `getMinorAlignment` (62), `getTranslations` (66), `getCharSpacings` (70), `getScales` (74), `getBearing` (78), `getBBox` (82), `resizeArray` (135), `horizontal` (144), `vertical` (309)

## `src/x_ite/Browser/Texturing/GifMedia.js` (10; no ledger entry)

`GifMedia` (1), `currentTime` (12), `duration` (23), `loop` (30), `loop` (34), `cycle` (42), `currentFrame` (49), `play` (60), `playbackRate` (68), `playbackRate` (72)

## `src/x_ite/Browser/Texturing/KTXDecoder.js` (4; no ledger entry)

`constructor` (7), `init` (14), `transcode` (21), `loadKTXFromBuffer` (97)

## `src/x_ite/Browser/Texturing/PNGMedia.js` (12; no ledger entry)

`PNGMedia` (1), `currentTime` (19), `currentTime` (25), `duration` (32), `loop` (39), `loop` (43), `cycle` (50), `currentFrame` (57), `play` (64), `pause` (72), `playbackRate` (79), `playbackRate` (83)

## `src/x_ite/Browser/Texturing/X3DTexturingContext.js` (22; no ledger entry)

`initialize` (40), `getMaxTextures` (106), `getMaxTextureTransforms` (110), `getMaxTexCoords` (114), `getMinTextureSize` (118), `getMaxTextureSize` (122), `getMaxAnisotropicDegree` (151), `getMaxCombinedTextureUnits` (159), `popGlobalTextureUnit` (165), `resetGlobalTextureUnits` (169), `popTextureUnit` (173), `resetTextureUnits` (177), `getDefaultTexture2DUnit` (181), `getDefaultTexture3DUnit` (185), `getDefaultTextureCubeUnit` (189), `getDefaultTexture2D` (193), `getDefaultTexture3D` (197), `getDefaultTextureCube` (201), `getDefaultTextureProperties` (205), `getDefaultTextureTransform` (220), `getDefaultTextureCoordinate` (231), `getKTXDecoder` (279)

## `src/x_ite/Browser/Texturing3D/DICOMParser.js` (38; no ledger entry)

`DicomParser` (4), `getPhotometricInterpretation` (42), `getComponents` (47), `getWidth` (52), `getHeight` (56), `getDepth` (60), `getBitsAllocated` (69), `getBitsStored` (73), `getPixelRepresentation` (77), `getPlanarConfiguration` (81), `getTansferSyntax` (85), `getPixelData` (89), `getFrames` (260), `getTypedArray` (326), `flipImage` (340), `getNormalizeOffsetAndFactor` (361), `unpackBinaryFrame` (377), `isBitSet` (379), `decodeLittleEndian` (404), `decodeBigEndian` (439), `swap16` (441), `decodeRLE` (473), `decodeRLE8` (488), `decodeRLE8Planar` (542), `decodeRLE16` (595), `decodeJPEGBaseline` (644), `decodeJPEGLossless` (664), `decodeJPEGLS` (672), `decodeJPEG2000` (754), `convertRGBColorByPlane` (858), `convertYBRFullByPixel` (880), `convertYBRFullByPlane` (907), `convertPaletteColor` (934), `convertLUTto8Bit` (936), `getLUT` (989), `populatePaletteColorLut` (1000), `getLutDescriptor` (1040), `getLutData` (1047)

## `src/x_ite/Browser/Texturing3D/NRRDParser.js` (11; no ledger entry)

`parse` (28), `setInput` (40), `getNRRD` (48), `getFields` (60), `getDimension` (145), `getSizes` (166), `getEndian` (217), `getData` (233), `ascii` (259), `hex` (371), `gzip` (392)

## `src/x_ite/Browser/Time/X3DTimeContext.js` (3; no ledger entry)

`getCurrentTime` (20), `getCurrentFrameRate` (24), `getCurrentSpeed` (28)

## `src/x_ite/Browser/VolumeRendering/VolumeMaterial.js` (5; no ledger entry)

`VolumeMaterial` (5), `getVolumeShaders` (15), `getShader` (19), `createShader` (35), `setShaderUniforms` (45)

## `src/x_ite/Browser/VolumeRendering/X3DVolumeRenderingContext.js` (2; no ledger entry)

`getDefaultVolumeStyle` (31), `getDefaultBlendedVolumeStyle` (42)

## `src/x_ite/Browser/WebXR/X3DWebXRContext.js` (8; no ledger entry)

`getPose` (35), `xrAddButton` (39), `xrToggleSession` (45), `xrStopSession` (132), `xrContentScale` (173), `xrUpdateBaseLayer` (177), `xrUpdateVisibility` (221), `xrSensorHitPulse` (527)

## `src/x_ite/Browser/WebXR/xrX3DViewer.js` (1; no ledger entry)

`gamepads` (5)

## `src/x_ite/Browser/X3DBrowser.js` (54; `sub:browser.sai-api` COMPLETE)

`initialize` (73), `getWelcomeMessage` (106), `getName` (131), `getVersion` (135), `getDescription` (139), `getWorldURL` (147), `getProfile` (151), `addSupportedProfile` (160), `updateSupportedProfile` (164), `removeSupportedProfile` (168), `getSupportedProfile` (172), `getSupportedProfiles` (181), `addSupportedComponent` (204), `updateSupportedComponent` (208), `removeSupportedComponent` (212), `getSupportedComponent` (216), `getSupportedComponents` (225), `loadComponent` (236), `addConcreteNode` (287), `updateConcreteNode` (291), `removeConcreteNode` (295), `getConcreteNode` (299), `addAbstractNode` (312), `updateAbstractNode` (316), `removeAbstractNode` (320), `getAbstractNode` (324), `getAbstractNodes` (333), `getFieldType` (337), `getFieldTypes` (346), `checkLoadCount` (448), `addBrowserListener` (670), `removeBrowserListener` (674), `removeBrowserCallback` (704), `getBrowserCallbacks` (726), `importDocument` (738), `getBrowserProperty` (778), `setBrowserOption` (782), `getBrowserOption` (786), `getRenderingProperty` (790), `addRoute` (880), `deleteRoute` (884), `constrainTranslation` (888), `getClosestObject` (902), `isContextLost` (926), `loseContext` (930), `print` (945), `println` (954), `toVRMLStream` (963), `toXMLStream` (967), `toJSONStream` (971), `dispose` (975), `activeLayer` (1070), `activeNavigationInfo` (1081), `activeViewpoint` (1092)

## `src/x_ite/Browser/X3DBrowserContext.js` (25; no ledger entry)

`initialize` (99), `initialized` (106), `shutdown` (110), `prepareEvents` (114), `timePrepareEvents` (118), `timeEvents` (122), `cameraEvents` (126), `sensorEvents` (130), `displayEvents` (134), `displayFinishEvents` (138), `finishEvents` (142), `getBrowser` (146), `getWorld` (150), `setExecutionContext` (158), `setSession` (166), `addBrowserEvent` (176), `nextFrame` (184), `[_traverse]` (199), `getSystemTime` (281), `getBrowserTime` (285), `getCameraTime` (289), `getCollisionTime` (293), `getDisplayTime` (297), `dispose` (301), `addComponent` (321)

## `src/x_ite/Browser/X_ITE/ExtensionKeys.js` (1; no ledger entry)

`add` (5)

## `src/x_ite/Browser/X_ITE/ScatterSamples.js` (5; no ledger entry)

`computeScatterSamples` (16), `burleySample` (47), `burleyEval` (79), `burleyPdf` (90), `burleySetup` (95)

## `src/x_ite/Browser/X_ITE/X3DX_ITEContext.js` (1; no ledger entry)

`initialize` (11)

## `src/x_ite/Components.js` (1; no ledger entry)

`add` (25)

## `src/x_ite/Components/Annotation/AnnotationLayer.js` (1; `node:AnnotationLayer` DIFFERENT)

`initialize` (22)

## `src/x_ite/Components/Annotation/AnnotationTarget.js` (1; `node:AnnotationTarget` DIFFERENT)

`initialize` (17)

## `src/x_ite/Components/Annotation/GroupAnnotation.js` (2; `node:GroupAnnotation` DIFFERENT)

`initialize` (20), `dispose` (25)

## `src/x_ite/Components/Annotation/IconAnnotation.js` (3; `node:IconAnnotation` DIFFERENT)

`initialize` (20), `requestImmediateLoad` (25), `dispose` (27)

## `src/x_ite/Components/Annotation/TextAnnotation.js` (1; `node:TextAnnotation` DIFFERENT)

`initialize` (17)

## `src/x_ite/Components/Annotation/URLAnnotation.js` (1; `node:URLAnnotation` DIFFERENT)

`initialize` (17)

## `src/x_ite/Components/Annotation/X3DAnnotationNode.js` (2; no ledger entry)

`X3DAnnotationNode` (5), `initialize` (14)

## `src/x_ite/Components/CADGeometry/CADFace.js` (13; `node:CADFace` COMPLETE)

`initialize` (34), `getBBox` (45), `getShapes` (52), `requestRebuild` (59), `set_boundedObjects__` (177), `set_pointingObjects__` (181), `set_cameraObjects__` (185), `set_pickableObjects__` (189), `set_collisionObjects__` (193), `set_shadowObjects__` (197), `set_visibleObjects__` (201), `traverse` (205), `dispose` (256)

## `src/x_ite/Components/CADGeometry/IndexedQuadSet.js` (4; `node:IndexedQuadSet` COMPLETE)

`initialize` (17), `getPolygonIndex` (34), `getVerticesPerPolygon` (38), `getNumVertices` (42)

## `src/x_ite/Components/CADGeometry/QuadSet.js` (3; `node:QuadSet` COMPLETE)

`getVerticesPerPolygon` (28), `getNumVertices` (32), `createNormals` (42)

## `src/x_ite/Components/CADGeometry/X3DProductStructureChildNode.js` (1; no ledger entry)

`X3DProductStructureChildNode` (5)

## `src/x_ite/Components/Core/MetadataBoolean.js` (3; `node:MetadataBoolean` COMPLETE)

`initialize` (20), `getContainerField` (25), `dispose` (32)

## `src/x_ite/Components/Core/MetadataDouble.js` (3; `node:MetadataDouble` COMPLETE)

`initialize` (20), `getContainerField` (25), `dispose` (32)

## `src/x_ite/Components/Core/MetadataFloat.js` (3; `node:MetadataFloat` COMPLETE)

`initialize` (20), `getContainerField` (25), `dispose` (32)

## `src/x_ite/Components/Core/MetadataInteger.js` (3; `node:MetadataInteger` COMPLETE)

`initialize` (20), `getContainerField` (25), `dispose` (32)

## `src/x_ite/Components/Core/MetadataSet.js` (7; `node:MetadataSet` COMPLETE)

`initialize` (20), `getContainerField` (25), `getMetaValue` (32), `setMetaValue` (36), `removeMetaValue` (71), `getMetadataObject` (80), `dispose` (104)

## `src/x_ite/Components/Core/MetadataString.js` (3; `node:MetadataString` COMPLETE)

`initialize` (20), `getContainerField` (25), `dispose` (32)

## `src/x_ite/Components/Core/X3DBindableNode.js` (5; no ledger entry)

`initialize` (20), `isCameraObject` (26), `getModificationCount` (30), `transitionStart` (34), `getModificationCount` (51)

## `src/x_ite/Components/Core/X3DChildNode.js` (16; no ledger entry)

`isBoundedObject` (31), `setPointingObject` (35), `isPointingObject` (40), `setCameraObject` (44), `isCameraObject` (49), `setPickableObject` (53), `isPickableObject` (58), `setShadowObject` (62), `isShadowObject` (67), `setCollisionObject` (71), `isCollisionObject` (76), `setVisibleObject` (80), `isVisible` (89), `connectChildNode` (94), `disconnectChildNode` (138), `getShapes` (148)

## `src/x_ite/Components/Core/X3DImportedNodeInstance.js` (7; no ledger entry)

`initialize` (21), `update` (29), `getExtendedEventHandling` (38), `getInnerNode` (42), `getImportedNode` (46), `getName` (50), `dispose` (102)

## `src/x_ite/Components/Core/X3DInfoNode.js` (1; no ledger entry)

`X3DInfoNode` (5)

## `src/x_ite/Components/Core/X3DMetadataObject.js` (3; no ledger entry)

`X3DMetadataObject` (4), `initialize` (11), `dispose` (13)

## `src/x_ite/Components/Core/X3DNode.js` (14; no ledger entry)

`getComponentInfo` (19), `getSpecificationRange` (27), `getDisplayName` (175), `collectCloneCount` (237), `traverse` (245), `hasMetaData` (247), `getMetaData` (261), `removeMetaData` (538), `removeMetaData` (545), `getMetadataSet` (563), `addMetaDataCallback` (589), `removeMetaDataCallback` (601), `processMetaDataCallback` (613), `getStaticProperties` (1488)

## `src/x_ite/Components/Core/X3DPrototypeInstance.js` (16; no ledger entry)

`create` (27), `getTypeName` (31), `getAppInfo` (35), `getDocumentation` (39), `set_live__` (85), `getExtendedEventHandling` (277), `getProtoNode` (281), `setProtoNode` (285), `getBody` (305), `addExternProtos` (335), `addProtos` (342), `copyNamedNodes` (349), `copyRootNodes` (354), `copyImportedNodes` (361), `copyRoutes` (379), `dispose` (729)

## `src/x_ite/Components/Core/X3DSensorNode.js` (1; no ledger entry)

`X3DSensorNode` (5)

## `src/x_ite/Components/CubeMapTexturing/ComposedCubeMapTexture.js` (3; `node:ComposedCubeMapTexture` COMPLETE)

`set_loadState__` (75), `setTextureBit` (81), `isComplete` (85)

## `src/x_ite/Components/CubeMapTexturing/GeneratedCubeMapTexture.js` (5; `node:GeneratedCubeMapTexture` COMPLETE)

`initialize` (34), `isRenderedTexture` (44), `addUpdateCallback` (52), `removeUpdateCallback` (56), `setShaderUniforms` (242)

## `src/x_ite/Components/CubeMapTexturing/ImageCubeMapTexture.js` (5; `node:ImageCubeMapTexture` COMPLETE)

`initialize` (23), `unloadData` (30), `setKTXTexture` (81), `imageToCubeMap` (127), `dispose` (276)

## `src/x_ite/Components/CubeMapTexturing/X3DEnvironmentTextureNode.js` (6; no ledger entry)

`getTarget` (35), `getTargets` (39), `getTextureType` (43), `getLevels` (62), `updateTextureParameters` (85), `setShaderUniforms` (97)

## `src/x_ite/Components/DIS/DISEntityTypeMapping.js` (2; `node:DISEntityTypeMapping` COMPLETE)

`initialize` (24), `dispose` (29)

## `src/x_ite/Components/DIS/EspduTransform.js` (1; `node:EspduTransform` COMPLETE)

`initialize` (32)

## `src/x_ite/Components/DIS/ReceiverPdu.js` (3; `node:ReceiverPdu` COMPLETE)

`initialize` (32), `getBBox` (37), `dispose` (44)

## `src/x_ite/Components/DIS/SignalPdu.js` (3; `node:SignalPdu` COMPLETE)

`initialize` (31), `getBBox` (36), `dispose` (43)

## `src/x_ite/Components/DIS/TransmitterPdu.js` (3; `node:TransmitterPdu` COMPLETE)

`initialize` (31), `getBBox` (36), `dispose` (43)

## `src/x_ite/Components/EnvironmentalEffects/Fog.js` (3; `node:Fog` COMPLETE)

`initialize` (25), `getModelMatrix` (30), `dispose` (40)

## `src/x_ite/Components/EnvironmentalEffects/FogCoordinate.js` (4; `node:FogCoordinate` COMPLETE)

`initialize` (21), `set_depth__` (29), `getSize` (34), `addDepths` (49)

## `src/x_ite/Components/EnvironmentalEffects/LocalFog.js` (3; `node:LocalFog` COMPLETE)

`initialize` (20), `pop` (36), `dispose` (41)

## `src/x_ite/Components/EnvironmentalEffects/TextureBackground.js` (2; `node:TextureBackground` COMPLETE)

`initialize` (20), `traverse` (45)

## `src/x_ite/Components/EnvironmentalEffects/X3DBackgroundNode.js` (12; no ledger entry)

`initialize` (53), `isHidden` (76), `setHidden` (80), `getRenderPassNodes` (106), `getTextureNodes` (110), `set_texture__` (114), `setTextureBit` (128), `getColor` (132), `transferSphere` (281), `traverse` (389), `drawSphere` (485), `drawRectangle` (573)

## `src/x_ite/Components/EnvironmentalEffects/X3DFogObject.js` (10; no ledger entry)

`FogContainer` (8), `getFogType` (21), `dispose` (37), `initialize` (61), `isHidden` (72), `setHidden` (76), `getFogType` (83), `getFogs` (87), `set_color__` (106), `dispose` (121)

## `src/x_ite/Components/EnvironmentalSensor/ProximitySensor.js` (1; `node:ProximitySensor` COMPLETE)

`initialize` (37)

## `src/x_ite/Components/EnvironmentalSensor/TransformSensor.js` (4; `node:TransformSensor` COMPLETE)

`set_live__` (54), `set_extents__` (77), `intersects` (159), `containsPoint` (183)

## `src/x_ite/Components/EnvironmentalSensor/VisibilitySensor.js` (1; `node:VisibilitySensor` COMPLETE)

`initialize` (23)

## `src/x_ite/Components/EnvironmentalSensor/X3DEnvironmentalSensorNode.js` (8; no ledger entry)

`X3DEnvironmentalSensorNode` (7), `initialize` (28), `set_live__` (40), `setZeroTest` (57), `getZeroTest` (61), `setTraversed` (65), `getTraversed` (80), `update` (84)

## `src/x_ite/Components/EventUtilities/BooleanFilter.js` (2; `node:BooleanFilter` COMPLETE)

`initialize` (17), `set_boolean__` (23)

## `src/x_ite/Components/EventUtilities/BooleanSequencer.js` (3; `node:BooleanSequencer` COMPLETE)

`initialize` (17), `getSize` (23), `sequence` (27)

## `src/x_ite/Components/EventUtilities/BooleanToggle.js` (2; `node:BooleanToggle` COMPLETE)

`initialize` (17), `set_boolean__` (23)

## `src/x_ite/Components/EventUtilities/BooleanTrigger.js` (2; `node:BooleanTrigger` COMPLETE)

`initialize` (17), `set_triggerTime__` (23)

## `src/x_ite/Components/EventUtilities/IntegerSequencer.js` (3; `node:IntegerSequencer` COMPLETE)

`initialize` (17), `getSize` (23), `sequence` (27)

## `src/x_ite/Components/EventUtilities/IntegerTrigger.js` (2; `node:IntegerTrigger` COMPLETE)

`initialize` (17), `set_boolean__` (23)

## `src/x_ite/Components/EventUtilities/TimeTrigger.js` (2; `node:TimeTrigger` COMPLETE)

`initialize` (17), `set_boolean__` (23)

## `src/x_ite/Components/EventUtilities/X3DSequencerNode.js` (6; no ledger entry)

`X3DSequencerNode` (6), `initialize` (17), `set_fraction__` (26), `set_previous__` (59), `set_next__` (73), `set_index__` (87)

## `src/x_ite/Components/EventUtilities/X3DTriggerNode.js` (1; no ledger entry)

`X3DTriggerNode` (5)

## `src/x_ite/Components/Followers/ColorChaser.js` (5; `node:ColorChaser` COMPLETE)

`getVector` (25), `getValue` (29), `getDestination` (33), `getInitialValue` (37), `getInitialDestination` (41)

## `src/x_ite/Components/Followers/ColorDamper.js` (5; `node:ColorDamper` COMPLETE)

`getVector` (25), `getValue` (29), `getDestination` (33), `getInitialValue` (37), `getInitialDestination` (41)

## `src/x_ite/Components/Followers/CoordinateChaser.js` (1; `node:CoordinateChaser` COMPLETE)

`getVector` (23)

## `src/x_ite/Components/Followers/CoordinateDamper.js` (1; `node:CoordinateDamper` COMPLETE)

`getVector` (23)

## `src/x_ite/Components/Followers/OrientationChaser.js` (1; `node:OrientationChaser` COMPLETE)

`getVector` (30)

## `src/x_ite/Components/Followers/OrientationDamper.js` (1; `node:OrientationDamper` COMPLETE)

`getVector` (30)

## `src/x_ite/Components/Followers/PositionChaser.js` (1; `node:PositionChaser` COMPLETE)

`getVector` (18)

## `src/x_ite/Components/Followers/PositionChaser2D.js` (1; `node:PositionChaser2D` COMPLETE)

`getVector` (18)

## `src/x_ite/Components/Followers/PositionDamper.js` (1; `node:PositionDamper` COMPLETE)

`getVector` (18)

## `src/x_ite/Components/Followers/PositionDamper2D.js` (1; `node:PositionDamper2D` COMPLETE)

`getVector` (18)

## `src/x_ite/Components/Followers/ScalarChaser.js` (4; `node:ScalarChaser` COMPLETE)

`getVector` (18), `setPreviousValue` (22), `duplicate` (30), `assign` (34)

## `src/x_ite/Components/Followers/ScalarDamper.js` (3; `node:ScalarDamper` COMPLETE)

`getVector` (18), `duplicate` (22), `assign` (26)

## `src/x_ite/Components/Followers/TexCoordChaser2D.js` (1; `node:TexCoordChaser2D` COMPLETE)

`getVector` (23)

## `src/x_ite/Components/Followers/TexCoordDamper2D.js` (1; `node:TexCoordDamper2D` COMPLETE)

`getVector` (23)

## `src/x_ite/Components/Followers/X3DChaserNode.js` (13; no ledger entry)

`X3DChaserNode` (5), `initialize` (22), `getNumBuffers` (54), `getTolerance` (58), `getArray` (62), `setPreviousValue` (66), `step` (70), `stepResponse` (74), `set_value__` (86), `set_destination__` (103), `set_duration__` (112), `prepareEvents` (116), `updateBuffer` (137)

## `src/x_ite/Components/Followers/X3DDamperNode.js` (3; no ledger entry)

`set_value__` (84), `set_destination__` (97), `set_order__` (103)

## `src/x_ite/Components/Followers/X3DFollowerNode.js` (10; no ledger entry)

`getBuffer` (26), `getDestination` (34), `getInitialValue` (38), `getInitialDestination` (42), `setValue` (46), `setDestination` (50), `duplicate` (54), `assign` (58), `equals` (62), `interpolate` (66)

## `src/x_ite/Components/GaussianSplats/GaussianSplats.js` (2; `node:GaussianSplats` PARTIAL)

`getInnerNode` (51), `getBBox` (55)

## `src/x_ite/Components/GaussianSplats/X3DGaussianSplatsNode.js` (3; no ledger entry)

`X3DGaussianSplatsNode` (6), `initialize` (17), `dispose` (22)

## `src/x_ite/Components/Geometry2D/Arc2D.js` (2; `node:Arc2D` COMPLETE)

`set_live__` (25), `getSweepAngle` (29)

## `src/x_ite/Components/Geometry2D/ArcClose2D.js` (2; `node:ArcClose2D` COMPLETE)

`set_live__` (27), `getSweepAngle` (31)

## `src/x_ite/Components/Geometry2D/Circle2D.js` (1; `node:Circle2D` COMPLETE)

`set_live__` (21)

## `src/x_ite/Components/Geometry2D/Disk2D.js` (2; `node:Disk2D` COMPLETE)

`set_live__` (25), `updateRenderFunctions` (155)

## `src/x_ite/Components/Geometry3D/Cone.js` (2; `node:Cone` COMPLETE)

`set_live__` (24), `setExtents` (95)

## `src/x_ite/Components/Geometry3D/Cylinder.js` (2; `node:Cylinder` COMPLETE)

`set_live__` (22), `setExtents` (114)

## `src/x_ite/Components/Geometry3D/ElevationGrid.js` (5; `node:ElevationGrid` COMPLETE)

`initialize` (28), `createTexCoords` (47), `createNormals` (64), `createCoordIndex` (100), `createPoints` (135)

## `src/x_ite/Components/Geometry3D/Extrusion.js` (3; `node:Extrusion` COMPLETE)

`getClosed` (37), `INDEX` (317), `addCap` (605)

## `src/x_ite/Components/Geometry3D/IndexedFaceSet.js` (2; `node:IndexedFaceSet` COMPLETE)

`initialize` (23), `generateNormals` (215)

## `src/x_ite/Components/Geometry3D/Sphere.js` (1; `node:Sphere` COMPLETE)

`set_live__` (21)

## `src/x_ite/Components/Geospatial/GeoCoordinate.js` (2; `node:GeoCoordinate` COMPLETE)

`initialize` (22), `dispose` (165)

## `src/x_ite/Components/Geospatial/GeoElevationGrid.js` (5; `node:GeoElevationGrid` COMPLETE)

`initialize` (29), `createTexCoords` (45), `createNormals` (62), `createCoordIndex` (99), `dispose` (275)

## `src/x_ite/Components/Geospatial/GeoLOD.js` (11; `node:GeoLOD` COMPLETE)

`initialize` (47), `getBBox` (88), `getShapes` (110), `set_live__` (130), `set_childBoundedObject__` (167), `set_childPointingObject__` (171), `set_childCameraObject__` (175), `set_childPickableObject__` (179), `set_childCollisionObject__` (183), `set_childShadowObject__` (187), `dispose` (324)

## `src/x_ite/Components/Geospatial/GeoLocation.js` (2; `node:GeoLocation` COMPLETE)

`initialize` (21), `dispose` (39)

## `src/x_ite/Components/Geospatial/GeoMetadata.js` (3; `node:GeoMetadata` COMPLETE)

`initialize` (20), `requestImmediateLoad` (25), `dispose` (27)

## `src/x_ite/Components/Geospatial/GeoOrigin.js` (2; `node:GeoOrigin` COMPLETE)

`initialize` (19), `set_geoSystem__` (27)

## `src/x_ite/Components/Geospatial/GeoPositionInterpolator.js` (4; `node:GeoPositionInterpolator` COMPLETE)

`setup` (30), `initialize` (36), `set_keyValue__` (42), `dispose` (69)

## `src/x_ite/Components/Geospatial/GeoProximitySensor.js` (3; `node:GeoProximitySensor` COMPLETE)

`initialize` (31), `traverse` (68), `dispose` (72)

## `src/x_ite/Components/Geospatial/GeoTouchSensor.js` (2; `node:GeoTouchSensor` COMPLETE)

`initialize` (25), `dispose` (42)

## `src/x_ite/Components/Geospatial/GeoTransform.js` (2; `node:GeoTransform` COMPLETE)

`initialize` (25), `dispose` (52)

## `src/x_ite/Components/Geospatial/GeoViewpoint.js` (2; `node:GeoViewpoint` COMPLETE)

`dispose` (193), `traverse` (200)

## `src/x_ite/Components/Geospatial/X3DGeospatialObject.js` (20; no ledger entry)

`X3DGeospatialObject` (16), `initialize` (28), `set_geoSystem__` (36), `set_geoOrigin__` (43), `set_origin__` (64), `set_originMatrix__` (73), `set_rotateYUp__` (106), `getReferenceFrame` (123), `getStandardOrder` (127), `getCoord` (131), `getGeoCoord` (135), `getGeoElevation` (139), `getGeoUpVector` (143), `getLocationMatrix` (147), `dispose` (160), `getCoordRotateYUp` (164), `getGeoCoordRotateYUp` (169), `getGeoUpVectorRotateYUp` (174), `getLocationMatrixRotateYUp` (179), `getStandardLocationMatrix` (184)

## `src/x_ite/Components/Grouping/StaticGroup.js` (9; `node:StaticGroup` COMPLETE)

`initialize` (51), `getBBox` (67), `getShapes` (79), `set_children__` (83), `createStaticShapes` (99), `hasTextureCoordinateGenerator` (238), `optimizeGroups` (253), `combineShapes` (322), `dispose` (802)

## `src/x_ite/Components/Grouping/Switch.js` (3; `node:Switch` COMPLETE)

`initialize` (22), `addChildren` (28), `set_addChildren__` (32)

## `src/x_ite/Components/Grouping/X3DBoundedObject.js` (13; no ledger entry)

`initialize` (30), `isVisible` (39), `isHidden` (43), `setHidden` (47), `isDefaultBBoxSize` (54), `isBBoxVisible` (58), `getBBoxNode` (76), `addTransformSensor` (88), `removeTransformSensor` (94), `getTransformSensors` (100), `set_visible_and_hidden__` (104), `set_bboxSize_and_bboxCenter__` (113), `dispose` (117)

## `src/x_ite/Components/Grouping/X3DGroupingNode.js` (17; no ledger entry)

`addAllowedTypes` (60), `getSubBBox` (72), `requestRebuild` (83), `set_children__` (138), `connectChildren` (143), `clearChildren` (148), `addChildren` (180), `removeChildren` (288), `removeChild` (295), `set_boundedObjects__` (374), `set_pointingObjects__` (378), `set_cameraObjects__` (382), `set_pickableObjects__` (386), `set_collisionObjects__` (390), `set_shadowObjects__` (394), `set_visibleObjects__` (398), `dispose` (541)

## `src/x_ite/Components/Grouping/X3DTransformMatrix3DNode.js` (7; no ledger entry)

`X3DTransformMatrix3DNode` (8), `getBBox` (19), `getShapes` (26), `getMatrix` (32), `setMatrix` (36), `setTransform` (59), `traverse` (82)

## `src/x_ite/Components/Grouping/X3DTransformNode.js` (3; no ledger entry)

`X3DTransformNode` (5), `initialize` (19), `eventsProcessed` (27)

## `src/x_ite/Components/HAnim/HAnimHumanoid.js` (19; `node:HAnimHumanoid` COMPLETE)

`initialize` (71), `set_depreciated__` (170), `getBBox` (180), `getSubBBox` (184), `getShapes` (188), `getMatrix` (192), `getHAnimKey` (196), `set_humanoidKey__` (200), `set_pose_fraction__` (234), `set_pose_active__` (247), `set_motions__` (258), `set_connectAllJoints__` (297), `set_connectJoints__` (302), `set_skinCoord__` (504), `getShaderOptions` (583), `setShaderUniforms` (589), `constructor` (673), `unlock` (694), `lock` (699)

## `src/x_ite/Components/HAnim/HAnimJoint.js` (4; `node:HAnimJoint` COMPLETE)

`initialize` (43), `getModelViewMatrix` (51), `set_visibleObjects__` (59), `groupTraverse` (108)

## `src/x_ite/Components/HAnim/HAnimMotion.js` (12; `node:HAnimMotion` COMPLETE)

`initialize` (28), `connectJoints` (60), `disconnectJoints` (83), `getJointsIndex` (96), `set_enabled__` (105), `set_joints__` (112), `set_next_or_previous__` (289), `set_frameIndex__` (319), `set_frameDuration__` (334), `set_frameIncrement__` (342), `set_start_or_endFrame__` (346), `getFraction` (358)

## `src/x_ite/Components/HAnim/HAnimPose.js` (10; `node:HAnimPose` COMPLETE)

`initialize` (30), `addJoints` (56), `removeJoints` (62), `setNeedsUpdateInterpolators` (66), `updateInterpolators` (70), `set_resetAllJoints__` (134), `set_commencePose__` (143), `set_startTime__` (150), `set_fraction__` (156), `set_children__` (166)

## `src/x_ite/Components/HAnim/HAnimSegment.js` (8; `node:HAnimSegment` COMPLETE)

`initialize` (37), `getHAnimKey` (72), `set_humanoidKey__` (76), `set_displacers__` (80), `set_displacementsTexture__` (113), `set_displacementWeightsTexture__` (169), `getShaderOptions` (257), `setShaderUniforms` (263)

## `src/x_ite/Components/Interpolation/ColorInterpolator.js` (2; `node:ColorInterpolator` COMPLETE)

`initialize` (20), `set_keyValue__` (26)

## `src/x_ite/Components/Interpolation/CoordinateInterpolator.js` (1; `node:CoordinateInterpolator` COMPLETE)

`set_keyValue__` (18)

## `src/x_ite/Components/Interpolation/CoordinateInterpolator2D.js` (1; `node:CoordinateInterpolator2D` COMPLETE)

`set_keyValue__` (18)

## `src/x_ite/Components/Interpolation/EaseInEaseOut.js` (2; `node:EaseInEaseOut` COMPLETE)

`initialize` (17), `set_keyValue__` (23)

## `src/x_ite/Components/Interpolation/NormalInterpolator.js` (2; `node:NormalInterpolator` COMPLETE)

`initialize` (19), `set_keyValue__` (25)

## `src/x_ite/Components/Interpolation/OrientationInterpolator.js` (2; `node:OrientationInterpolator` COMPLETE)

`initialize` (23), `set_keyValue__` (29)

## `src/x_ite/Components/Interpolation/PositionInterpolator.js` (2; `node:PositionInterpolator` COMPLETE)

`initialize` (18), `set_keyValue__` (24)

## `src/x_ite/Components/Interpolation/PositionInterpolator2D.js` (2; `node:PositionInterpolator2D` COMPLETE)

`initialize` (18), `set_keyValue__` (24)

## `src/x_ite/Components/Interpolation/ScalarInterpolator.js` (2; `node:ScalarInterpolator` COMPLETE)

`initialize` (18), `set_keyValue__` (24)

## `src/x_ite/Components/Interpolation/SplinePositionInterpolator.js` (4; `node:SplinePositionInterpolator` DIFFERENT)

`initialize` (20), `set_keyValue__` (28), `set_keyVelocity__` (39), `set_normalizeVelocity__` (49)

## `src/x_ite/Components/Interpolation/SplinePositionInterpolator2D.js` (4; `node:SplinePositionInterpolator2D` DIFFERENT)

`initialize` (20), `set_keyValue__` (28), `set_keyVelocity__` (39), `set_normalizeVelocity__` (49)

## `src/x_ite/Components/Interpolation/SplineScalarInterpolator.js` (4; `node:SplineScalarInterpolator` COMPLETE)

`initialize` (20), `set_keyValue__` (28), `set_keyVelocity__` (39), `set_normalizeVelocity__` (49)

## `src/x_ite/Components/Interpolation/SquadOrientationInterpolator.js` (2; `node:SquadOrientationInterpolator` COMPLETE)

`initialize` (27), `set_keyValue__` (33)

## `src/x_ite/Components/KeyDeviceSensor/KeySensor.js` (3; `node:KeySensor` COMPLETE)

`keydown` (40), `keyup` (157), `release` (280)

## `src/x_ite/Components/KeyDeviceSensor/StringSensor.js` (1; `node:StringSensor` COMPLETE)

`keydown` (17)

## `src/x_ite/Components/KeyDeviceSensor/X3DKeyDeviceSensorNode.js` (5; no ledger entry)

`enable` (45), `disable` (62), `keydown` (68), `keyup` (70), `release` (72)

## `src/x_ite/Components/Layering/LayerSet.js` (3; `node:LayerSet` COMPLETE)

`initialize` (23), `getLayer0` (41), `setLayer0` (45)

## `src/x_ite/Components/Layering/Viewport.js` (2; `node:Viewport` COMPLETE)

`initialize` (24), `set_clipBoundary__` (32)

## `src/x_ite/Components/Layering/X3DLayerNode.js` (23; no ledger entry)

`initialize` (83), `isHidden` (121), `setHidden` (125), `isLayer0` (136), `getLayer` (145), `getGroups` (149), `getGroup` (153), `getViewport` (157), `getBackground` (161), `getFog` (165), `getNavigationInfo` (169), `getBackgrounds` (177), `getFogs` (181), `getNavigationInfos` (185), `getViewpoints` (189), `getBackgroundStack` (204), `getFogStack` (208), `getNavigationInfoStack` (212), `getViewpointStack` (216), `straightenView` (228), `set_addChildren__` (246), `set_removeChildren__` (255), `pointer` (393)

## `src/x_ite/Components/Layering/X3DViewportNode.js` (1; no ledger entry)

`X3DViewportNode` (5)

## `src/x_ite/Components/Layout/Layout.js` (19; `node:Layout` COMPLETE)

`initialize` (39), `set_align__` (57), `set_offsetUnits__` (83), `set_offset__` (116), `set_sizeUnits__` (125), `set_size__` (158), `set_scaleMode__` (167), `getAlignX` (206), `getAlignY` (210), `getOffsetUnitX` (214), `getOffsetUnitY` (221), `getOffsetX` (228), `getOffsetY` (232), `getSizeUnitX` (236), `getSizeUnitY` (243), `getSizeX` (250), `getSizeY` (254), `getScaleModeX` (258), `getScaleModeY` (268)

## `src/x_ite/Components/Layout/LayoutGroup.js` (5; `node:LayoutGroup` COMPLETE)

`initialize` (27), `set_layout__` (41), `set_collisionObjects__` (45), `getBBox` (47), `getMatrix` (54)

## `src/x_ite/Components/Layout/LayoutLayer.js` (1; `node:LayoutLayer` COMPLETE)

`initialize` (22)

## `src/x_ite/Components/Layout/ScreenFontStyle.js` (1; `node:ScreenFontStyle` COMPLETE)

`createTextGeometry` (25)

## `src/x_ite/Components/Layout/ScreenGroup.js` (2; `node:ScreenGroup` COMPLETE)

`getBBox` (27), `getMatrix` (34)

## `src/x_ite/Components/Layout/X3DLayoutNode.js` (1; no ledger entry)

`X3DLayoutNode` (5)

## `src/x_ite/Components/Lighting/DirectionalLight.js` (5; `node:DirectionalLight` COMPLETE)

`DirectionalLightContainer` (20), `set` (40), `renderShadowMap` (57), `setShaderUniforms` (114), `getLights` (197)

## `src/x_ite/Components/Lighting/EnvironmentLight.js` (12; `node:EnvironmentLight` COMPLETE)

`EnvironmentLightContainer` (20), `set` (29), `renderShadowMap` (37), `setShaderUniforms` (48), `dispose` (126), `initialize` (154), `getLightKey` (165), `getDiffuseTexture` (169), `getSpecularTexture` (173), `getSheenTexture` (177), `getLights` (181), `set_specularTexture__` (185)

## `src/x_ite/Components/Lighting/PointLight.js` (10; `node:PointLight` COMPLETE)

`PointLightContainer` (44), `set` (65), `renderShadowMap` (82), `setShaderUniforms` (147), `initialize` (236), `set_attenuation__` (244), `getAttenuation` (252), `getLocation` (256), `getRadius` (260), `getLights` (265)

## `src/x_ite/Components/Lighting/SpotLight.js` (12; `node:SpotLight` COMPLETE)

`SpotLightContainer` (21), `set` (46), `renderShadowMap` (63), `setShaderUniforms` (135), `initialize` (242), `set_attenuation__` (250), `getAttenuation` (258), `getLocation` (262), `getRadius` (266), `getBeamWidth` (271), `getCutOffAngle` (284), `getLights` (288)

## `src/x_ite/Components/Lighting/X3DLightNode.js` (13; no ledger entry)

`initialize` (20), `set_global__` (31), `getLightKey` (48), `getEnabled` (52), `setEnabled` (56), `getGlobal` (60), `getColor` (64), `getIntensity` (68), `getAmbientIntensity` (72), `getDirection` (76), `getShadows` (80), `getShadowColor` (84), `getShadowBias` (92)

## `src/x_ite/Components/NURBS/Contour2D.js` (5; `node:Contour2D` COMPLETE)

`initialize` (19), `set_addChildren__` (29), `set_removeChildren__` (40), `set_children__` (48), `filter` (86)

## `src/x_ite/Components/NURBS/ContourPolyline2D.js` (1; `node:ContourPolyline2D` COMPLETE)

`tessellate` (20)

## `src/x_ite/Components/NURBS/NurbsCurve.js` (7; `node:NurbsCurve` DIFFERENT)

`initialize` (29), `set_controlPoint__` (37), `getTessellation` (45), `getClosed` (49), `getWeights` (56), `getControlPoints` (60), `dispose` (144)

## `src/x_ite/Components/NURBS/NurbsCurve2D.js` (5; `node:NurbsCurve2D` DIFFERENT)

`getTessellation` (27), `getClosed` (31), `getKnots` (38), `getWeights` (42), `getControlPoints` (46)

## `src/x_ite/Components/NURBS/NurbsOrientationInterpolator.js` (1; `node:NurbsOrientationInterpolator` DIFFERENT)

`initialize` (23)

## `src/x_ite/Components/NURBS/NurbsPositionInterpolator.js` (1; `node:NurbsPositionInterpolator` DIFFERENT)

`initialize` (22)

## `src/x_ite/Components/NURBS/NurbsSet.js` (6; `node:NurbsSet` DIFFERENT)

`initialize` (23), `getBBox` (35), `set_addGeometry__` (56), `set_removeGeometry__` (67), `dispose` (92), `filter` (99)

## `src/x_ite/Components/NURBS/NurbsSurfaceInterpolator.js` (1; `node:NurbsSurfaceInterpolator` DIFFERENT)

`initialize` (22)

## `src/x_ite/Components/NURBS/NurbsSweptSurface.js` (4; `node:NurbsSweptSurface` DIFFERENT)

`initialize` (21), `getTrajectoryCurve` (45), `set_crossSectionCurve__` (49), `set_trajectoryCurve__` (57)

## `src/x_ite/Components/NURBS/NurbsSwungSurface.js` (4; `node:NurbsSwungSurface` DIFFERENT)

`initialize` (21), `getTrajectoryCurve` (45), `set_profileCurve__` (49), `set_trajectoryCurve__` (57)

## `src/x_ite/Components/NURBS/NurbsTextureCoordinate.js` (1; `node:NurbsTextureCoordinate` DIFFERENT)

`getControlPoints` (19)

## `src/x_ite/Components/NURBS/NurbsTrimmedSurface.js` (10; `node:NurbsTrimmedSurface` DIFFERENT)

`initialize` (24), `set_addTrimmingContour__` (34), `set_removeTrimmingContour__` (45), `set_trimmingContour__` (53), `getTrimmingContours` (73), `createDefaultNurbsTriangles` (275), `vertexCallback` (292), `combineCallback` (297), `combineCallbackIndex` (302), `filter` (340)

## `src/x_ite/Components/NURBS/X3DNurbsControlCurveNode.js` (1; no ledger entry)

`X3DNurbsControlCurveNode` (4)

## `src/x_ite/Components/NURBS/X3DNurbsSurfaceGeometryNode.js` (15; no ledger entry)

`initialize` (33), `set_texCoord__` (43), `set_controlPoint__` (54), `getUTessellation` (68), `getVTessellation` (72), `getUClosed` (76), `getVClosed` (83), `getUVWeights` (90), `getTexControlPoints` (94), `getUVControlPoints` (98), `getSurface` (102), `trimSurface` (106), `build` (108), `generateNormals` (284), `createNormals` (293)

## `src/x_ite/Components/NURBS/X3DParametricGeometryNode.js` (2; no ledger entry)

`X3DParametricGeometryNode` (6), `getKnots` (15)

## `src/x_ite/Components/Navigation/Billboard.js` (1; `node:Billboard` COMPLETE)

`getMatrix` (32)

## `src/x_ite/Components/Navigation/Collision.js` (3; `node:Collision` COMPLETE)

`initialize` (27), `set_live__` (38), `traverse` (80)

## `src/x_ite/Components/Navigation/LOD.js` (3; `node:LOD` COMPLETE)

`addChildren` (36), `set_addChildren__` (40), `set_visibleObjects__` (65)

## `src/x_ite/Components/Navigation/NavigationInfo.js` (13; `node:NavigationInfo` COMPLETE)

`initialize` (34), `getViewer` (51), `getCollisionRadius` (55), `getAvatarHeight` (59), `getStepHeight` (63), `getNearValue` (67), `getFarValue` (71), `set_avatarSize__` (210), `set_headlight__` (219), `set_visibilityLimit__` (226), `set_transitionStart__` (230), `set_transitionComplete__` (235), `traverse` (258)

## `src/x_ite/Components/Navigation/OrthoViewpoint.js` (14; `node:OrthoViewpoint` COMPLETE)

`initialize` (49), `set_fieldOfViewOffset__` (73), `getRelativeTransformation` (89), `getFieldOfView` (140), `setFieldOfView` (149), `getUserFieldOfView` (153), `getMinimumX` (162), `getMinimumY` (170), `getUserMinimumY` (174), `getMaximumX` (178), `getMaximumY` (186), `getUserMaximumY` (190), `getSizeX` (194), `getSizeY` (202)

## `src/x_ite/Components/Navigation/Viewpoint.js` (4; `node:Viewpoint` COMPLETE)

`getRelativeTransformation` (32), `setFieldOfView` (68), `getLookAtDistance` (122), `getProjectionMatrixWithLimits` (147)

## `src/x_ite/Components/Navigation/ViewpointGroup.js` (3; `node:ViewpointGroup` COMPLETE)

`initialize` (35), `getDisplayed` (55), `set_children__` (72)

## `src/x_ite/Components/Navigation/X3DViewpointNode.js` (25; no ledger entry)

`initialize` (57), `set_nearDistance__` (104), `set_farDistance__` (110), `setInterpolators` (158), `getPosition` (164), `setPosition` (168), `getOrientation` (172), `setOrientation` (176), `getCenterOfRotation` (180), `setCenterOfRotation` (184), `getUserPosition` (188), `getUserOrientation` (203), `getUserCenterOfRotation` (219), `getFieldOfViewScale` (234), `setFieldOfViewScale` (238), `getNearDistance` (242), `setNearDistance` (246), `getFarDistance` (250), `setFarDistance` (254), `getProjectionMatrix` (258), `getCameraSpaceMatrix` (266), `getViewMatrix` (270), `getModelMatrix` (274), `setVRMLTransition` (292), `getVRMLTransition` (297)

## `src/x_ite/Components/Networking/Anchor.js` (4; `node:Anchor` COMPLETE)

`initialize` (28), `set_load__` (51), `set_url__` (53), `requestUnload` (95)

## `src/x_ite/Components/Networking/Inline.js` (4; `node:Inline` PARTIAL)

`set_live__` (52), `getBBox` (57), `getShapes` (64), `dispose` (205)

## `src/x_ite/Components/Networking/InlineGeometry.js` (6; `node:InlineGeometry` COMPLETE)

`initialize` (32), `set_live__` (44), `loadData` (104), `getInternalScene` (171), `build` (230), `dispose` (232)

## `src/x_ite/Components/Networking/LoadSensor.js` (9; `node:LoadSensor` COMPLETE)

`set_enabled__` (39), `set_timeOut__` (50), `set_children__` (64), `set_loadState__` (68), `count` (75), `abort` (128), `reset` (137), `remove` (159), `clearTimeout` (170)

## `src/x_ite/Components/Networking/X3DNetworkSensorNode.js` (1; no ledger entry)

`X3DNetworkSensorNode` (5)

## `src/x_ite/Components/Networking/X3DUrlObject.js` (7; no ledger entry)

`getAllowEmptyUrl` (34), `getLoadState` (65), `setCache` (69), `getCache` (73), `loading` (121), `test` (127), `dispose` (235)

## `src/x_ite/Components/Networking/X3DUrlOutputObject.js` (1; no ledger entry)

`X3DUrlOutputObject` (5)

## `src/x_ite/Components/ParticleSystems/BoundedPhysicsModel.js` (4; `node:BoundedPhysicsModel` PARTIAL)

`initialize` (18), `getBBox` (26), `set_geometry__` (30), `addGeometry` (38)

## `src/x_ite/Components/ParticleSystems/ConeEmitter.js` (4; `node:ConeEmitter` COMPLETE)

`initialize` (23), `set_position__` (75), `set_direction__` (83), `set_angle__` (89)

## `src/x_ite/Components/ParticleSystems/ExplosionEmitter.js` (3; `node:ExplosionEmitter` COMPLETE)

`initialize` (22), `isExplosive` (56), `set_position__` (60)

## `src/x_ite/Components/ParticleSystems/ParticleSystem.js` (17; `node:ParticleSystem` PARTIAL)

`getGeometryContext` (188), `getGeometryType` (198), `getNumInstances` (202), `set_geometryType__` (324), `set_particleLifetime__` (410), `set_lifetimeVariation__` (414), `set_particleSize__` (418), `set_emitter__` (425), `set_physics__` (434), `set_colorRamp__` (539), `set_texCoordRamp__` (588), `set_texCoord__` (598), `set_scaleRamp__` (634), `updateVertexArrays` (682), `createTexture` (690), `createBuffer` (707), `resizeBuffers` (718)

## `src/x_ite/Components/ParticleSystems/PointEmitter.js` (2; `node:PointEmitter` COMPLETE)

`initialize` (22), `set_position__` (65)

## `src/x_ite/Components/ParticleSystems/PolylineEmitter.js` (2; `node:PolylineEmitter` COMPLETE)

`set_verticesIndex__` (124), `activateTextures` (175)

## `src/x_ite/Components/ParticleSystems/SurfaceEmitter.js` (6; `node:SurfaceEmitter` COMPLETE)

`initialize` (24), `set_surface__` (92), `set_solid__` (111), `set_verticesIndex__` (115), `set_normalsIndex__` (119), `activateTextures` (193)

## `src/x_ite/Components/ParticleSystems/VolumeEmitter.js` (5; `node:VolumeEmitter` PARTIAL)

`set_verticesIndex__` (151), `set_normalsIndex__` (155), `set_hierarchyIndex__` (159), `set_hierarchyRoot__` (163), `activateTextures` (238)

## `src/x_ite/Components/ParticleSystems/WindPhysicsModel.js` (1; `node:WindPhysicsModel` PARTIAL)

`getRandomSpeed` (28)

## `src/x_ite/Components/ParticleSystems/X3DParticleEmitterNode.js` (21; no ledger entry)

`initialize` (37), `isExplosive` (75), `getMass` (79), `getSurfaceArea` (83), `set_on__` (87), `set_speed__` (91), `set_variation__` (95), `set_mass__` (99), `set_surfaceArea__` (103), `getRandomValue` (107), `getRandomNormal` (111), `addDefine` (202), `addSampler` (206), `addUniform` (210), `setUniform` (214), `addCallback` (224), `addFunction` (228), `getProgram` (232), `activateTextures` (975), `createTexture` (977), `popTextureUnit` (994)

## `src/x_ite/Components/ParticleSystems/X3DParticlePhysicsModelNode.js` (2; no ledger entry)

`X3DParticlePhysicsModelNode` (4), `addForce` (13)

## `src/x_ite/Components/Picking/LinePickSensor.js` (1; `node:LinePickSensor` COMPLETE)

`initialize` (25)

## `src/x_ite/Components/Picking/PickableGroup.js` (3; `node:PickableGroup` COMPLETE)

`initialize` (26), `set_pickableObjects__` (33), `dispose` (133)

## `src/x_ite/Components/Picking/PointPickSensor.js` (1; `node:PointPickSensor` COMPLETE)

`initialize` (29)

## `src/x_ite/Components/Picking/PrimitivePickSensor.js` (1; `node:PrimitivePickSensor` COMPLETE)

`initialize` (25)

## `src/x_ite/Components/Picking/VolumePickSensor.js` (2; `node:VolumePickSensor` COMPLETE)

`initialize` (26), `set_pickingGeometry__` (34)

## `src/x_ite/Components/Picking/X3DPickSensorNode.js` (13; no ledger entry)

`compareDistance` (14), `getObjectType` (55), `getMatchCriterion` (59), `getIntersectionType` (63), `getSortOrder` (67), `getModelMatrices` (71), `getTargets` (75), `getPickedGeometries` (111), `getPickedGeometry` (187), `getPickedTargets` (199), `set_live__` (203), `set_objectType__` (216), `set_pickTarget__` (255)

## `src/x_ite/Components/Picking/X3DPickableObject.js` (5; no ledger entry)

`X3DPickableObject` (4), `initialize` (15), `getObjectType` (21), `set_objectType__` (25), `dispose` (32)

## `src/x_ite/Components/PointingDeviceSensor/CylinderSensor.js` (6; `node:CylinderSensor` COMPLETE)

`initialize` (31), `isBehind` (51), `getTrackPoint` (61), `getAngle` (83), `set_fields__` (90), `activate` (118)

## `src/x_ite/Components/PointingDeviceSensor/PlaneSensor.js` (5; `node:PlaneSensor` COMPLETE)

`initialize` (38), `set_fields__` (65), `activate` (95), `trackStart` (153), `track` (208)

## `src/x_ite/Components/PointingDeviceSensor/SphereSensor.js` (2; `node:SphereSensor` COMPLETE)

`initialize` (23), `getTrackPoint` (37)

## `src/x_ite/Components/PointingDeviceSensor/X3DDragSensorNode.js` (1; no ledger entry)

`X3DDragSensorNode` (5)

## `src/x_ite/Components/PointingDeviceSensor/X3DPointingDeviceSensorNode.js` (9; no ledger entry)

`PointingDeviceSensorContainer` (10), `set` (21), `set_active__` (33), `set_motion__` (37), `dispose` (41), `initialize` (60), `set_live__` (70), `set_active__` (101), `set_motion__` (108)

## `src/x_ite/Components/PointingDeviceSensor/X3DTouchSensorNode.js` (2; no ledger entry)

`X3DTouchSensorNode` (8), `set_active__` (17)

## `src/x_ite/Components/Rendering/ClipPlane.js` (6; `node:ClipPlane` COMPLETE)

`ClipPlaneContainer` (13), `dispose` (43), `getClipPlaneKey` (63), `initialize` (67), `set_enabled__` (76), `pop` (94)

## `src/x_ite/Components/Rendering/Color.js` (4; `node:Color` COMPLETE)

`initialize` (17), `set_color__` (25), `addColor` (30), `addColors` (45)

## `src/x_ite/Components/Rendering/ColorRGBA.js` (4; `node:ColorRGBA` COMPLETE)

`initialize` (19), `set_color__` (27), `addColor` (32), `addColors` (47)

## `src/x_ite/Components/Rendering/IndexedLineSet.js` (2; `node:IndexedLineSet` COMPLETE)

`initialize` (17), `getPolylineIndices` (38)

## `src/x_ite/Components/Rendering/IndexedTriangleFanSet.js` (4; `node:IndexedTriangleFanSet` COMPLETE)

`initialize` (21), `getPolygonIndex` (69), `getVerticesPerPolygon` (73), `getNumVertices` (77)

## `src/x_ite/Components/Rendering/IndexedTriangleSet.js` (4; `node:IndexedTriangleSet` COMPLETE)

`getPolygonIndex` (17), `getVerticesPerPolygon` (21), `getNumVertices` (25), `initialize` (29)

## `src/x_ite/Components/Rendering/IndexedTriangleStripSet.js` (4; `node:IndexedTriangleStripSet` COMPLETE)

`initialize` (21), `getPolygonIndex` (77), `getVerticesPerPolygon` (81), `getNumVertices` (85)

## `src/x_ite/Components/Rendering/LineSet.js` (1; `node:LineSet` COMPLETE)

`initialize` (17)

## `src/x_ite/Components/Rendering/Normal.js` (2; `node:Normal` COMPLETE)

`initialize` (17), `set_vector__` (25)

## `src/x_ite/Components/Rendering/PointSet.js` (1; `node:PointSet` COMPLETE)

`initialize` (17)

## `src/x_ite/Components/Rendering/Tangent.js` (3; `node:Tangent` COMPLETE)

`initialize` (21), `set_vector__` (29), `addVectors` (49)

## `src/x_ite/Components/Rendering/TriangleFanSet.js` (4; `node:TriangleFanSet` COMPLETE)

`initialize` (21), `getPolygonIndex` (53), `getVerticesPerPolygon` (57), `getNumVertices` (61)

## `src/x_ite/Components/Rendering/TriangleSet.js` (3; `node:TriangleSet` COMPLETE)

`getVerticesPerPolygon` (17), `getNumVertices` (21), `createNormals` (31)

## `src/x_ite/Components/Rendering/TriangleStripSet.js` (4; `node:TriangleStripSet` COMPLETE)

`initialize` (21), `getPolygonIndex` (57), `getVerticesPerPolygon` (61), `getNumVertices` (65)

## `src/x_ite/Components/Rendering/X3DColorNode.js` (3; no ledger entry)

`X3DColorNode` (6), `setTransparent` (17), `isTransparent` (22)

## `src/x_ite/Components/Rendering/X3DComposedGeometryNode.js` (8; no ledger entry)

`initialize` (20), `checkVertexCount` (40), `getPolygonIndex` (44), `getTriangleIndex` (48), `createPolygons` (52), `createTriangles` (59), `generateNormals` (137), `createNormals` (150)

## `src/x_ite/Components/Rendering/X3DCoordinateNode.js` (6; no ledger entry)

`initialize` (16), `set_point__` (24), `getSize` (29), `set1Point` (33), `addPoint` (52), `addPoints` (67)

## `src/x_ite/Components/Rendering/X3DGeometricPropertyNode.js` (1; no ledger entry)

`X3DGeometricPropertyNode` (4)

## `src/x_ite/Components/Rendering/X3DGeometryNode.js` (47; no ledger entry)

`create` (62), `assign` (68), `getValue` (77), `value` (98), `setGeometryType` (145), `getMin` (163), `getMax` (168), `isSolid` (173), `setSolid` (177), `getCCW` (181), `setCCW` (187), `getCoordIndices` (194), `getAttrib` (198), `getAttribs` (202), `getAttribBuffers` (206), `getFogDepths` (210), `getColors` (214), `getMultiTexCoords` (218), `getTexCoords` (222), `getTextureCoordinate` (226), `setTextureCoordinate` (230), `getTextureCoordinateMapping` (240), `updateTextureCoordinateMapping` (244), `getTangents` (250), `updateVertexArrays` (262), `set_live__` (423), `clear` (490), `transfer` (550), `updateGeometryKey` (633), `updateRenderFunctions` (645), `displaySimple` (699), `displayGeometry` (765), `displaySimpleInstanced` (834), `displayInstanced` (862), `displayInstancedGeometry` (910), `getFogCoord` (997), `getColor` (1001), `getTexCoord` (1005), `getTangent` (1009), `getNormal` (1013), `getCoord` (1017), `set_attrib__` (1021), `set_fogCoord__` (1049), `set_texCoord__` (1067), `set_tangent__` (1077), `set_normal__` (1085), `set_coord__` (1093)

## `src/x_ite/Components/Rendering/X3DLineGeometryNode.js` (7; no ledger entry)

`intersectsLine` (38), `updateVertexArrays` (42), `generateTexCoords` (48), `displaySimple` (107), `displaySimpleThick` (128), `displaySimpleInstanced` (180), `displaySimpleInstancedThick` (210)

## `src/x_ite/Components/Rendering/X3DNormalNode.js` (1; no ledger entry)

`X3DNormalNode` (5)

## `src/x_ite/Components/Rendering/X3DPointGeometryNode.js` (5; no ledger entry)

`intersectsLine` (14), `generateTexCoords` (18), `displaySimple` (20), `displaySimpleInstanced` (117), `displayInstanced` (145)

## `src/x_ite/Components/Rendering/X3DTangentNode.js` (1; no ledger entry)

`X3DTangentNode` (5)

## `src/x_ite/Components/RigidBodyPhysics/BallJoint.js` (5; `node:BallJoint` COMPLETE)

`initialize` (28), `addJoint` (34), `removeJoint` (60), `set_forceOutput__` (70), `set_anchorPoint__` (89)

## `src/x_ite/Components/RigidBodyPhysics/CollidableOffset.js` (3; `node:CollidableOffset` COMPLETE)

`initialize` (18), `set_child__` (27), `set_collidableGeometry__` (53)

## `src/x_ite/Components/RigidBodyPhysics/CollidableShape.js` (7; `node:CollidableShape` COMPLETE)

`initialize` (23), `isConvex` (32), `setConvex` (36), `set_child__` (92), `set_geometry__` (116), `removeCollidableGeometry` (265), `dispose` (286)

## `src/x_ite/Components/RigidBodyPhysics/CollisionCollection.js` (7; `node:CollisionCollection` COMPLETE)

`initialize` (34), `getBBox` (45), `getAppliedParameters` (52), `getCollidables` (56), `set_collidables__` (77), `collect` (99), `dispose` (129)

## `src/x_ite/Components/RigidBodyPhysics/CollisionSensor.js` (4; `node:CollisionSensor` COMPLETE)

`initialize` (21), `set_live__` (31), `set_collider__` (42), `getContact` (177)

## `src/x_ite/Components/RigidBodyPhysics/CollisionSpace.js` (5; `node:CollisionSpace` COMPLETE)

`initialize` (22), `getBBox` (30), `getCollidables` (38), `set_collidables__` (42), `collect` (64)

## `src/x_ite/Components/RigidBodyPhysics/DoubleAxisHingeJoint.js` (3; `node:DoubleAxisHingeJoint` COMPLETE)

`initialize` (52), `removeJoint` (106), `set_forceOutput__` (116)

## `src/x_ite/Components/RigidBodyPhysics/RigidBody.js` (13; `node:RigidBody` COMPLETE)

`initialize` (52), `getBBox` (83), `getCollection` (94), `getRigidBody` (98), `getMatrix` (102), `set_position__` (106), `set_orientation__` (111), `set_forces__` (252), `set_torques__` (259), `set_disable__` (266), `set_geometry__` (277), `set_body__` (333), `dispose` (417)

## `src/x_ite/Components/RigidBodyPhysics/RigidBodyCollection.js` (14; `node:RigidBodyCollection` COMPLETE)

`initialize` (46), `getBBox` (66), `getDynamicsWorld` (73), `set_enabled__` (87), `set_contacts__` (94), `set_contactSurfaceThickness__` (110), `set_collider__` (115), `set_colliderParameters__` (125), `set_bounce__` (159), `set_bodies__` (184), `set_dynamicsWorld__` (224), `set_joints__` (242), `update` (273), `dispose` (313)

## `src/x_ite/Components/RigidBodyPhysics/SingleAxisHingeJoint.js` (3; `node:SingleAxisHingeJoint` COMPLETE)

`initialize` (43), `removeJoint` (98), `set_forceOutput__` (108)

## `src/x_ite/Components/RigidBodyPhysics/SliderJoint.js` (4; `node:SliderJoint` COMPLETE)

`initialize` (34), `removeJoint` (104), `set_forceOutput__` (114), `set_separation__` (133)

## `src/x_ite/Components/RigidBodyPhysics/X3DNBodyCollidableNode.js` (22; no ledger entry)

`getBBox` (57), `getShapes` (64), `setBody` (98), `getBody` (102), `getCompoundShape` (106), `setOffset` (110), `getOffset` (114), `getMatrix` (118), `getChild` (122), `setChild` (126), `set_boundedObjects__` (217), `set_pointingObjects__` (221), `set_cameraObjects__` (225), `set_pickableObjects__` (229), `set_collisionObjects__` (233), `set_shadowObjects__` (237), `set_visibleObjects__` (241), `requestRebuild` (245), `set_child__` (249), `eventsProcessed` (251), `traverse` (259), `dispose` (323)

## `src/x_ite/Components/RigidBodyPhysics/X3DNBodyCollisionSpaceNode.js` (3; no ledger entry)

`X3DNBodyCollisionSpaceNode` (5), `initialize` (16), `dispose` (21)

## `src/x_ite/Components/RigidBodyPhysics/X3DRigidJointNode.js` (18; no ledger entry)

`setCollection` (34), `getCollection` (42), `getBody1` (46), `getBody2` (50), `getInitialInverseMatrix1` (54), `getInitialInverseMatrix2` (58), `setOutput` (62), `addJoint` (83), `removeJoint` (85), `set_forceOutput__` (87), `set_joint__` (89), `set_body1__` (94), `set_body2__` (115), `initialize1` (136), `initialize2` (142), `update1` (148), `update2` (150), `dispose` (152)

## `src/x_ite/Components/Scripting/Script.js` (9; `node:Script` PARTIAL)

`canUserDefinedFields` (57), `addUserDefinedField` (61), `removeUserDefinedField` (71), `getSourceText` (83), `createGlobalObject` (130), `getScriptNode` (132), `SFNode` (152), `evaluate` (307), `shutdown` (328)

## `src/x_ite/Components/Scripting/X3DScriptNode.js` (3; no ledger entry)

`X3DScriptNode` (6), `initialize` (17), `dispose` (22)

## `src/x_ite/Components/Shaders/ComposedShader.js` (4; `node:ComposedShader` COMPLETE)

`addUserDefinedField` (51), `removeUserDefinedField` (63), `getProgram` (79), `dispose` (139)

## `src/x_ite/Components/Shaders/FloatVertexAttribute.js` (5; `node:FloatVertexAttribute` COMPLETE)

`initialize` (18), `set_numComponents__` (29), `set_value__` (33), `addValue` (38), `enable` (65)

## `src/x_ite/Components/Shaders/Matrix3VertexAttribute.js` (4; `node:Matrix3VertexAttribute` COMPLETE)

`initialize` (18), `set_value__` (26), `addValue` (31), `enable` (57)

## `src/x_ite/Components/Shaders/Matrix4VertexAttribute.js` (4; `node:Matrix4VertexAttribute` COMPLETE)

`initialize` (18), `set_value__` (26), `addValue` (31), `enable` (57)

## `src/x_ite/Components/Shaders/PackagedShader.js` (4; `node:PackagedShader` COMPLETE)

`getSourceText` (27), `requestImmediateLoad` (31), `initialize` (33), `dispose` (39)

## `src/x_ite/Components/Shaders/ShaderPart.js` (6; `node:ShaderPart` PARTIAL)

`getSourceText` (54), `getOptions` (58), `setOptions` (62), `getShader` (66), `unloadData` (86), `dispose` (145)

## `src/x_ite/Components/Shaders/ShaderProgram.js` (4; `node:ShaderProgram` COMPLETE)

`getSourceText` (26), `requestImmediateLoad` (30), `initialize` (32), `dispose` (38)

## `src/x_ite/Components/Shaders/X3DProgrammableShaderObject.js` (35; no ledger entry)

`initialize` (39), `canUserDefinedFields` (41), `getAttributeNames` (45), `setAttributeNames` (49), `getUniformNames` (53), `setUniformNames` (57), `getUniformLocation` (323), `getAttribLocation` (347), `removeShaderFields` (547), `getImagesLength` (780), `getLocationLength` (791), `getRenderedTextures` (803), `hasFog` (808), `hasEnvironmentLight` (817), `hasLight` (826), `hasTextureProjector` (835), `setClipPlanes` (844), `enable` (976), `enableUintAttrib` (997), `enableFloatAttrib` (1009), `enableMatrix3Attrib` (1021), `enableMatrix4Attrib` (1040), `enableCoordIndexAttribute` (1059), `enableLineStippleAttribute` (1067), `enableFogDepthAttribute` (1075), `enableColorAttribute` (1083), `enableTexCoordAttribute` (1092), `enableNormalAttribute` (1102), `enableTangentAttribute` (1111), `enableVertexAttribute` (1120), `enableParticleAttribute` (1128), `enableParticleVelocityAttribute` (1137), `enableInstanceMatrixAttribute` (1146), `enableInstanceNormalMatrixAttribute` (1163), `dispose` (1259)

## `src/x_ite/Components/Shaders/X3DShaderNode.js` (1; no ledger entry)

`isValid` (23)

## `src/x_ite/Components/Shaders/X3DVertexAttributeNode.js` (3; no ledger entry)

`X3DVertexAttributeNode` (6), `initialize` (17), `set_attribute__` (23)

## `src/x_ite/Components/Shape/AcousticProperties.js` (1; `node:AcousticProperties` COMPLETE)

`AcousticProperties` (8)

## `src/x_ite/Components/Shape/Appearance.js` (13; `node:Appearance` COMPLETE)

`getAlphaMode` (74), `getMaterial` (86), `getBackMaterial` (90), `getTextureBits` (98), `updateTextureBits` (102), `getTextureTransform` (107), `getTextureTransformMapping` (111), `getShader` (115), `getBackShader` (119), `getRenderModes` (123), `set_contentScale__` (131), `updateTextureTransformMapping` (285), `getShader` (315)

## `src/x_ite/Components/Shape/FillProperties.js` (8; `node:FillProperties` COMPLETE)

`initialize` (23), `getStyleKey` (40), `set_hatched__` (50), `set_hatchColor__` (54), `set_hatchStyle__` (60), `setTransparent` (69), `isTransparent` (74), `setShaderUniforms` (78)

## `src/x_ite/Components/Shape/LineProperties.js` (7; `node:LineProperties` COMPLETE)

`initialize` (17), `getStyleKey` (36), `getApplied` (40), `getLinetype` (44), `getLinewidthScaleFactor` (48), `set_applied__` (56), `set_linetype__` (60)

## `src/x_ite/Components/Shape/Material.js` (13; `node:Material` COMPLETE)

`initialize` (30), `getBaseTexture` (81), `set_ambientIntensity__` (85), `set_ambientTexture__` (89), `set_diffuseColor__` (101), `set_diffuseTexture__` (105), `set_specularColor__` (125), `set_specularTexture__` (129), `set_shininess__` (141), `set_shininessTexture__` (145), `set_occlusionStrength__` (151), `set_occlusionTexture__` (155), `setShaderUniforms` (228)

## `src/x_ite/Components/Shape/PhysicalMaterial.js` (14; `node:PhysicalMaterial` COMPLETE)

`initialize` (24), `isPhysical` (48), `getMaterialKey` (52), `getBaseTexture` (74), `set_baseColor__` (78), `set_baseTexture__` (82), `set_metallic__` (102), `set_roughness__` (106), `set_metallicRoughnessTexture__` (110), `set_occlusionStrength__` (116), `set_occlusionTexture__` (120), `set_extensionsKey__` (161), `createShader` (182), `setShaderUniforms` (219)

## `src/x_ite/Components/Shape/PointProperties.js` (7; `node:PointProperties` COMPLETE)

`initialize` (22), `getStyleKey` (44), `set_contentScale__` (48), `set_pointSizeScaleFactor__` (54), `set_pointSizeMinValue__` (60), `set_pointSizeMaxValue__` (66), `set_attenuation__` (72)

## `src/x_ite/Components/Shape/Shape.js` (3; `node:Shape` COMPLETE)

`getShapeKey` (17), `getNumInstances` (21), `displaySimple` (25)

## `src/x_ite/Components/Shape/TwoSidedMaterial.js` (14; `node:TwoSidedMaterial` COMPLETE)

`initialize` (35), `set_ambientIntensity__` (73), `set_diffuseColor__` (77), `set_specularColor__` (81), `set_emissiveColor__` (85), `set_shininess__` (89), `set_transparency__` (93), `set_backAmbientIntensity__` (100), `set_backDiffuseColor__` (104), `set_backSpecularColor__` (108), `set_backEmissiveColor__` (112), `set_backShininess__` (116), `set_backTransparency__` (120), `setShaderUniforms` (131)

## `src/x_ite/Components/Shape/UnlitMaterial.js` (4; `node:UnlitMaterial` COMPLETE)

`initialize` (17), `getMaterialKey` (25), `set_emissiveTexture__` (43), `createShader` (51)

## `src/x_ite/Components/Shape/X3DAppearanceChildNode.js` (1; no ledger entry)

`X3DAppearanceChildNode` (4)

## `src/x_ite/Components/Shape/X3DAppearanceNode.js` (7; no ledger entry)

`X3DAppearanceNode` (5), `setTransparent` (18), `isTransparent` (23), `setTransmission` (27), `isTransmission` (32), `setVolumeScatter` (36), `isVolumeScatter` (41)

## `src/x_ite/Components/Shape/X3DMaterialNode.js` (10; no ledger entry)

`isPhysical` (29), `setTransparent` (33), `setTransmission` (42), `isTransmission` (47), `setVolumeScatter` (51), `isVolumeScatter` (56), `addTexture` (60), `getTextureBits` (75), `getRenderedTextures` (79), `getShader` (84)

## `src/x_ite/Components/Shape/X3DOneSidedMaterialNode.js` (7; no ledger entry)

`initialize` (20), `set_emissiveColor__` (37), `set_emissiveTexture__` (41), `set_normalTexture__` (57), `set_transparency__` (63), `getBaseTexture` (71), `getShaderOptions` (75)

## `src/x_ite/Components/Shape/X3DShapeNode.js` (22; no ledger entry)

`initialize` (41), `getGeometryType` (59), `getNumInstances` (63), `getBBoxSize` (85), `getBBoxCenter` (89), `isTransparent` (93), `setTransparent` (97), `getAlphaMode` (102), `setAlphaMode` (106), `getRenderPasses` (111), `getAppearance` (115), `getGeometry` (119), `getGeometryContext` (123), `getShapes` (127), `set_appearance__` (153), `set_geometry__` (175), `set_renderPass__` (224), `set_objects__` (231), `set_boundedObject__` (240), `set_pickableObject__` (248), `set_visibleObject__` (260), `dispose` (333)

## `src/x_ite/Components/Sound/Analyser.js` (2; `node:Analyser` COMPLETE)

`initialize` (26), `set_decibels__` (47)

## `src/x_ite/Components/Sound/AudioClip.js` (2; `node:AudioClip` PARTIAL)

`initialize` (23), `dispose` (92)

## `src/x_ite/Components/Sound/AudioDestination.js` (2; `node:AudioDestination` PARTIAL)

`initialize` (25), `set_enabled__` (37)

## `src/x_ite/Components/Sound/BiquadFilter.js` (3; `node:BiquadFilter` COMPLETE)

`initialize` (24), `set_frequency__` (62), `set_qualityFactor__` (66)

## `src/x_ite/Components/Sound/BufferAudioSource.js` (2; `node:BufferAudioSource` PARTIAL)

`initialize` (24), `dispose` (171)

## `src/x_ite/Components/Sound/ChannelMerger.js` (3; `node:ChannelMerger` COMPLETE)

`initialize` (18), `connectChildNode` (44), `disconnectChildNode` (56)

## `src/x_ite/Components/Sound/ChannelSelector.js` (2; `node:ChannelSelector` COMPLETE)

`initialize` (18), `set_channelSelection__` (26)

## `src/x_ite/Components/Sound/ChannelSplitter.js` (2; `node:ChannelSplitter` COMPLETE)

`initialize` (21), `set_outputs__` (30)

## `src/x_ite/Components/Sound/Convolver.js` (3; `node:Convolver` COMPLETE)

`initialize` (24), `set_buffer__` (37), `set_normalize__` (59)

## `src/x_ite/Components/Sound/Delay.js` (4; `node:Delay` COMPLETE)

`initialize` (18), `set_delayTime__` (27), `set_maxDelayTime__` (35), `createSoundProcessor` (45)

## `src/x_ite/Components/Sound/DynamicsCompressor.js` (7; `node:DynamicsCompressor` COMPLETE)

`initialize` (24), `set_attack__` (42), `set_knee__` (46), `set_ratio__` (50), `set_release__` (54), `set_threshold__` (58), `set_time` (62)

## `src/x_ite/Components/Sound/ListenerPointSource.js` (2; `node:ListenerPointSource` COMPLETE)

`initialize` (27), `set_trackCurrentView__` (36)

## `src/x_ite/Components/Sound/MicrophoneSource.js` (6; `node:MicrophoneSource` COMPLETE)

`initialize` (19), `set_mediaDeviceID__` (25), `set_pause` (70), `set_resume` (90), `set_stop` (106), `set_time` (122)

## `src/x_ite/Components/Sound/OscillatorSource.js` (9; `node:OscillatorSource` COMPLETE)

`initialize` (20), `set_frequency__` (37), `set_periodicWave__` (44), `set_periodicWaveOptions__` (93), `set_start` (97), `set_pause` (113), `set_resume` (117), `set_stop` (121), `set_time` (134)

## `src/x_ite/Components/Sound/Sound.js` (5; `node:Sound` PARTIAL)

`getSoundDestination` (97), `setTraversed` (101), `getTraversed` (116), `set_enabled__` (137), `update` (216)

## `src/x_ite/Components/Sound/SpatialSound.js` (15; `node:SpatialSound` DIFFERENT)

`initialize` (37), `getSoundDestination` (90), `getTraversed` (109), `set_enabled__` (113), `set_spatialize__` (136), `set_intensity__` (147), `set_coneOuterGain__` (151), `set_coneInnerAngle__` (155), `set_coneOuterAngle__` (159), `set_maxDistance__` (183), `set_referenceDistance__` (187), `set_enableHRTF__` (191), `set_dopplerEnabled__` (198), `set_children__` (202), `update` (240)

## `src/x_ite/Components/Sound/StreamAudioDestination.js` (3; `node:StreamAudioDestination` COMPLETE)

`initialize` (29), `set_enabled__` (37), `set_mediaDeviceID__` (39)

## `src/x_ite/Components/Sound/StreamAudioSource.js` (1; `node:StreamAudioSource` COMPLETE)

`initialize` (23)

## `src/x_ite/Components/Sound/WaveShaper.js` (2; `node:WaveShaper` COMPLETE)

`initialize` (23), `set_curve__` (35)

## `src/x_ite/Components/Sound/X3DSoundChannelNode.js` (7; no ledger entry)

`initialize` (23), `getAudioDestination` (41), `getAudioSource` (45), `getSoundProcessor` (49), `setSoundProcessor` (53), `set_gain__` (82), `set_channelCount__` (89)

## `src/x_ite/Components/Sound/X3DSoundDestinationNode.js` (5; no ledger entry)

`initialize` (24), `getSoundDestination` (44), `getAudioDestination` (48), `set_gain__` (73), `set_channelCount__` (77)

## `src/x_ite/Components/Sound/X3DSoundNode.js` (1; no ledger entry)

`X3DSoundNode` (5)

## `src/x_ite/Components/Sound/X3DSoundProcessingNode.js` (9; no ledger entry)

`initialize` (29), `getAudioDestination` (47), `getAudioSource` (51), `getSoundProcessor` (55), `set_gain__` (90), `set_channelCount__` (97), `set_start` (169), `set_time` (173), `dispose` (177)

## `src/x_ite/Components/Sound/X3DSoundSourceNode.js` (7; no ledger entry)

`getAudioSource` (31), `getMediaElement` (35), `set_pause` (103), `set_resume` (107), `set_stop` (111), `set_end` (115), `dispose` (134)

## `src/x_ite/Components/Text/FontLibrary.js` (1; `node:FontLibrary` PARTIAL)

`dispose` (82)

## `src/x_ite/Components/Text/FontStyle.js` (3; `node:FontStyle` COMPLETE)

`createTextGeometry` (22), `getScale` (26), `getContentScale` (30)

## `src/x_ite/Components/Text/Text.js` (3; `node:Text` COMPLETE)

`traverseBefore` (77), `traverseAfter` (81), `displayInstanced` (93)

## `src/x_ite/Components/Text/X3DFontStyleNode.js` (6; no ledger entry)

`getCache` (89), `getAllowEmptyUrl` (93), `getFont` (97), `getMajorAlignment` (107), `getMinorAlignment` (111), `dispose` (219)

## `src/x_ite/Components/TextureProjection/TextureProjector.js` (9; `node:TextureProjector` COMPLETE)

`TextureProjectorContainer` (16), `set` (34), `renderShadowMap` (44), `setGlobalVariables` (85), `setShaderUniforms` (94), `dispose` (127), `initialize` (156), `getFieldOfView` (160), `getLights` (166)

## `src/x_ite/Components/TextureProjection/TextureProjectorParallel.js` (15; `node:TextureProjectorParallel` COMPLETE)

`TextureProjectorParallelContainer` (16), `set` (34), `renderShadowMap` (44), `setGlobalVariables` (106), `setShaderUniforms` (115), `dispose` (148), `initialize` (177), `set_fieldOfView___` (185), `getMinimumX` (195), `getMinimumY` (199), `getMaximumX` (203), `getMaximumY` (207), `getSizeX` (211), `getSizeY` (215), `getLights` (219)

## `src/x_ite/Components/TextureProjection/X3DTextureProjectorNode.js` (12; no ledger entry)

`getLightKey` (36), `getGlobal` (40), `getLocation` (44), `getDirection` (48), `getNearDistance` (52), `getNearParameter` (56), `getFarDistance` (60), `getFarParameter` (64), `getTexture` (68), `straightenHorizon` (85), `set_nearDistance__` (89), `set_farDistance__` (96)

## `src/x_ite/Components/Texturing/ImageTexture.js` (5; `node:ImageTexture` COMPLETE)

`initialize` (26), `getTextureType` (35), `setKTXTexture` (92), `updateOutputs` (131), `dispose` (137)

## `src/x_ite/Components/Texturing/MovieTexture.js` (7; `node:MovieTexture` PARTIAL)

`set_live__` (48), `getTextureType` (53), `on_error` (84), `onloadeddata` (158), `onabort` (161), `hasAudioTracks` (216), `dispose` (258)

## `src/x_ite/Components/Texturing/MultiTexture.js` (4; `node:MultiTexture` COMPLETE)

`initialize` (35), `getCount` (53), `updateTextureBits` (158), `getShaderOptions` (175)

## `src/x_ite/Components/Texturing/MultiTextureCoordinate.js` (9; `node:MultiTextureCoordinate` COMPLETE)

`initialize` (25), `set_texCoord__` (33), `getCount` (53), `init` (57), `addPoint` (66), `addPoints` (75), `getTextureCoordinates` (82), `getTextureCoordinateMapping` (86), `setShaderUniforms` (95)

## `src/x_ite/Components/Texturing/MultiTextureTransform.js` (6; `node:MultiTextureTransform` COMPLETE)

`initialize` (25), `set_textureTransform__` (33), `getCount` (50), `getTextureTransformMapping` (54), `setShaderUniforms` (63), `transformPoint` (72)

## `src/x_ite/Components/Texturing/PixelTexture.js` (4; `node:PixelTexture` COMPLETE)

`initialize` (19), `getTextureType` (27), `checkLoadState` (31), `convert` (35)

## `src/x_ite/Components/Texturing/RenderedTexture.js` (4; `node:RenderedTexture` COMPLETE)

`getTextureType` (55), `isRenderedTexture` (59), `checkLoadState` (67), `dispose` (291)

## `src/x_ite/Components/Texturing/TextureCoordinate.js` (5; `node:TextureCoordinate` COMPLETE)

`initialize` (18), `set_point__` (26), `getSize` (31), `get1Point` (35), `addPoints` (74)

## `src/x_ite/Components/Texturing/TextureCoordinateGenerator.js` (5; `node:TextureCoordinateGenerator` COMPLETE)

`initialize` (23), `set_parameter__` (56), `addPointToChannel` (67), `addPoints` (71), `setShaderUniforms` (75)

## `src/x_ite/Components/Texturing/TextureProperties.js` (4; `node:TextureProperties` COMPLETE)

`getBorderWidth` (26), `getBoundaryModeS` (46), `getBoundaryModeT` (50), `getBoundaryModeR` (54)

## `src/x_ite/Components/Texturing/TextureTransform.js` (1; `node:TextureTransform` COMPLETE)

`initialize` (28)

## `src/x_ite/Components/Texturing/X3DSingleTextureCoordinateNode.js` (4; no ledger entry)

`getCount` (15), `init` (19), `addPoint` (24), `setShaderUniforms` (32)

## `src/x_ite/Components/Texturing/X3DSingleTextureNode.js` (19; no ledger entry)

`initialize` (24), `getCount` (36), `getTextureTypeString` (40), `getTexture` (50), `setTexture` (54), `setLinear` (68), `canMipMaps` (73), `setMipMaps` (77), `isRenderedTexture` (86), `getRenderedTextures` (90), `loadImage` (92), `onload` (98), `onabort` (101), `isImageTransparent` (107), `set_textureProperties__` (119), `getTextureBits` (186), `updateTextureBits` (190), `getShaderOptions` (194), `setNamedShaderUniforms` (219)

## `src/x_ite/Components/Texturing/X3DSingleTextureTransformNode.js` (7; no ledger entry)

`X3DSingleTextureTransformNode` (6), `getCount` (19), `getMatrix` (23), `setMatrix` (27), `getTextureTransformMapping` (31), `setShaderUniforms` (35), `transformPoint` (39)

## `src/x_ite/Components/Texturing/X3DTexture2DNode.js` (5; no ledger entry)

`getTarget` (31), `getTextureData` (67), `updateTextureData` (104), `updateTextureParameters` (116), `setShaderUniforms` (128)

## `src/x_ite/Components/Texturing/X3DTextureCoordinateNode.js` (1; no ledger entry)

`X3DTextureCoordinateNode` (5)

## `src/x_ite/Components/Texturing/X3DTextureNode.js` (3; no ledger entry)

`X3DTextureNode` (6), `setTransparent` (17), `isTransparent` (22)

## `src/x_ite/Components/Texturing/X3DTextureTransformNode.js` (1; no ledger entry)

`X3DTextureTransformNode` (5)

## `src/x_ite/Components/Texturing3D/ComposedTexture3D.js` (1; `node:ComposedTexture3D` COMPLETE)

`isComplete` (53)

## `src/x_ite/Components/Texturing3D/ImageTexture3D.js` (4; `node:ImageTexture3D` COMPLETE)

`initialize` (24), `getInternalType` (31), `setKTXTexture` (114), `dispose` (144)

## `src/x_ite/Components/Texturing3D/ImageTextureAtlas.js` (2; `node:ImageTextureAtlas` COMPLETE)

`initialize` (26), `dispose` (146)

## `src/x_ite/Components/Texturing3D/PixelTexture3D.js` (1; `node:PixelTexture3D` COMPLETE)

`initialize` (17)

## `src/x_ite/Components/Texturing3D/TextureCoordinate3D.js` (6; `node:TextureCoordinate3D` COMPLETE)

`initialize` (18), `set_point__` (26), `getSize` (31), `get1Point` (35), `addPointToChannel` (59), `addPoints` (74)

## `src/x_ite/Components/Texturing3D/TextureCoordinate4D.js` (7; `node:TextureCoordinate4D` COMPLETE)

`initialize` (17), `set_point__` (25), `getSize` (30), `get1Point` (34), `set1Point` (49), `addPointToChannel` (53), `addPoints` (68)

## `src/x_ite/Components/Texturing3D/TextureTransform3D.js` (1; `node:TextureTransform3D` COMPLETE)

`initialize` (22)

## `src/x_ite/Components/Texturing3D/TextureTransformMatrix3D.js` (1; `node:TextureTransformMatrix3D` COMPLETE)

`initialize` (17)

## `src/x_ite/Components/Texturing3D/X3DTexture3DNode.js` (10; no ledger entry)

`initialize` (20), `getTarget` (30), `getTextureType` (34), `getWidth` (38), `setWidth` (42), `getHeight` (46), `setHeight` (50), `getDepth` (54), `setDepth` (58), `setShaderUniforms` (111)

## `src/x_ite/Components/Time/TimeSensor.js` (4; `node:TimeSensor` COMPLETE)

`set_cycleInterval__` (59), `set_pause` (95), `set_stop` (108), `dispose` (155)

## `src/x_ite/Components/Time/X3DTimeDependentNode.js` (15; no ledger entry)

`set_loop__` (94), `set_startTime__` (110), `set_pauseTime__` (125), `set_resumeTime__` (143), `set_stopTime__` (161), `do_pause` (203), `do_resume` (221), `do_stop` (242), `set_loop` (285), `set_start` (286), `set_pause` (287), `set_resume` (288), `set_stop` (289), `set_time` (290), `dispose` (291)

## `src/x_ite/Components/VolumeRendering/BlendedVolumeStyle.js` (10; `node:BlendedVolumeStyle` PARTIAL)

`initialize` (18), `addVolumeData` (32), `removeVolumeData` (38), `set_weightTransferFunction1__` (44), `set_weightTransferFunction2__` (48), `set_voxels__` (72), `addShaderFields` (76), `getDefines` (99), `getUniformsText` (103), `getFunctionsText` (246)

## `src/x_ite/Components/VolumeRendering/BoundaryEnhancementVolumeStyle.js` (3; `node:BoundaryEnhancementVolumeStyle` COMPLETE)

`addShaderFields` (17), `getUniformsText` (26), `getFunctionsText` (57)

## `src/x_ite/Components/VolumeRendering/CartoonVolumeStyle.js` (5; `node:CartoonVolumeStyle` COMPLETE)

`initialize` (18), `addShaderFields` (30), `getDefines` (42), `getUniformsText` (46), `getFunctionsText` (91)

## `src/x_ite/Components/VolumeRendering/ComposedVolumeStyle.js` (7; `node:ComposedVolumeStyle` COMPLETE)

`initialize` (20), `addVolumeData` (28), `removeVolumeData` (35), `addShaderFields` (72), `getDefines` (80), `getUniformsText` (85), `getFunctionsText` (109)

## `src/x_ite/Components/VolumeRendering/EdgeEnhancementVolumeStyle.js` (4; `node:EdgeEnhancementVolumeStyle` COMPLETE)

`initialize` (18), `addShaderFields` (30), `getUniformsText` (41), `getFunctionsText` (78)

## `src/x_ite/Components/VolumeRendering/IsoSurfaceVolumeData.js` (2; `node:IsoSurfaceVolumeData` COMPLETE)

`initialize` (22), `set_voxels__` (70)

## `src/x_ite/Components/VolumeRendering/OpacityMapVolumeStyle.js` (4; `node:OpacityMapVolumeStyle` COMPLETE)

`initialize` (18), `addShaderFields` (32), `getUniformsText` (39), `getFunctionsText` (75)

## `src/x_ite/Components/VolumeRendering/ProjectionVolumeStyle.js` (4; `node:ProjectionVolumeStyle` COMPLETE)

`addShaderFields` (17), `getDefines` (24), `getUniformsText` (28), `getFunctionsText` (135)

## `src/x_ite/Components/VolumeRendering/SegmentedVolumeData.js` (4; `node:SegmentedVolumeData` COMPLETE)

`initialize` (22), `getSegmentEnabled` (39), `set_segmentIdentifiers__` (43), `set_voxels__` (73)

## `src/x_ite/Components/VolumeRendering/ShadedVolumeStyle.js` (6; `node:ShadedVolumeStyle` PARTIAL)

`initialize` (18), `set_material__` (28), `addShaderFields` (42), `getDefines` (60), `getUniformsText` (64), `getFunctionsText` (168)

## `src/x_ite/Components/VolumeRendering/SilhouetteEnhancementVolumeStyle.js` (4; `node:SilhouetteEnhancementVolumeStyle` COMPLETE)

`initialize` (18), `addShaderFields` (30), `getUniformsText` (42), `getFunctionsText` (76)

## `src/x_ite/Components/VolumeRendering/ToneMappedVolumeStyle.js` (4; `node:ToneMappedVolumeStyle` COMPLETE)

`initialize` (18), `addShaderFields` (30), `getUniformsText` (41), `getFunctionsText` (86)

## `src/x_ite/Components/VolumeRendering/VolumeData.js` (3; `node:VolumeData` COMPLETE)

`initialize` (20), `set_voxels__` (49), `createShader` (55)

## `src/x_ite/Components/VolumeRendering/X3DComposableVolumeRenderStyleNode.js` (1; no ledger entry)

`X3DComposableVolumeRenderStyleNode` (5)

## `src/x_ite/Components/VolumeRendering/X3DVolumeDataNode.js` (13; no ledger entry)

`initialize` (50), `getBBox` (134), `getAppearance` (141), `updateShader` (145), `addShaderUniforms` (149), `getPoints` (165), `set_live__` (185), `set_dimensions__` (205), `set_textureTransform__` (235), `set_active__` (239), `update` (255), `setShaderUniforms` (271), `dispose` (275)

## `src/x_ite/Components/VolumeRendering/X3DVolumeRenderStyleNode.js` (7; no ledger entry)

`addShaderFields` (15), `getDefines` (17), `getUniformsText` (19), `getFunctionsText` (23), `getVolumeData` (27), `addVolumeData` (31), `removeVolumeData` (35)

## `src/x_ite/Components/X_ITE/AnisotropyMaterialExtension.js` (8; `node:AnisotropyMaterialExtension` COMPLETE)

`initialize` (42), `set_anisotropyStrength__` (54), `set_anisotropyRotation__` (58), `set_anisotropyTexture__` (65), `getExtensionKey` (71), `getShaderOptions` (75), `getShaderUniforms` (86), `setShaderUniforms` (90)

## `src/x_ite/Components/X_ITE/BlendMode.js` (7; `node:BlendMode` COMPLETE)

`set_sourceColorFactor__` (70), `set_sourceAlphaFactor__` (75), `set_destinationColorFactor__` (80), `set_destinationAlphaFactor__` (85), `set_colorEquation__` (90), `set_alphaEquation__` (95), `disable` (108)

## `src/x_ite/Components/X_ITE/ClearcoatMaterialExtension.js` (10; `node:ClearcoatMaterialExtension` COMPLETE)

`initialize` (36), `set_clearcoat__` (52), `set_clearcoatTexture__` (56), `set_clearcoatRoughness__` (62), `set_clearcoatRoughnessTexture__` (66), `set_clearcoatNormalTexture__` (72), `getExtensionKey` (78), `getShaderOptions` (82), `getShaderUniforms` (95), `setShaderUniforms` (100)

## `src/x_ite/Components/X_ITE/DepthMode.js` (1; `node:DepthMode` COMPLETE)

`initialize` (23)

## `src/x_ite/Components/X_ITE/DiffuseTransmissionMaterialExtension.js` (9; `node:DiffuseTransmissionMaterialExtension` COMPLETE)

`initialize` (37), `set_diffuseTransmission__` (51), `set_diffuseTransmissionTexture__` (55), `set_diffuseTransmissionColor__` (61), `set_diffuseTransmissionColorTexture__` (65), `getExtensionKey` (71), `getShaderOptions` (75), `getShaderUniforms` (87), `setShaderUniforms` (92)

## `src/x_ite/Components/X_ITE/DispersionMaterialExtension.js` (5; `node:DispersionMaterialExtension` COMPLETE)

`initialize` (26), `getExtensionKey` (38), `getShaderOptions` (42), `getShaderUniforms` (46), `setShaderUniforms` (50)

## `src/x_ite/Components/X_ITE/EmissiveStrengthMaterialExtension.js` (6; `node:EmissiveStrengthMaterialExtension` COMPLETE)

`initialize` (26), `set_emissiveStrength__` (34), `getExtensionKey` (38), `getShaderOptions` (42), `getShaderUniforms` (46), `setShaderUniforms` (50)

## `src/x_ite/Components/X_ITE/IORMaterialExtension.js` (6; `node:IORMaterialExtension` COMPLETE)

`initialize` (26), `set_indexOfRefraction__` (34), `getExtensionKey` (38), `getShaderOptions` (42), `getShaderUniforms` (46), `setShaderUniforms` (50)

## `src/x_ite/Components/X_ITE/InstancedShape.js` (5; `node:InstancedShape` COMPLETE)

`initialize` (39), `getShapeKey` (62), `getNumInstances` (66), `getInstances` (70), `set_transform__` (107)

## `src/x_ite/Components/X_ITE/IridescenceMaterialExtension.js` (11; `node:IridescenceMaterialExtension` COMPLETE)

`initialize` (42), `set_iridescence__` (60), `set_iridescenceTexture__` (64), `set_iridescenceIndexOfRefraction__` (70), `set_iridescenceThicknessMinimum__` (74), `set_iridescenceThicknessMaximum__` (78), `set_iridescenceThicknessTexture__` (82), `getExtensionKey` (88), `getShaderOptions` (92), `getShaderUniforms` (104), `setShaderUniforms` (111)

## `src/x_ite/Components/X_ITE/RetroreflectionMaterialExtension.js` (6; `node:RetroreflectionMaterialExtension` COMPLETE)

`initialize` (34), `set_retroreflectionTexture__` (48), `getExtensionKey` (54), `getShaderOptions` (58), `getShaderUniforms` (69), `setShaderUniforms` (73)

## `src/x_ite/Components/X_ITE/SheenMaterialExtension.js` (9; `node:SheenMaterialExtension` COMPLETE)

`initialize` (37), `set_sheenColor__` (55), `set_sheenColorTexture__` (59), `set_sheenRoughness__` (65), `set_sheenRoughnessTexture__` (69), `getExtensionKey` (75), `getShaderOptions` (79), `getShaderUniforms` (91), `setShaderUniforms` (97)

## `src/x_ite/Components/X_ITE/SpecularGlossinessMaterial.js` (13; `node:SpecularGlossinessMaterial` COMPLETE)

`initialize` (44), `isPhysical` (66), `getMaterialKey` (70), `getBaseTexture` (91), `set_diffuseColor__` (95), `set_diffuseTexture__` (99), `set_specularColor__` (119), `set_glossiness__` (123), `set_specularGlossinessTexture__` (127), `set_occlusionStrength__` (133), `set_occlusionTexture__` (137), `createShader` (143), `setShaderUniforms` (169)

## `src/x_ite/Components/X_ITE/SpecularMaterialExtension.js` (10; `node:SpecularMaterialExtension` COMPLETE)

`initialize` (37), `set_specular__` (53), `set_specularTexture__` (57), `set_specularColor__` (63), `set_specularStrength__` (67), `set_specularColorTexture__` (71), `getExtensionKey` (77), `getShaderOptions` (81), `getShaderUniforms` (93), `setShaderUniforms` (99)

## `src/x_ite/Components/X_ITE/TransmissionMaterialExtension.js` (6; `node:TransmissionMaterialExtension` COMPLETE)

`initialize` (34), `set_transmissionTexture__` (48), `getExtensionKey` (54), `getShaderOptions` (58), `getShaderUniforms` (69), `setShaderUniforms` (74)

## `src/x_ite/Components/X_ITE/VolumeMaterialExtension.js` (9; `node:VolumeMaterialExtension` COMPLETE)

`initialize` (42), `set_thickness__` (56), `set_thicknessTexture__` (60), `set_attenuationDistance__` (66), `set_attenuationColor__` (72), `getExtensionKey` (76), `getShaderOptions` (80), `getShaderUniforms` (91), `setShaderUniforms` (97)

## `src/x_ite/Components/X_ITE/VolumeScatterMaterialExtension.js` (9; `node:VolumeScatterMaterialExtension` COMPLETE)

`initialize` (48), `set_scatterStrength__` (64), `set_scatterStrengthTexture__` (68), `set_scatterAnisotropy__` (74), `set_multiscatterColorTexture__` (82), `getExtensionKey` (88), `getShaderOptions` (92), `getShaderUniforms` (105), `setShaderUniforms` (116)

## `src/x_ite/Components/X_ITE/X3DMaterialExtensionNode.js` (2; no ledger entry)

`getTextureBits` (37), `getRenderedTextures` (41)

## `src/x_ite/Configuration/AbstractNodesArray.js` (2; no ledger entry)

`AbstractNodesArray` (4), `add` (11)

## `src/x_ite/Configuration/ComponentInfo.js` (3; no ledger entry)

`toVRMLStream` (19), `toXMLStream` (30), `providerUrl` (62)

## `src/x_ite/Configuration/ComponentInfoArray.js` (2; no ledger entry)

`ComponentInfoArray` (4), `add` (11)

## `src/x_ite/Configuration/ConcreteNodesArray.js` (1; no ledger entry)

`update` (19)

## `src/x_ite/Configuration/FieldTypesArray.js` (1; no ledger entry)

`FieldTypesArray` (3)

## `src/x_ite/Configuration/ProfileInfo.js` (5; no ledger entry)

`ProfileInfo` (4), `toVRMLStream` (17), `toXMLStream` (24), `toJSONStream` (28), `providerUrl` (50)

## `src/x_ite/Configuration/ProfileInfoArray.js` (2; no ledger entry)

`ProfileInfoArray` (5), `add` (12)

## `src/x_ite/Configuration/SupportedProfiles.js` (1; no ledger entry)

`getComponent` (6)

## `src/x_ite/Configuration/UnitInfo.js` (3; no ledger entry)

`toXMLStream` (26), `toJSONStream` (34), `get` (52)

## `src/x_ite/Configuration/UnitInfoArray.js` (1; no ledger entry)

`UnitInfoArray` (4)

## `src/x_ite/Execution/BindableList.js` (4; no ledger entry)

`get` (17), `push` (79), `update` (83), `equals` (135)

## `src/x_ite/Execution/BindableStack.js` (2; no ledger entry)

`get` (12), `top` (16)

## `src/x_ite/Execution/ExportedNodesArray.js` (1; no ledger entry)

`ExportedNodesArray` (4)

## `src/x_ite/Execution/ImportedNodesArray.js` (1; no ledger entry)

`ImportedNodesArray` (4)

## `src/x_ite/Execution/NamedNodesArray.js` (1; no ledger entry)

`NamedNodesArray` (4)

## `src/x_ite/Execution/X3DExecutionContext.js` (22; `sub:exec.context-api` PARTIAL)

`collectCloneCount` (41), `setExecutionContext` (70), `getCountPrimitives` (93), `setCountPrimitives` (97), `getBBox` (101), `getEncoding` (112), `getProfile` (124), `getComponents` (132), `fromUnit` (136), `toUnit` (140), `getUnits` (144), `setRootNodes` (441), `getRootNodes` (443), `addProtoDeclaration` (458), `removeProtoDeclaration` (512), `getProtoDeclarations` (520), `addExternProtoDeclaration` (539), `getExternProtoDeclarations` (601), `getRoutes` (746), `getWorldInfos` (750), `addWorldInfo` (754), `removeWorldInfo` (758)

## `src/x_ite/Execution/X3DExportedNode.js` (6; no ledger entry)

`getExecutionContext` (22), `getExportedName` (26), `getLocalNode` (30), `getDescription` (34), `dispose` (103), `localNode` (123)

## `src/x_ite/Execution/X3DImportedNode.js` (11; no ledger entry)

`getExecutionContext` (30), `getInlineNode` (34), `getExportedName` (38), `getImportedName` (51), `setImportName` (55), `getInstance` (61), `refreshInstance` (65), `getDescription` (74), `inlineNode` (178), `exportedNode` (191), `instance` (204)

## `src/x_ite/Execution/X3DScene.js` (19; `sub:exec.scene` COMPLETE)

`getLocalScene` (66), `setSpecificationVersion` (70), `getSpecificationVersion` (74), `setWorldURL` (86), `getWorldURL` (90), `getBaseURL` (94), `setProfile` (104), `getProfile` (110), `removeComponent` (141), `getComponents` (147), `getUnit` (162), `getUnits` (166), `getMetaData` (267), `getMetaDatas` (280), `getLoadingObjects` (700), `profile_changed` (753), `metadata_changed` (778), `sceneGraph_changed` (795), `bbox_changed` (801)

## `src/x_ite/Execution/X3DWorld.js` (4; no ledger entry)

`initialize` (23), `getCache` (42), `getLayerSet` (46), `getLayer0` (54)

## `src/x_ite/Features.js` (9; no ledger entry)

`ENVIRONMENT` (7), `at` (29), `constructor` (68), `deref` (73), `register` (87), `unregister` (90), `getOrInsert` (105), `getOrInsertComputed` (118), `value` (139)

## `src/x_ite/Fields/ArrayFields.js` (20; no ledger entry)

`MFNode` (33), `getSingleType` (40), `toStream` (60), `toVRMLStream` (101), `dispose` (234), `MFString` (250), `getSingleType` (257), `toXMLStream` (261), `MFImage` (296), `getSingleType` (303), `setValue` (307), `flat` (337), `sort` (341), `ArrayField` (359), `getSingleValue` (366), `getSingleType` (370), `getValueType` (374), `getArrayType` (378), `getComponents` (382), `Value` (396)

## `src/x_ite/Fields/SFColor.js` (22; no ledger entry)

`SFColor` (4), `[Symbol.iterator]` (11), `copy` (15), `equals` (19), `isDefaultValue` (23), `set` (27), `getHSV` (31), `setHSV` (35), `linearToSRGB` (40), `sRGBToLinear` (44), `toStream` (69), `toVRMLStream` (83), `toXMLStream` (87), `toJSONStream` (91), `toJSONStreamValue` (101), `get` (122), `set` (126), `get` (134), `set` (138), `get` (146), `set` (150), `fromHSV` (179)

## `src/x_ite/Fields/SFColorRGBA.js` (17; no ledger entry)

`SFColorRGBA` (15), `[Symbol.iterator]` (22), `copy` (26), `isDefaultValue` (31), `getHSVA` (36), `setHSVA` (40), `linearToSRGB` (45), `sRGBToLinear` (49), `get` (85), `set` (89), `get` (97), `set` (101), `get` (109), `set` (113), `get` (121), `set` (125), `fromHSVA` (157)

## `src/x_ite/Fields/SFImage.js` (26; no ledger entry)

`Image` (11), `assign` (33), `getWidth` (41), `setWidth` (45), `getHeight` (49), `setHeight` (53), `getComp` (57), `setComp` (61), `getArray` (65), `setArray` (69), `[Symbol.iterator]` (88), `copy` (104), `equals` (108), `isDefaultValue` (112), `set` (119), `toVRMLStream` (160), `toXMLStream` (164), `toJSONStream` (168), `get` (234), `set` (238), `get` (246), `set` (250), `get` (258), `set` (262), `get` (270), `set` (274)

## `src/x_ite/Fields/SFMatrix3.js` (6; no ledger entry)

`SFMatrix3Template` (6), `SFMatrix3` (8), `setTransform` (19), `rotate` (27), `skewX` (31), `skewY` (35)

## `src/x_ite/Fields/SFMatrix4.js` (2; no ledger entry)

`SFMatrix4Template` (5), `SFMatrix4` (7)

## `src/x_ite/Fields/SFMatrixPrototypeTemplate.js` (28; no ledger entry)

`SFMatrixPrototypeTemplate` (3), `[Symbol.iterator]` (11), `copy` (15), `equals` (19), `isDefaultValue` (23), `set` (27), `setTransform` (31), `getTransform` (39), `determinant` (52), `transpose` (56), `inverse` (60), `multLeft` (64), `multRight` (68), `multVecMatrix` (72), `multMatrixVec` (76), `multDirMatrix` (80), `multMatrixDir` (84), `translate` (88), `rotate` (92), `scale` (96), `toStream` (100), `toVRMLStream` (114), `toXMLStream` (118), `toJSONStream` (122), `toJSONStreamValue` (132), `defineProperty` (153), `get` (157), `set` (161)

## `src/x_ite/Fields/SFNode.js` (22; no ledger entry)

`get` (11), `set` (34), `has` (56), `ownKeys` (67), `getOwnPropertyDescriptor` (81), `equals` (154), `isDefaultValue` (163), `set` (167), `getNodeTypeName` (188), `getNodeName` (199), `getNodeDisplayName` (210), `getNodeType` (221), `getFieldDefinition` (232), `getFieldDefinitions` (240), `addFieldCallback` (281), `removeFieldCallback` (306), `getNodeUserData` (331), `setNodeUserData` (340), `removeNodeUserData` (349), `valueOf` (358), `toStream` (366), `toVRMLStream` (377)

## `src/x_ite/Fields/SFNodeCache.js` (2; no ledger entry)

`set` (24), `delete` (28)

## `src/x_ite/Fields/SFQuaternion.js` (37; no ledger entry)

`SFQuaternion` (7), `[Symbol.iterator]` (14), `copy` (18), `equals` (22), `isDefaultValue` (26), `set` (30), `add` (34), `conjugate` (38), `divide` (42), `dot` (46), `getMatrix` (50), `inverse` (54), `length` (58), `multiply` (62), `multLeft` (66), `multQuatVec` (70), `multRight` (74), `multVecQuat` (78), `negate` (82), `normalize` (86), `setMatrix` (90), `slerp` (95), `subtract` (99), `toStream` (103), `toVRMLStream` (115), `toXMLStream` (119), `toJSONStream` (123), `toJSONStreamValue` (133), `get` (154), `set` (158), `get` (166), `set` (170), `get` (178), `set` (182), `get` (190), `set` (194), `fromMatrix` (233)

## `src/x_ite/Fields/SFRotation.js` (32; no ledger entry)

`SFRotation` (11), `[Symbol.iterator]` (34), `copy` (38), `equals` (42), `isDefaultValue` (46), `set` (50), `getAxis` (54), `getMatrix` (58), `getQuaternion` (62), `inverse` (66), `multiply` (70), `multVec` (74), `setAxis` (78), `setMatrix` (83), `setQuaternion` (88), `slerp` (93), `straighten` (97), `toStream` (101), `toVRMLStream` (113), `toXMLStream` (117), `toJSONStream` (121), `toJSONStreamValue` (131), `get` (152), `set` (156), `get` (164), `set` (168), `get` (176), `set` (180), `get` (188), `set` (192), `fromMatrix` (222), `fromQuaternion` (233)

## `src/x_ite/Fields/SFScalar.js` (33; no ledger entry)

`SFBoolTemplate` (3), `SFBool` (5), `isDefaultValue` (12), `set` (16), `toStream` (20), `toXMLStream` (24), `toJSONStreamValue` (28), `SFNumberTemplate` (35), `SFNumber` (39), `isDefaultValue` (46), `set` (50), `toStream` (54), `toJSONStreamValue` (60), `SFInt32Template` (69), `SFInt32` (71), `isDefaultValue` (78), `set` (82), `SFStringTemplate` (89), `SFString` (91), `[Symbol.iterator]` (98), `isDefaultValue` (102), `set` (106), `toStream` (110), `toXMLStream` (116), `toJSONStreamValue` (122), `get` (132), `SFScalarPrototypeTemplate` (153), `copy` (159), `toStream` (164), `toVRMLStream` (168), `toXMLStream` (172), `toJSONStream` (176), `toJSONStreamValue` (180)

## `src/x_ite/Fields/SFVec2.js` (2; no ledger entry)

`SFVec2Template` (5), `SFVec2` (7)

## `src/x_ite/Fields/SFVec3.js` (3; no ledger entry)

`SFVec3Template` (5), `SFVec3` (7), `cross` (14)

## `src/x_ite/Fields/SFVec4.js` (3; no ledger entry)

`SFVec4Template` (5), `SFVec4` (7), `isDefaultValue` (14)

## `src/x_ite/Fields/SFVecPrototypeTemplate.js` (37; no ledger entry)

`SFVecPrototypeTemplate` (3), `[Symbol.iterator]` (11), `copy` (15), `equals` (19), `isDefaultValue` (23), `set` (27), `abs` (31), `add` (35), `clamp` (39), `distance` (43), `divide` (47), `divVec` (51), `dot` (55), `inverse` (59), `length` (63), `lerp` (67), `max` (71), `min` (75), `multiply` (79), `multVec` (83), `negate` (87), `normalize` (91), `reflect` (95), `subtract` (99), `toStream` (103), `toVRMLStream` (118), `toXMLStream` (122), `toJSONStream` (126), `toJSONStreamValue` (136), `get` (159), `set` (163), `get` (171), `set` (175), `get` (183), `set` (187), `get` (195), `set` (199)

## `src/x_ite/InputOutput/FileLoader.js` (5; `sub:io.file-loader` PARTIAL)

`constructor` (53), `isPrivate` (68), `getBlob` (318), `checkResponse` (352), `describe` (430)

## `src/x_ite/InputOutput/JSONGenerator.js` (3; no ledger entry)

`RemoveComma` (39), `endObject` (115), `endArray` (129)

## `src/x_ite/InputOutput/X3DGenerator.js` (15; no ledger entry)

`Generator` (5), `Style` (34), `Break` (103), `NeedsSpace` (161), `createFloatExponentialFormat` (182), `PushExecutionContext` (224), `PopExecutionContext` (242), `ExportedNodes` (277), `ImportedNodes` (284), `AddRouteNode` (291), `ExistsRouteNode` (297), `IsSharedNode` (303), `AddNode` (307), `ExistsNode` (315), `AccessType` (390)

## `src/x_ite/InputOutput/XMLGenerator.js` (7; no ledger entry)

`PushContainerField` (16), `PopContainerField` (20), `openingTag` (63), `closingTag` (70), `openTag` (77), `endTag` (83), `attribute` (101)

## `src/x_ite/Namespace.js` (1; no ledger entry)

`value` (5)

## `src/x_ite/Parser/Expressions.js` (3; no ledger entry)

`parse` (3), `lookahead` (17), `Expressions` (29)

## `src/x_ite/Parser/GLB2Parser.js` (6; `sub:parse.glb2` PARTIAL)

`getEncoding` (15), `setInput` (19), `isValid` (24), `parseIntoScene` (45), `glb` (51), `chunks` (71)

## `src/x_ite/Parser/GLTF2Parser.js` (145; `sub:parse.gltf2` PARTIAL)

`getEncoding` (64), `setInput` (68), `setBuffers` (120), `parseIntoScene` (124), `rootObject` (130), `assetObject` (188), `extensionsArray` (235), `getLibrary` (315), `extensionsObject` (327), `extLightsImageBasedObject` (352), `envLightsArray` (359), `envLightObject` (366), `khrLightsPunctualObject` (440), `lightsArray` (447), `lightObject` (454), `lightType` (488), `directionalLight` (500), `spotLight` (508), `pointLight` (524), `khrMaterialsVariantsObjectVariants` (536), `khrXmpJsonLdObject` (548), `buffersArray` (595), `bufferViewsArray` (620), `getBuffer` (628), `accessorsArray` (700), `get` (743), `denseArray` (770), `normalizedArray` (832), `samplersArray` (857), `get` (896), `imagesArray` (938), `videosArray` (945), `imageObject` (952), `blobToDataUrl` (976), `texturesArray` (989), `textureObject` (996), `textureVideoObject` (1049), `textureImageObject` (1058), `textureImageExtensionsObject` (1068), `materialsArray` (1088), `materialObject` (1095), `getTexCoordIndices` (1172), `createMaterial` (1185), `pbrMetallicRoughnessObject` (1201), `pbrSpecularGlossinessObject` (1232), `occlusionTextureInfo` (1269), `normalTextureInfo` (1278), `textureInfo` (1287), `textureMapping` (1299), `materialExtensions` (1306), `khrMaterialsAnisotropyObject` (1361), `khrMaterialsClearcoatObject` (1379), `khrMaterialsDiffuseTransmissionObject` (1403), `khrMaterialsDispersionObject` (1428), `khrMaterialsEmissiveStrengthObject` (1443), `khrMaterialsIorStrengthObject` (1458), `khrMaterialsIridescenceObject` (1470), `khrMaterialsRetroreflectionObject` (1489), `khrMaterialsSheenObject` (1506), `khrMaterialsSpecularObject` (1531), `khrMaterialsTransmission` (1563), `khrMaterialsVolumeObject` (1580), `khrMaterialsScatterObject` (1600), `khrMaterialsUnlitObject` (1626), `get` (1694), `get` (1723), `meshesArray` (1769), `meshObject` (1776), `primitivesArray` (1819), `primitiveObject` (1831), `khrGaussianSplatting` (1853), `attributesObject` (1900), `targetsArray` (1925), `primitiveExtensionsObject` (1930), `khrDracoMeshCompressionObject` (1945), `indicesCallback` (1953), `attributeCallback` (1958), `dracoDecodeMesh` (1970), `khrMaterialsVariantsExtension` (2060), `khrMaterialsVariantsObjectMapping` (2101), `camerasArray` (2128), `cameraObject` (2135), `cameraType` (2165), `orthographicCamera` (2177), `get` (2206), `perspectiveCamera` (2241), `nodesArray` (2269), `nodeObject` (2282), `nodeSkeleton` (2303), `nodeExtensions` (2448), `nodeChildrenArray` (2480), `skinsArray` (2494), `skinObject` (2507), `jointsArray` (2554), `skeleton` (2566), `skeletonChildren` (2574), `inverseBindMatricesAccessors` (2593), `skinsHumanoid` (2608), `skinBBox` (2663), `scenesArray` (2721), `sceneObject` (2771), `sceneNodesArray` (2817), `exportGroup` (2821), `materialVariantsSwitch` (2845), `animationsArray` (2869), `animationObject` (2877), `animationChannelsArray` (2908), `animationChannelObject` (2945), `cleanupAnimationScripts` (2983), `createShape` (3007), `meshInstancing` (3023), `getDefaultAppearance` (3077), `hasTextures` (3096), `createMultiTextureTransform` (3108), `createGeometry` (3153), `createPointSet` (3200), `createIndexedLineSet` (3218), `createLineSet` (3306), `createIndexedTriangleSet` (3324), `createTriangleSet` (3346), `createIndexedTriangleStripSet` (3367), `createTriangleStripSet` (3389), `createIndexedTriangleFanSet` (3418), `createTriangleFanSet` (3440), `createMultiTextureCoordinate` (3504), `createTextureCoordinate` (3548), `createNormal` (3567), `createTangent` (3601), `createCoordinate` (3626), `attributesJointsArray` (3660), `attributesJointsObject` (3674), `skinGeometry` (3709), `createInterpolator` (3795), `getAnimationPointer` (4059), `addAnimationPointerAlias` (4075), `getAnimationPointerAlias` (4081), `createNamedInterpolator` (4087), `createOrientationInterpolator` (4174), `createArrayInterpolator` (4263), `cubicSplineVector` (4393), `cubicSplineScalarArray` (4414), `vectorValue` (4437), `numberValue` (4449), `stringValue` (4456), `description` (4463)

## `src/x_ite/Parser/GoldenGate.js` (1; `sub:parse.golden-gate` PARTIAL)

`createInput` (99)

## `src/x_ite/Parser/HTMLSupport.js` (1; `sub:parse.html-support` COMPLETE)

`addConcreteNode` (9)

## `src/x_ite/Parser/JSONParser.js` (11; `sub:parse.json` COMPLETE)

`getEncoding` (14), `setInput` (18), `elementSetAttribute` (62), `convertChildren` (90), `createElement` (107), `createCDATA` (137), `convertObject` (145), `commentStringToXML` (199), `SFStringToXML` (207), `JSONStringToXML` (215), `convertToDOM` (226)

## `src/x_ite/Parser/OBJParser.js` (25; `sub:parse.obj` COMPLETE)

`getEncoding` (89), `setInput` (93), `parseIntoScene` (104), `obj` (110), `statements` (188), `createDefaultMaterial` (249), `vt` (444), `vn` (469), `v` (496), `indices` (630), `index` (657), `vec2` (703), `MaterialParser` (744), `parse` (762), `statements` (796), `newmtl` (846), `Ka` (874), `Kd` (896), `Ks` (916), `Ns` (936), `d` (956), `Tr` (976), `illum` (996), `map_X` (1015), `col3` (1102)

## `src/x_ite/Parser/PLYAParser.js` (10; `sub:parse.ply` COMPLETE)

`getEncoding` (66), `setInput` (70), `parseIntoScene` (78), `ply` (84), `headings` (152), `head` (157), `properties` (201), `processElement` (431), `getSphericalHarmonicsDegree` (580), `getDimensionsForDegree` (593)

## `src/x_ite/Parser/PLYBParser.js` (12; no ledger entry)

`getEncoding` (39), `isValid` (52), `binaryInt8` (59), `binaryUint8` (66), `binaryInt16` (73), `binaryUint16` (80), `binaryInt32` (87), `binaryUint32` (94), `binaryFloat32` (101), `binaryFloat64` (108), `processElements` (115), `parseUnknown` (121)

## `src/x_ite/Parser/Placeholder.js` (4; no ledger entry)

`constructor` (14), `getComponentInfo` (27), `getContainerField` (32), `getSpecificationRange` (37)

## `src/x_ite/Parser/STLAParser.js` (9; `sub:parse.stl` COMPLETE)

`getEncoding` (53), `setInput` (57), `parseIntoScene` (68), `stl` (74), `statements` (123), `facets` (181), `normal` (211), `loop` (244), `vertex` (276)

## `src/x_ite/Parser/STLBParser.js` (6; no ledger entry)

`getEncoding` (19), `setInput` (23), `parseIntoScene` (42), `stl` (48), `header` (76), `shape` (93)

## `src/x_ite/Parser/SVGParser.js` (22; `sub:parse.svg` PARTIAL)

`getEncoding` (157), `setInput` (161), `xmlElement` (193), `textElement` (732), `linearGradientElementUrl` (955), `radialGradientElementUrl` (1003), `patternUrl` (1157), `displayStyle` (2184), `fillOpacityStyle` (2230), `fillRuleStyle` (2250), `strokeOpacityStyle` (2296), `strokeWidthStyle` (2316), `opacityStyle` (2336), `stopColorStyle` (2356), `stopOpacityStyle` (2370), `vectorEffectStyle` (2390), `cssColor` (2470), `createTextureCoordinate` (2651), `createTesselator` (2666), `vertexCallback` (2670), `combineCallback` (2688), `get` (2725)

## `src/x_ite/Parser/VRMLParser.js` (57; `sub:parse.vrml-classic` COMPLETE)

`getEncoding` (151), `getLine` (232), `getLastLine` (246), `componentSupportLevel` (426), `unitConversionFactor` (476), `metaKey` (511), `metaValue` (515), `URLList` (1174), `profileNameId` (1544), `componentNameId` (1545), `categoryNameId` (1551), `unitNameId` (1552), `exportedNodeNameId` (1553), `nodeNameId` (1554), `nodeTypeId` (1555), `initializeOnlyId` (1556), `inputOnlyId` (1557), `outputOnlyId` (1558), `inputOutputId` (1559), `bool` (1576), `sfboolValue` (1664), `sfboolValues` (1696), `mfcolorValue` (1751), `sfcolorValues` (1774), `mfcolorrgbaValue` (1850), `sfcolorrgbaValues` (1873), `sfdoubleValue` (1898), `mfdoubleValue` (1908), `sfdoubleValues` (1930), `sfimageValues` (2005), `sfint32Value` (2018), `mfint32Value` (2028), `sfint32Values` (2050), `sfmatrix3Value` (2061), `mfmatrix3Value` (2122), `sfmatrixValues` (2144), `sfmatrix4Value` (2155), `mfmatrix4Value` (2258), `sfnodeValue` (2280), `nodeStatements` (2316), `sfquaternionValue` (2329), `mfquaternionValue` (2360), `sfquaternionValues` (2382), `sfrotationValue` (2395), `mfrotationValue` (2426), `sfrotationValues` (2448), `sfstringValue` (2461), `sfstringValues` (2493), `sfvec2Value` (2504), `mfvec2Value` (2523), `sfvecValues` (2545), `sfvec3Value` (2558), `mfvec3Value` (2583), `sfvec4Value` (2605), `mfvec4Value` (2636), `sfunknownValues` (2718), `accessTypeToString` (2723)

## `src/x_ite/Parser/X3DOptimizer.js` (2; `sub:parse.optimizer` COMPLETE)

`viewpointsCenterOfRotationNodes` (374), `viewpointsCenterOfRotationNode` (379)

## `src/x_ite/Parser/X3DParser.js` (11; no ledger entry)

`getBrowser` (21), `getScene` (25), `getExecutionContext` (29), `pushExecutionContext` (33), `popExecutionContext` (37), `loadComponents` (52), `setUnits` (56), `getPlaceholders` (151), `getNamedNodes` (155), `getContainer` (163), `checkNodeType` (182)

## `src/x_ite/Parser/XMLParser.js` (45; `sub:parse.xml` COMPLETE)

`getEncoding` (45), `setInput` (49), `isValid` (67), `isXML` (82), `x3dElementChildHead` (172), `x3dElementChildScene` (182), `headElement` (192), `headElementChild` (197), `componentElement` (215), `unitElement` (238), `metaElement` (256), `sceneElement` (270), `childrenElements` (276), `externProtoDeclareElement` (339), `protoDeclareElement` (383), `protoInterfaceElement` (456), `protoInterfaceElementChild` (461), `fieldElement` (471), `protoBodyElement` (522), `isElement` (526), `isElementChild` (534), `connectElement` (544), `fieldValueElement` (625), `importElement` (766), `exportElement` (802), `cdataNode` (832), `useAttribute` (842), `checkNodeType` (888), `defAttribute` (912), `nodeAttributes` (933), `id` (966), `getParents` (976), `getParent` (980), `popParent` (988), `addProtoName` (1050), `addProtoFieldNames` (1052), `protoNameToCamelCase` (1054), `nodeNameToCamelCase` (1058), `attributeToCamelCase` (1062), `[X3DConstants.SFNode]` (1081), `[X3DConstants.SFString]` (1088), `[X3DConstants.MFNode]` (1114), `[X3DConstants.MFString]` (1121), `protoNameToCamelCase` (1189), `nodeNameToCamelCase` (1194)

## `src/x_ite/Prototype/ExternProtoDeclarationArray.js` (1; no ledger entry)

`ExternProtoDeclarationArray` (4)

## `src/x_ite/Prototype/ProtoDeclarationArray.js` (1; no ledger entry)

`ProtoDeclarationArray` (4)

## `src/x_ite/Prototype/X3DExternProtoDeclaration.js` (11; `sub:proto.externproto` PARTIAL)

`initialize` (31), `set_live__` (38), `getAppInfo` (42), `setAppInfo` (47), `getDocumentation` (54), `setDocumentation` (59), `getProtoDeclaration` (66), `getInternalScene` (91), `toVRMLStreamUserDefinedField` (185), `dispose` (303), `urls` (322)

## `src/x_ite/Prototype/X3DProtoDeclaration.js` (5; `sub:proto.proto` PARTIAL)

`initialize` (24), `getProtoDeclaration` (30), `getCloneCount` (38), `collectCloneCount` (42), `toVRMLStreamUserDefinedField` (110)

## `src/x_ite/Prototype/X3DProtoDeclarationNode.js` (6; no ledger entry)

`canUserDefinedFields` (27), `getAppInfo` (31), `setAppInfo` (35), `getDocumentation` (39), `setDocumentation` (43), `newInstance` (62)

## `src/x_ite/Rendering/DependentRenderer.js` (12; no ledger entry)

`initialize` (20), `isIndependent` (25), `getLayer` (29), `getBackground` (33), `setBackground` (37), `getFog` (41), `setFog` (45), `getNavigationInfo` (49), `getViewpoint` (53), `getViewpointStack` (57), `getFramebuffers` (65), `setFramebuffer` (69)

## `src/x_ite/Rendering/MultiSampleFramebuffer.js` (8; no ledger entry)

`getX` (210), `getY` (214), `getWidth` (218), `getHeight` (222), `getSamples` (226), `bind` (234), `clear` (240), `dispose` (351)

## `src/x_ite/Rendering/PointingBuffer.js` (2; no ledger entry)

`bind` (66), `dispose` (104)

## `src/x_ite/Rendering/TextureBuffer.js` (10; no ledger entry)

`TextureBuffer` (5), `get` (15), `getOIT` (99), `getWidth` (103), `getHeight` (107), `getColorTexture` (111), `getDepthTexture` (115), `readPixels` (119), `bind` (178), `dispose` (184)

## `src/x_ite/Rendering/VertexArray.js` (4; no ledger entry)

`VertexArray` (1), `update` (9), `enable` (16), `dispose` (45)

## `src/x_ite/Rendering/X3DRenderObject.js` (50; no ledger entry)

`initialize` (77), `isIndependent` (92), `getOrderIndependentTransparency` (149), `getRenderKey` (153), `getRenderCount` (157), `getFramebuffers` (173), `getView` (177), `getViewVolumes` (181), `getProjectionMatrix` (185), `getProjectionMatrixWithLimits` (189), `getViewMatrix` (197), `getCameraSpaceMatrix` (201), `getProjectionMatrixArray` (205), `getEyeMatrixArray` (209), `getViewMatrixArray` (213), `getCameraSpaceMatrixArray` (217), `getPath` (225), `getSensors` (229), `getViewpointGroups` (233), `getLights` (237), `getGlobalLights` (241), `getGlobalLightsKeys` (245), `getLocalObjects` (249), `getLocalObjectsKeys` (253), `pushGlobalShadows` (257), `getGlobalShadows` (261), `getGlobalShadow` (265), `pushLocalShadows` (269), `popLocalShadows` (273), `getLocalFogs` (298), `getLayouts` (302), `getLayoutRectangles` (306), `getHAnimNode` (310), `getInvHumanoidMatrix` (314), `getRenderedTextures` (318), `getCollisions` (322), `getCollisionTime` (326), `getNumOpaqueShapes` (330), `getOpaqueShapes` (334), `getNumTransparentShapes` (338), `getTransparentShapes` (342), `getRenderPass` (346), `setDepthClearColor` (350), `render` (534), `renderContext` (614), `renderContext` (665), `renderContext` (715), `createRenderContext` (797), `renderContext` (806), `assign` (1314)

## `src/x_ite/Routing/RouteArray.js` (1; no ledger entry)

`RouteArray` (4)

## `src/x_ite/Routing/X3DRoute.js` (6; no ledger entry)

`getExecutionContext` (36), `getSourceNode` (40), `getDestinationNode` (60), `disconnect` (132), `sourceNode` (247), `destinationNode` (260)

## `src/x_ite/Routing/X3DRoutingContext.js` (6; no ledger entry)

`X3DRoutingContext` (8), `initialize` (19), `addTaintedField` (21), `addTaintedNode` (25), `[computed]` (29), `dispose` (76)

## `src/x_ite/X3D.js` (5; no ledger entry)

`ready` (53), `require` (120), `getBrowser` (143), `createBrowser` (154), `toString` (163)

## `src/x_ite/X3DCanvasElement.js` (9; no ledger entry)

`constructor` (7), `connectedCallback` (31), `disconnectedCallback` (36), `observedAttributes` (41), `attributeChangedCallback` (74), `captureStream` (79), `toBlob` (84), `toDataURL` (89), `[Symbol.toStringTag]` (94)
