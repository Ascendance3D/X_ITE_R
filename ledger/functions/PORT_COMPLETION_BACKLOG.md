# Port completion backlog

Upstream: X_ITE 16.5.0 @ 34cdf9f97 (`oracle/x_ite-pin/src`). Generator: `ledger/functions/tools/` (extractor and builder; the rules are in the
builder's module documentation). This file is generated: do not edit it.

`mapping` is code presence only: `cited` (a Rust source line names the X_ITE function), `name` (a Rust function of the same name
in a file or `impl` of the same class; a candidate), `class` (a constructor whose class has Rust files in the 346-entry ledger), `none` (no symbol found by these rules; not proof of absence). `tests` is
separate: `direct` (a Rust test names the function), `entry` (its entry of the 346-entry ledger is COMPLETE with tests), `none`.

One row for each X_ITE source file with applicable functions that have no Rust symbol, or whose ledger entry has an open gap. The order is the owner's order of work (directive of 2026-11-11), then the number of unmapped functions.

A row is closed when each of its functions has a `cited` mapping and a test, or a recorded reason why Rust does not need it. The backlog does not change the 346-entry ledger.

## Priority 2 rendering and graphics

305 files, 1972 functions without a Rust symbol.

| X_ITE file | functions | no Rust symbol | name candidate only | no test | ledger entries |
| --- | --- | --- | --- | --- | --- |
| `src/x_ite/Rendering/X3DRenderObject.js` | 60 | 50 | 0 | 57 | none |
| `src/x_ite/Components/Rendering/X3DGeometryNode.js` | 66 | 47 | 0 | 64 | none |
| `src/x_ite/Browser/Texturing3D/DICOMParser.js` | 39 | 38 | 0 | 39 | none |
| `src/x_ite/Components/Shaders/X3DProgrammableShaderObject.js` | 38 | 35 | 0 | 38 | none |
| `src/standard/Math/Numbers/Vector4.js` | 34 | 34 | 0 | 34 | none |
| `src/x_ite/Browser/Rendering/X3DRenderingContext.js` | 35 | 31 | 0 | 35 | none |
| `src/standard/Math/Numbers/Vector2.js` | 30 | 25 | 5 | 30 | none |
| `src/standard/Math/Numbers/Vector3.js` | 33 | 24 | 7 | 33 | none |
| `src/x_ite/Components/Layering/X3DLayerNode.js` | 35 | 23 | 0 | 33 | none |
| `src/x_ite/Browser/Texturing/X3DTexturingContext.js` | 25 | 22 | 0 | 25 | none |
| `src/x_ite/Components/RigidBodyPhysics/X3DNBodyCollidableNode.js` | 24 | 22 | 0 | 24 | none |
| `src/x_ite/Components/Shape/X3DShapeNode.js` | 34 | 22 | 0 | 33 | none |
| `src/x_ite/Components/ParticleSystems/X3DParticleEmitterNode.js` | 24 | 21 | 0 | 24 | none |
| `src/standard/Math/Numbers/Matrix2.js` | 20 | 20 | 0 | 20 | none |
| `src/x_ite/Browser/GaussianSplats/GaussianSplatsShape.js` | 24 | 20 | 0 | 22 | none |
| `src/x_ite/Components/Geospatial/X3DGeospatialObject.js` | 20 | 20 | 0 | 20 | none |
| `src/x_ite/Components/HAnim/HAnimHumanoid.js` | 29 | 19 | 2 | 0 | node:HAnimHumanoid COMPLETE |
| `src/x_ite/Components/Layout/Layout.js` | 21 | 19 | 1 | 0 | node:Layout COMPLETE |
| `src/x_ite/Components/Texturing/X3DSingleTextureNode.js` | 23 | 19 | 0 | 21 | none |
| `src/x_ite/Browser/PointingDeviceSensor/X3DPointingDeviceSensorContext.js` | 27 | 18 | 0 | 27 | none |
| `src/x_ite/Components/RigidBodyPhysics/X3DRigidJointNode.js` | 20 | 18 | 0 | 20 | none |
| `src/x_ite/Components/Grouping/X3DGroupingNode.js` | 26 | 17 | 0 | 26 | none |
| `src/x_ite/Components/ParticleSystems/ParticleSystem.js` | 32 | 17 | 3 | 27 | node:ParticleSystem PARTIAL |
| `src/x_ite/Browser/Rendering/MikkTSpace.js` | 16 | 16 | 0 | 15 | none |
| `src/standard/Math/Numbers/Complex.js` | 17 | 15 | 0 | 17 | none |
| `src/x_ite/Components/NURBS/X3DNurbsSurfaceGeometryNode.js` | 19 | 15 | 0 | 19 | none |
| `src/x_ite/Components/TextureProjection/TextureProjectorParallel.js` | 16 | 15 | 0 | 0 | node:TextureProjectorParallel COMPLETE |
| `src/standard/Math/Numbers/Rotation4.js` | 37 | 14 | 13 | 37 | none |
| `src/x_ite/Components/RigidBodyPhysics/RigidBodyCollection.js` | 16 | 14 | 0 | 0 | node:RigidBodyCollection COMPLETE |
| `src/x_ite/Components/Shape/PhysicalMaterial.js` | 18 | 14 | 2 | 0 | node:PhysicalMaterial COMPLETE |
| `src/x_ite/Components/Shape/TwoSidedMaterial.js` | 16 | 14 | 0 | 0 | node:TwoSidedMaterial COMPLETE |
| `src/standard/Math/Numbers/Matrix3.js` | 27 | 13 | 7 | 27 | none |
| `src/standard/Math/Numbers/Quaternion.js` | 26 | 13 | 11 | 26 | none |
| `src/x_ite/Components/CADGeometry/CADFace.js` | 16 | 13 | 0 | 0 | node:CADFace COMPLETE |
| `src/x_ite/Components/Followers/X3DChaserNode.js` | 13 | 13 | 0 | 13 | none |
| `src/x_ite/Components/Grouping/X3DBoundedObject.js` | 15 | 13 | 0 | 15 | none |
| `src/x_ite/Components/Lighting/X3DLightNode.js` | 19 | 13 | 0 | 19 | none |
| `src/x_ite/Components/RigidBodyPhysics/RigidBody.js` | 16 | 13 | 1 | 0 | node:RigidBody COMPLETE |
| `src/x_ite/Components/Shape/Appearance.js` | 36 | 13 | 3 | 0 | node:Appearance COMPLETE |
| `src/x_ite/Components/Shape/Material.js` | 17 | 13 | 0 | 0 | node:Material COMPLETE |
| `src/x_ite/Components/VolumeRendering/X3DVolumeDataNode.js` | 16 | 13 | 0 | 16 | none |
| `src/x_ite/Components/X_ITE/SpecularGlossinessMaterial.js` | 14 | 13 | 0 | 0 | node:SpecularGlossinessMaterial COMPLETE |
| `src/x_ite/Browser/Texturing/PNGMedia.js` | 12 | 12 | 0 | 12 | none |
| `src/x_ite/Components/EnvironmentalEffects/X3DBackgroundNode.js` | 16 | 12 | 0 | 16 | none |
| `src/x_ite/Components/HAnim/HAnimMotion.js` | 13 | 12 | 0 | 0 | node:HAnimMotion COMPLETE |
| `src/x_ite/Components/Lighting/EnvironmentLight.js` | 16 | 12 | 0 | 0 | node:EnvironmentLight COMPLETE |
| `src/x_ite/Components/Lighting/SpotLight.js` | 15 | 12 | 1 | 0 | node:SpotLight COMPLETE |
| `src/x_ite/Components/TextureProjection/X3DTextureProjectorNode.js` | 17 | 12 | 0 | 17 | none |
| `src/x_ite/Rendering/DependentRenderer.js` | 14 | 12 | 0 | 14 | none |
| `src/lib/helper.js` | 11 | 11 | 0 | 11 | none |
| `src/standard/Math/Numbers/Color4.js` | 21 | 11 | 9 | 21 | none |
| `src/x_ite/Browser/Texturing3D/NRRDParser.js` | 13 | 11 | 0 | 13 | none |
| `src/x_ite/Components/Geospatial/GeoLOD.js` | 15 | 11 | 0 | 0 | node:GeoLOD COMPLETE |
| `src/x_ite/Components/X_ITE/IridescenceMaterialExtension.js` | 12 | 11 | 0 | 0 | node:IridescenceMaterialExtension COMPLETE |
| `src/standard/Math/Algorithm.js` | 18 | 10 | 0 | 17 | none |
| `src/x_ite/Browser/Shaders/X3DShadersContext.js` | 12 | 10 | 0 | 12 | none |
| `src/x_ite/Browser/Texturing/GifMedia.js` | 10 | 10 | 0 | 10 | none |
| `src/x_ite/Components/EnvironmentalEffects/X3DFogObject.js` | 14 | 10 | 0 | 14 | none |
| `src/x_ite/Components/Followers/X3DFollowerNode.js` | 15 | 10 | 0 | 15 | none |
| `src/x_ite/Components/HAnim/HAnimPose.js` | 11 | 10 | 0 | 0 | node:HAnimPose COMPLETE |
| `src/x_ite/Components/Lighting/PointLight.js` | 13 | 10 | 1 | 0 | node:PointLight COMPLETE |
| `src/x_ite/Components/NURBS/NurbsTrimmedSurface.js` | 11 | 10 | 0 | 11 | node:NurbsTrimmedSurface DIFFERENT |
| `src/x_ite/Components/Shape/X3DMaterialNode.js` | 14 | 10 | 0 | 12 | none |
| `src/x_ite/Components/Texturing3D/X3DTexture3DNode.js` | 13 | 10 | 0 | 13 | none |
| `src/x_ite/Components/VolumeRendering/BlendedVolumeStyle.js` | 12 | 10 | 1 | 12 | node:BlendedVolumeStyle PARTIAL |
| `src/x_ite/Components/X_ITE/ClearcoatMaterialExtension.js` | 11 | 10 | 0 | 0 | node:ClearcoatMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/SpecularMaterialExtension.js` | 11 | 10 | 0 | 0 | node:SpecularMaterialExtension COMPLETE |
| `src/x_ite/Rendering/TextureBuffer.js` | 10 | 10 | 0 | 10 | none |
| `src/standard/Math/Numbers/Matrix4.js` | 26 | 9 | 10 | 24 | none |
| `src/x_ite/Browser/Picking/X3DPickingContext.js` | 11 | 9 | 0 | 11 | none |
| `src/x_ite/Components/Grouping/StaticGroup.js` | 11 | 9 | 0 | 0 | node:StaticGroup COMPLETE |
| `src/x_ite/Components/TextureProjection/TextureProjector.js` | 10 | 9 | 0 | 0 | node:TextureProjector COMPLETE |
| `src/x_ite/Components/Texturing/MultiTextureCoordinate.js` | 10 | 9 | 0 | 0 | node:MultiTextureCoordinate COMPLETE |
| `src/x_ite/Components/X_ITE/DiffuseTransmissionMaterialExtension.js` | 10 | 9 | 0 | 0 | node:DiffuseTransmissionMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/SheenMaterialExtension.js` | 10 | 9 | 0 | 0 | node:SheenMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/VolumeMaterialExtension.js` | 10 | 9 | 0 | 0 | node:VolumeMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/VolumeScatterMaterialExtension.js` | 11 | 9 | 0 | 0 | node:VolumeScatterMaterialExtension COMPLETE |
| `src/standard/Math/Numbers/Color3.js` | 19 | 8 | 5 | 19 | none |
| `src/x_ite/Browser/Shape/X3DShapeContext.js` | 12 | 8 | 0 | 12 | none |
| `src/x_ite/Components/HAnim/HAnimSegment.js` | 11 | 8 | 1 | 0 | node:HAnimSegment COMPLETE |
| `src/x_ite/Components/Rendering/X3DComposedGeometryNode.js` | 11 | 8 | 0 | 11 | none |
| `src/x_ite/Components/Shape/FillProperties.js` | 10 | 8 | 0 | 0 | node:FillProperties COMPLETE |
| `src/x_ite/Components/X_ITE/AnisotropyMaterialExtension.js` | 9 | 8 | 0 | 0 | node:AnisotropyMaterialExtension COMPLETE |
| `src/x_ite/Rendering/MultiSampleFramebuffer.js` | 13 | 8 | 0 | 13 | none |
| `src/x_ite/Components/Grouping/X3DTransformMatrix3DNode.js` | 7 | 7 | 0 | 7 | none |
| `src/x_ite/Components/NURBS/NurbsCurve.js` | 11 | 7 | 1 | 11 | node:NurbsCurve DIFFERENT |
| `src/x_ite/Components/Rendering/X3DLineGeometryNode.js` | 10 | 7 | 0 | 10 | none |
| `src/x_ite/Components/RigidBodyPhysics/CollidableShape.js` | 8 | 7 | 0 | 0 | node:CollidableShape COMPLETE |
| `src/x_ite/Components/RigidBodyPhysics/CollisionCollection.js` | 8 | 7 | 0 | 0 | node:CollisionCollection COMPLETE |
| `src/x_ite/Components/Shape/LineProperties.js` | 11 | 7 | 0 | 0 | node:LineProperties COMPLETE |
| `src/x_ite/Components/Shape/PointProperties.js` | 9 | 7 | 0 | 0 | node:PointProperties COMPLETE |
| `src/x_ite/Components/Shape/X3DAppearanceNode.js` | 7 | 7 | 0 | 7 | none |
| `src/x_ite/Components/Shape/X3DOneSidedMaterialNode.js` | 11 | 7 | 0 | 11 | none |
| `src/x_ite/Components/Texturing/MovieTexture.js` | 19 | 7 | 0 | 19 | node:MovieTexture PARTIAL |
| `src/x_ite/Components/Texturing/X3DSingleTextureTransformNode.js` | 7 | 7 | 0 | 7 | none |
| `src/x_ite/Components/Texturing3D/TextureCoordinate4D.js` | 8 | 7 | 0 | 0 | node:TextureCoordinate4D COMPLETE |
| `src/x_ite/Components/VolumeRendering/ComposedVolumeStyle.js` | 9 | 7 | 1 | 0 | node:ComposedVolumeStyle COMPLETE |
| `src/x_ite/Components/VolumeRendering/X3DVolumeRenderStyleNode.js` | 9 | 7 | 0 | 9 | none |
| `src/x_ite/Components/X_ITE/BlendMode.js` | 10 | 7 | 0 | 0 | node:BlendMode COMPLETE |
| `src/x_ite/Browser/Lighting/X3DLightingContext.js` | 9 | 6 | 0 | 9 | none |
| `src/x_ite/Browser/PointingDeviceSensor/PointingDevice.js` | 13 | 6 | 0 | 10 | none |
| `src/x_ite/Components/CubeMapTexturing/X3DEnvironmentTextureNode.js` | 10 | 6 | 0 | 10 | none |
| `src/x_ite/Components/NURBS/NurbsSet.js` | 9 | 6 | 0 | 9 | node:NurbsSet DIFFERENT |
| `src/x_ite/Components/ParticleSystems/SurfaceEmitter.js` | 7 | 6 | 0 | 0 | node:SurfaceEmitter COMPLETE |
| `src/x_ite/Components/Rendering/ClipPlane.js` | 10 | 6 | 0 | 0 | node:ClipPlane COMPLETE |
| `src/x_ite/Components/Rendering/X3DCoordinateNode.js` | 8 | 6 | 0 | 8 | none |
| `src/x_ite/Components/Shaders/ShaderPart.js` | 10 | 6 | 0 | 8 | node:ShaderPart PARTIAL |
| `src/x_ite/Components/Texturing/MultiTextureTransform.js` | 7 | 6 | 0 | 0 | node:MultiTextureTransform COMPLETE |
| `src/x_ite/Components/Texturing3D/TextureCoordinate3D.js` | 7 | 6 | 0 | 0 | node:TextureCoordinate3D COMPLETE |
| `src/x_ite/Components/VolumeRendering/ShadedVolumeStyle.js` | 8 | 6 | 1 | 8 | node:ShadedVolumeStyle PARTIAL |
| `src/x_ite/Components/X_ITE/EmissiveStrengthMaterialExtension.js` | 7 | 6 | 0 | 0 | node:EmissiveStrengthMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/IORMaterialExtension.js` | 7 | 6 | 0 | 0 | node:IORMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/RetroreflectionMaterialExtension.js` | 8 | 6 | 0 | 0 | node:RetroreflectionMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/TransmissionMaterialExtension.js` | 8 | 6 | 1 | 0 | node:TransmissionMaterialExtension COMPLETE |
| `src/standard/Math/Algorithms/MergeSort.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Browser/VolumeRendering/VolumeMaterial.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Components/CubeMapTexturing/GeneratedCubeMapTexture.js` | 9 | 5 | 1 | 0 | node:GeneratedCubeMapTexture COMPLETE |
| `src/x_ite/Components/CubeMapTexturing/ImageCubeMapTexture.js` | 9 | 5 | 0 | 0 | node:ImageCubeMapTexture COMPLETE |
| `src/x_ite/Components/Followers/ColorChaser.js` | 9 | 5 | 3 | 0 | node:ColorChaser COMPLETE |
| `src/x_ite/Components/Followers/ColorDamper.js` | 9 | 5 | 3 | 0 | node:ColorDamper COMPLETE |
| `src/x_ite/Components/Geometry3D/ElevationGrid.js` | 7 | 5 | 0 | 0 | node:ElevationGrid COMPLETE |
| `src/x_ite/Components/Geospatial/GeoElevationGrid.js` | 8 | 5 | 0 | 0 | node:GeoElevationGrid COMPLETE |
| `src/x_ite/Components/Layout/LayoutGroup.js` | 8 | 5 | 1 | 0 | node:LayoutGroup COMPLETE |
| `src/x_ite/Components/Lighting/DirectionalLight.js` | 8 | 5 | 1 | 0 | node:DirectionalLight COMPLETE |
| `src/x_ite/Components/NURBS/Contour2D.js` | 7 | 5 | 0 | 0 | node:Contour2D COMPLETE |
| `src/x_ite/Components/NURBS/NurbsCurve2D.js` | 7 | 5 | 0 | 7 | node:NurbsCurve2D DIFFERENT |
| `src/x_ite/Components/ParticleSystems/VolumeEmitter.js` | 7 | 5 | 0 | 7 | node:VolumeEmitter PARTIAL |
| `src/x_ite/Components/Rendering/X3DPointGeometryNode.js` | 7 | 5 | 0 | 7 | none |
| `src/x_ite/Components/RigidBodyPhysics/BallJoint.js` | 6 | 5 | 0 | 0 | node:BallJoint COMPLETE |
| `src/x_ite/Components/RigidBodyPhysics/CollisionSpace.js` | 6 | 5 | 0 | 0 | node:CollisionSpace COMPLETE |
| `src/x_ite/Components/Shaders/FloatVertexAttribute.js` | 6 | 5 | 0 | 0 | node:FloatVertexAttribute COMPLETE |
| `src/x_ite/Components/Texturing/ImageTexture.js` | 9 | 5 | 0 | 0 | node:ImageTexture COMPLETE |
| `src/x_ite/Components/Texturing/TextureCoordinate.js` | 7 | 5 | 0 | 0 | node:TextureCoordinate COMPLETE |
| `src/x_ite/Components/Texturing/TextureCoordinateGenerator.js` | 6 | 5 | 0 | 0 | node:TextureCoordinateGenerator COMPLETE |
| `src/x_ite/Components/Texturing/X3DTexture2DNode.js` | 13 | 5 | 0 | 13 | none |
| `src/x_ite/Components/VolumeRendering/CartoonVolumeStyle.js` | 7 | 5 | 1 | 0 | node:CartoonVolumeStyle COMPLETE |
| `src/x_ite/Components/X_ITE/DispersionMaterialExtension.js` | 7 | 5 | 0 | 0 | node:DispersionMaterialExtension COMPLETE |
| `src/x_ite/Components/X_ITE/InstancedShape.js` | 9 | 5 | 1 | 0 | node:InstancedShape COMPLETE |
| `src/standard/Math/Algorithms/QuickSort.js` | 4 | 4 | 0 | 4 | none |
| `src/standard/Math/Algorithms/SAT.js` | 5 | 4 | 0 | 5 | none |
| `src/standard/Math/Geometry/Box3.js` | 17 | 4 | 8 | 17 | none |
| `src/x_ite/Browser/Texturing/KTXDecoder.js` | 4 | 4 | 0 | 4 | none |
| `src/x_ite/Components/CADGeometry/IndexedQuadSet.js` | 6 | 4 | 1 | 0 | node:IndexedQuadSet COMPLETE |
| `src/x_ite/Components/EnvironmentalEffects/FogCoordinate.js` | 6 | 4 | 0 | 0 | node:FogCoordinate COMPLETE |
| `src/x_ite/Components/Followers/ScalarChaser.js` | 9 | 4 | 4 | 0 | node:ScalarChaser COMPLETE |
| `src/x_ite/Components/Geospatial/GeoPositionInterpolator.js` | 5 | 4 | 0 | 0 | node:GeoPositionInterpolator COMPLETE |
| `src/x_ite/Components/HAnim/HAnimJoint.js` | 8 | 4 | 2 | 0 | node:HAnimJoint COMPLETE |
| `src/x_ite/Components/Interpolation/SplinePositionInterpolator.js` | 6 | 4 | 1 | 6 | node:SplinePositionInterpolator DIFFERENT |
| `src/x_ite/Components/Interpolation/SplinePositionInterpolator2D.js` | 6 | 4 | 1 | 6 | node:SplinePositionInterpolator2D DIFFERENT |
| `src/x_ite/Components/Interpolation/SplineScalarInterpolator.js` | 6 | 4 | 1 | 0 | node:SplineScalarInterpolator COMPLETE |
| `src/x_ite/Components/NURBS/NurbsSweptSurface.js` | 6 | 4 | 0 | 6 | node:NurbsSweptSurface DIFFERENT |
| `src/x_ite/Components/NURBS/NurbsSwungSurface.js` | 6 | 4 | 0 | 6 | node:NurbsSwungSurface DIFFERENT |
| `src/x_ite/Components/ParticleSystems/BoundedPhysicsModel.js` | 5 | 4 | 0 | 5 | node:BoundedPhysicsModel PARTIAL |
| `src/x_ite/Components/ParticleSystems/ConeEmitter.js` | 5 | 4 | 0 | 0 | node:ConeEmitter COMPLETE |
| `src/x_ite/Components/Rendering/Color.js` | 5 | 4 | 0 | 0 | node:Color COMPLETE |
| `src/x_ite/Components/Rendering/ColorRGBA.js` | 5 | 4 | 0 | 0 | node:ColorRGBA COMPLETE |
| `src/x_ite/Components/Rendering/IndexedTriangleFanSet.js` | 7 | 4 | 1 | 0 | node:IndexedTriangleFanSet COMPLETE |
| `src/x_ite/Components/Rendering/IndexedTriangleSet.js` | 6 | 4 | 1 | 0 | node:IndexedTriangleSet COMPLETE |
| `src/x_ite/Components/Rendering/IndexedTriangleStripSet.js` | 7 | 4 | 1 | 0 | node:IndexedTriangleStripSet COMPLETE |
| `src/x_ite/Components/Rendering/TriangleFanSet.js` | 7 | 4 | 1 | 0 | node:TriangleFanSet COMPLETE |
| `src/x_ite/Components/Rendering/TriangleStripSet.js` | 7 | 4 | 1 | 0 | node:TriangleStripSet COMPLETE |
| `src/x_ite/Components/RigidBodyPhysics/CollisionSensor.js` | 5 | 4 | 0 | 0 | node:CollisionSensor COMPLETE |
| `src/x_ite/Components/RigidBodyPhysics/SliderJoint.js` | 5 | 4 | 0 | 0 | node:SliderJoint COMPLETE |
| `src/x_ite/Components/Shaders/ComposedShader.js` | 10 | 4 | 0 | 0 | node:ComposedShader COMPLETE |
| `src/x_ite/Components/Shaders/Matrix3VertexAttribute.js` | 5 | 4 | 0 | 0 | node:Matrix3VertexAttribute COMPLETE |
| `src/x_ite/Components/Shaders/Matrix4VertexAttribute.js` | 5 | 4 | 0 | 0 | node:Matrix4VertexAttribute COMPLETE |
| `src/x_ite/Components/Shaders/PackagedShader.js` | 5 | 4 | 0 | 0 | node:PackagedShader COMPLETE |
| `src/x_ite/Components/Shaders/ShaderProgram.js` | 5 | 4 | 0 | 0 | node:ShaderProgram COMPLETE |
| `src/x_ite/Components/Shape/UnlitMaterial.js` | 5 | 4 | 0 | 0 | node:UnlitMaterial COMPLETE |
| `src/x_ite/Components/Texturing/MultiTexture.js` | 10 | 4 | 2 | 0 | node:MultiTexture COMPLETE |
| `src/x_ite/Components/Texturing/PixelTexture.js` | 6 | 4 | 0 | 0 | node:PixelTexture COMPLETE |
| `src/x_ite/Components/Texturing/RenderedTexture.js` | 12 | 4 | 1 | 0 | node:RenderedTexture COMPLETE |
| `src/x_ite/Components/Texturing/TextureProperties.js` | 5 | 4 | 0 | 0 | node:TextureProperties COMPLETE |
| `src/x_ite/Components/Texturing/X3DSingleTextureCoordinateNode.js` | 6 | 4 | 0 | 6 | none |
| `src/x_ite/Components/Texturing3D/ImageTexture3D.js` | 8 | 4 | 0 | 0 | node:ImageTexture3D COMPLETE |
| `src/x_ite/Components/VolumeRendering/EdgeEnhancementVolumeStyle.js` | 6 | 4 | 1 | 0 | node:EdgeEnhancementVolumeStyle COMPLETE |
| `src/x_ite/Components/VolumeRendering/OpacityMapVolumeStyle.js` | 6 | 4 | 1 | 0 | node:OpacityMapVolumeStyle COMPLETE |
| `src/x_ite/Components/VolumeRendering/ProjectionVolumeStyle.js` | 5 | 4 | 0 | 0 | node:ProjectionVolumeStyle COMPLETE |
| `src/x_ite/Components/VolumeRendering/SegmentedVolumeData.js` | 7 | 4 | 1 | 0 | node:SegmentedVolumeData COMPLETE |
| `src/x_ite/Components/VolumeRendering/SilhouetteEnhancementVolumeStyle.js` | 6 | 4 | 1 | 0 | node:SilhouetteEnhancementVolumeStyle COMPLETE |
| `src/x_ite/Components/VolumeRendering/ToneMappedVolumeStyle.js` | 6 | 4 | 1 | 0 | node:ToneMappedVolumeStyle COMPLETE |
| `src/x_ite/Rendering/VertexArray.js` | 4 | 4 | 0 | 4 | none |
| `src/standard/Geospatial/UniversalTransverseMercator.js` | 3 | 3 | 0 | 3 | none |
| `src/standard/Math/Geometry/Spheroid3.js` | 6 | 3 | 2 | 6 | none |
| `src/x_ite/Browser/Picking/VolumePicker.js` | 7 | 3 | 0 | 5 | none |
| `src/x_ite/Browser/Rendering/GeometryContext.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Browser/Rendering/ScreenLine.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Browser/Shaders/ShaderRegistry.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/CADGeometry/QuadSet.js` | 5 | 3 | 1 | 0 | node:QuadSet COMPLETE |
| `src/x_ite/Components/CubeMapTexturing/ComposedCubeMapTexture.js` | 8 | 3 | 1 | 0 | node:ComposedCubeMapTexture COMPLETE |
| `src/x_ite/Components/EnvironmentalEffects/Fog.js` | 5 | 3 | 0 | 0 | node:Fog COMPLETE |
| `src/x_ite/Components/EnvironmentalEffects/LocalFog.js` | 5 | 3 | 0 | 0 | node:LocalFog COMPLETE |
| `src/x_ite/Components/Followers/ScalarDamper.js` | 6 | 3 | 2 | 0 | node:ScalarDamper COMPLETE |
| `src/x_ite/Components/Followers/X3DDamperNode.js` | 8 | 3 | 0 | 8 | none |
| `src/x_ite/Components/Geometry3D/Extrusion.js` | 5 | 3 | 0 | 0 | node:Extrusion COMPLETE |
| `src/x_ite/Components/Geospatial/GeoMetadata.js` | 4 | 3 | 0 | 0 | node:GeoMetadata COMPLETE |
| `src/x_ite/Components/Geospatial/GeoProximitySensor.js` | 4 | 3 | 0 | 0 | node:GeoProximitySensor COMPLETE |
| `src/x_ite/Components/Grouping/Switch.js` | 7 | 3 | 3 | 0 | node:Switch COMPLETE |
| `src/x_ite/Components/Grouping/X3DTransformNode.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/Layering/LayerSet.js` | 10 | 3 | 0 | 0 | node:LayerSet COMPLETE |
| `src/x_ite/Components/ParticleSystems/ExplosionEmitter.js` | 4 | 3 | 0 | 0 | node:ExplosionEmitter COMPLETE |
| `src/x_ite/Components/Rendering/Tangent.js` | 5 | 3 | 0 | 0 | node:Tangent COMPLETE |
| `src/x_ite/Components/Rendering/TriangleSet.js` | 5 | 3 | 1 | 0 | node:TriangleSet COMPLETE |
| `src/x_ite/Components/Rendering/X3DColorNode.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/RigidBodyPhysics/CollidableOffset.js` | 4 | 3 | 0 | 0 | node:CollidableOffset COMPLETE |
| `src/x_ite/Components/RigidBodyPhysics/DoubleAxisHingeJoint.js` | 4 | 3 | 0 | 0 | node:DoubleAxisHingeJoint COMPLETE |
| `src/x_ite/Components/RigidBodyPhysics/SingleAxisHingeJoint.js` | 4 | 3 | 0 | 0 | node:SingleAxisHingeJoint COMPLETE |
| `src/x_ite/Components/RigidBodyPhysics/X3DNBodyCollisionSpaceNode.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/Shaders/X3DVertexAttributeNode.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/Shape/Shape.js` | 5 | 3 | 2 | 0 | node:Shape COMPLETE |
| `src/x_ite/Components/Texturing/X3DTextureNode.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/VolumeRendering/BoundaryEnhancementVolumeStyle.js` | 4 | 3 | 0 | 0 | node:BoundaryEnhancementVolumeStyle COMPLETE |
| `src/x_ite/Components/VolumeRendering/VolumeData.js` | 5 | 3 | 1 | 0 | node:VolumeData COMPLETE |
| `src/standard/Math/Geometry/Triangle3.js` | 5 | 2 | 0 | 5 | none |
| `src/standard/Time/StopWatch.js` | 4 | 2 | 0 | 4 | none |
| `src/x_ite/Browser/CubeMapTexturing/X3DCubeMapTexturingContext.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Browser/EnvironmentalEffects/X3DEnvironmentalEffectsContext.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Browser/Rendering/ScreenPoint.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Browser/VolumeRendering/X3DVolumeRenderingContext.js` | 5 | 2 | 0 | 5 | none |
| `src/x_ite/Components/EnvironmentalEffects/TextureBackground.js` | 4 | 2 | 1 | 0 | node:TextureBackground COMPLETE |
| `src/x_ite/Components/Geometry2D/Arc2D.js` | 3 | 2 | 0 | 0 | node:Arc2D COMPLETE |
| `src/x_ite/Components/Geometry2D/ArcClose2D.js` | 3 | 2 | 0 | 0 | node:ArcClose2D COMPLETE |
| `src/x_ite/Components/Geometry2D/Disk2D.js` | 4 | 2 | 0 | 0 | node:Disk2D COMPLETE |
| `src/x_ite/Components/Geometry3D/Cone.js` | 4 | 2 | 0 | 0 | node:Cone COMPLETE |
| `src/x_ite/Components/Geometry3D/Cylinder.js` | 4 | 2 | 0 | 0 | node:Cylinder COMPLETE |
| `src/x_ite/Components/Geometry3D/IndexedFaceSet.js` | 6 | 2 | 0 | 0 | node:IndexedFaceSet COMPLETE |
| `src/x_ite/Components/Geospatial/GeoCoordinate.js` | 3 | 2 | 0 | 0 | node:GeoCoordinate COMPLETE |
| `src/x_ite/Components/Geospatial/GeoLocation.js` | 3 | 2 | 0 | 0 | node:GeoLocation COMPLETE |
| `src/x_ite/Components/Geospatial/GeoOrigin.js` | 4 | 2 | 0 | 0 | node:GeoOrigin COMPLETE |
| `src/x_ite/Components/Geospatial/GeoTouchSensor.js` | 3 | 2 | 0 | 0 | node:GeoTouchSensor COMPLETE |
| `src/x_ite/Components/Geospatial/GeoTransform.js` | 3 | 2 | 0 | 0 | node:GeoTransform COMPLETE |
| `src/x_ite/Components/Geospatial/GeoViewpoint.js` | 7 | 2 | 1 | 0 | node:GeoViewpoint COMPLETE |
| `src/x_ite/Components/Interpolation/ColorInterpolator.js` | 3 | 2 | 0 | 0 | node:ColorInterpolator COMPLETE |
| `src/x_ite/Components/Interpolation/EaseInEaseOut.js` | 4 | 2 | 0 | 0 | node:EaseInEaseOut COMPLETE |
| `src/x_ite/Components/Interpolation/NormalInterpolator.js` | 3 | 2 | 0 | 0 | node:NormalInterpolator COMPLETE |
| `src/x_ite/Components/Interpolation/OrientationInterpolator.js` | 3 | 2 | 0 | 0 | node:OrientationInterpolator COMPLETE |
| `src/x_ite/Components/Interpolation/PositionInterpolator.js` | 3 | 2 | 0 | 0 | node:PositionInterpolator COMPLETE |
| `src/x_ite/Components/Interpolation/PositionInterpolator2D.js` | 3 | 2 | 0 | 0 | node:PositionInterpolator2D COMPLETE |
| `src/x_ite/Components/Interpolation/ScalarInterpolator.js` | 4 | 2 | 1 | 0 | node:ScalarInterpolator COMPLETE |
| `src/x_ite/Components/Interpolation/SquadOrientationInterpolator.js` | 4 | 2 | 1 | 0 | node:SquadOrientationInterpolator COMPLETE |
| `src/x_ite/Components/Layering/Viewport.js` | 7 | 2 | 3 | 0 | node:Viewport COMPLETE |
| `src/x_ite/Components/Layout/ScreenGroup.js` | 4 | 2 | 0 | 0 | node:ScreenGroup COMPLETE |
| `src/x_ite/Components/NURBS/X3DParametricGeometryNode.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Components/ParticleSystems/PointEmitter.js` | 3 | 2 | 0 | 0 | node:PointEmitter COMPLETE |
| `src/x_ite/Components/ParticleSystems/PolylineEmitter.js` | 4 | 2 | 0 | 0 | node:PolylineEmitter COMPLETE |
| `src/x_ite/Components/ParticleSystems/X3DParticlePhysicsModelNode.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Components/Rendering/IndexedLineSet.js` | 4 | 2 | 0 | 0 | node:IndexedLineSet COMPLETE |
| `src/x_ite/Components/Rendering/Normal.js` | 5 | 2 | 0 | 0 | node:Normal COMPLETE |
| `src/x_ite/Components/Texturing3D/ImageTextureAtlas.js` | 7 | 2 | 0 | 0 | node:ImageTextureAtlas COMPLETE |
| `src/x_ite/Components/VolumeRendering/IsoSurfaceVolumeData.js` | 6 | 2 | 1 | 0 | node:IsoSurfaceVolumeData COMPLETE |
| `src/x_ite/Components/X_ITE/X3DMaterialExtensionNode.js` | 4 | 2 | 0 | 4 | none |
| `src/x_ite/Rendering/PointingBuffer.js` | 4 | 2 | 0 | 2 | none |
| `src/standard/Geospatial/Geodetic.js` | 6 | 1 | 5 | 6 | none |
| `src/standard/Math/Algorithms/Bezier.js` | 2 | 1 | 0 | 2 | none |
| `src/standard/Math/Algorithms/eigen_decomposition.js` | 1 | 1 | 0 | 1 | none |
| `src/standard/Math/Geometry/Box2.js` | 14 | 1 | 10 | 14 | none |
| `src/standard/Math/Geometry/Cylinder3.js` | 8 | 1 | 0 | 8 | none |
| `src/standard/Math/Geometry/Line2.js` | 11 | 1 | 0 | 11 | none |
| `src/standard/Math/Geometry/Line3.js` | 11 | 1 | 0 | 11 | none |
| `src/standard/Math/Geometry/Plane3.js` | 12 | 1 | 0 | 12 | none |
| `src/standard/Math/Geometry/Sphere3.js` | 7 | 1 | 0 | 7 | none |
| `src/standard/Math/Geometry/ViewVolume.js` | 4 | 1 | 0 | 4 | none |
| `src/x_ite/Components/CADGeometry/X3DProductStructureChildNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Followers/CoordinateChaser.js` | 2 | 1 | 0 | 0 | node:CoordinateChaser COMPLETE |
| `src/x_ite/Components/Followers/CoordinateDamper.js` | 2 | 1 | 0 | 0 | node:CoordinateDamper COMPLETE |
| `src/x_ite/Components/Followers/OrientationChaser.js` | 5 | 1 | 3 | 0 | node:OrientationChaser COMPLETE |
| `src/x_ite/Components/Followers/OrientationDamper.js` | 4 | 1 | 2 | 0 | node:OrientationDamper COMPLETE |
| `src/x_ite/Components/Followers/PositionChaser.js` | 2 | 1 | 0 | 0 | node:PositionChaser COMPLETE |
| `src/x_ite/Components/Followers/PositionChaser2D.js` | 2 | 1 | 0 | 0 | node:PositionChaser2D COMPLETE |
| `src/x_ite/Components/Followers/PositionDamper.js` | 2 | 1 | 0 | 0 | node:PositionDamper COMPLETE |
| `src/x_ite/Components/Followers/PositionDamper2D.js` | 2 | 1 | 0 | 0 | node:PositionDamper2D COMPLETE |
| `src/x_ite/Components/Followers/TexCoordChaser2D.js` | 2 | 1 | 0 | 0 | node:TexCoordChaser2D COMPLETE |
| `src/x_ite/Components/Followers/TexCoordDamper2D.js` | 2 | 1 | 0 | 0 | node:TexCoordDamper2D COMPLETE |
| `src/x_ite/Components/Geometry2D/Circle2D.js` | 3 | 1 | 0 | 0 | node:Circle2D COMPLETE |
| `src/x_ite/Components/Geometry3D/Sphere.js` | 3 | 1 | 0 | 0 | node:Sphere COMPLETE |
| `src/x_ite/Components/Interpolation/CoordinateInterpolator.js` | 3 | 1 | 1 | 0 | node:CoordinateInterpolator COMPLETE |
| `src/x_ite/Components/Interpolation/CoordinateInterpolator2D.js` | 3 | 1 | 1 | 0 | node:CoordinateInterpolator2D COMPLETE |
| `src/x_ite/Components/Layering/X3DViewportNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Layout/LayoutLayer.js` | 2 | 1 | 0 | 0 | node:LayoutLayer COMPLETE |
| `src/x_ite/Components/Layout/ScreenFontStyle.js` | 5 | 1 | 0 | 0 | node:ScreenFontStyle COMPLETE |
| `src/x_ite/Components/Layout/X3DLayoutNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/NURBS/ContourPolyline2D.js` | 2 | 1 | 0 | 0 | node:ContourPolyline2D COMPLETE |
| `src/x_ite/Components/NURBS/NurbsOrientationInterpolator.js` | 3 | 1 | 1 | 3 | node:NurbsOrientationInterpolator DIFFERENT |
| `src/x_ite/Components/NURBS/NurbsPositionInterpolator.js` | 3 | 1 | 1 | 3 | node:NurbsPositionInterpolator DIFFERENT |
| `src/x_ite/Components/NURBS/NurbsSurfaceInterpolator.js` | 3 | 1 | 1 | 3 | node:NurbsSurfaceInterpolator DIFFERENT |
| `src/x_ite/Components/NURBS/NurbsTextureCoordinate.js` | 3 | 1 | 0 | 3 | node:NurbsTextureCoordinate DIFFERENT |
| `src/x_ite/Components/NURBS/X3DNurbsControlCurveNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/ParticleSystems/WindPhysicsModel.js` | 2 | 1 | 0 | 2 | node:WindPhysicsModel PARTIAL |
| `src/x_ite/Components/Rendering/LineSet.js` | 3 | 1 | 0 | 0 | node:LineSet COMPLETE |
| `src/x_ite/Components/Rendering/PointSet.js` | 3 | 1 | 0 | 0 | node:PointSet COMPLETE |
| `src/x_ite/Components/Rendering/X3DGeometricPropertyNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Rendering/X3DNormalNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Rendering/X3DTangentNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Shaders/X3DShaderNode.js` | 5 | 1 | 0 | 5 | none |
| `src/x_ite/Components/Shape/AcousticProperties.js` | 1 | 1 | 0 | 0 | node:AcousticProperties COMPLETE |
| `src/x_ite/Components/Shape/X3DAppearanceChildNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Texturing/TextureTransform.js` | 2 | 1 | 0 | 0 | node:TextureTransform COMPLETE |
| `src/x_ite/Components/Texturing/X3DTextureCoordinateNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Texturing/X3DTextureTransformNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Texturing3D/ComposedTexture3D.js` | 6 | 1 | 1 | 0 | node:ComposedTexture3D COMPLETE |
| `src/x_ite/Components/Texturing3D/PixelTexture3D.js` | 2 | 1 | 0 | 0 | node:PixelTexture3D COMPLETE |
| `src/x_ite/Components/Texturing3D/TextureTransform3D.js` | 2 | 1 | 0 | 0 | node:TextureTransform3D COMPLETE |
| `src/x_ite/Components/Texturing3D/TextureTransformMatrix3D.js` | 3 | 1 | 0 | 0 | node:TextureTransformMatrix3D COMPLETE |
| `src/x_ite/Components/VolumeRendering/X3DComposableVolumeRenderStyleNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/X_ITE/DepthMode.js` | 4 | 1 | 0 | 0 | node:DepthMode COMPLETE |

## Priority 3 navigation and collision

41 files, 229 functions without a Rust symbol.

| X_ITE file | functions | no Rust symbol | name candidate only | no test | ledger entries |
| --- | --- | --- | --- | --- | --- |
| `src/x_ite/Components/Navigation/X3DViewpointNode.js` | 45 | 25 | 0 | 45 | none |
| `src/x_ite/Browser/Navigation/X3DNavigationContext.js` | 19 | 16 | 0 | 19 | none |
| `src/x_ite/Components/Navigation/OrthoViewpoint.js` | 31 | 14 | 5 | 0 | node:OrthoViewpoint COMPLETE |
| `src/x_ite/Browser/Navigation/LookAtViewer.js` | 13 | 13 | 0 | 13 | none |
| `src/x_ite/Components/Navigation/NavigationInfo.js` | 17 | 13 | 0 | 0 | node:NavigationInfo COMPLETE |
| `src/x_ite/Components/Picking/X3DPickSensorNode.js` | 18 | 13 | 0 | 18 | none |
| `src/x_ite/Browser/Followers/X3DArrayFollowerTemplate.js` | 13 | 11 | 0 | 13 | none |
| `src/x_ite/Browser/Navigation/X3DFlyViewer.js` | 14 | 10 | 0 | 14 | none |
| `src/x_ite/Components/PointingDeviceSensor/X3DPointingDeviceSensorNode.js` | 13 | 9 | 0 | 13 | none |
| `src/x_ite/Browser/Navigation/PlaneViewer.js` | 11 | 8 | 0 | 11 | none |
| `src/x_ite/Browser/Navigation/X3DViewer.js` | 16 | 8 | 1 | 16 | none |
| `src/x_ite/Components/EnvironmentalSensor/X3DEnvironmentalSensorNode.js` | 8 | 8 | 0 | 8 | none |
| `src/x_ite/Components/PointingDeviceSensor/CylinderSensor.js` | 9 | 6 | 2 | 0 | node:CylinderSensor COMPLETE |
| `src/x_ite/Browser/Navigation/WalkViewer.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Components/KeyDeviceSensor/X3DKeyDeviceSensorNode.js` | 9 | 5 | 0 | 9 | none |
| `src/x_ite/Components/Picking/X3DPickableObject.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Components/PointingDeviceSensor/PlaneSensor.js` | 9 | 5 | 3 | 0 | node:PlaneSensor COMPLETE |
| `src/x_ite/Browser/Navigation/ExamineViewer.js` | 20 | 4 | 10 | 20 | sub:nav.examine PARTIAL |
| `src/x_ite/Browser/Navigation/FlyViewer.js` | 4 | 4 | 0 | 2 | none |
| `src/x_ite/Components/EnvironmentalSensor/TransformSensor.js` | 10 | 4 | 0 | 0 | node:TransformSensor COMPLETE |
| `src/x_ite/Components/Navigation/Viewpoint.js` | 10 | 4 | 1 | 0 | node:Viewpoint COMPLETE |
| `src/x_ite/Browser/Followers/X3DArrayChaserTemplate.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Browser/Layering/X3DLayeringContext.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Browser/Layout/ScreenText.js` | 6 | 3 | 0 | 6 | none |
| `src/x_ite/Components/KeyDeviceSensor/KeySensor.js` | 4 | 3 | 0 | 0 | node:KeySensor COMPLETE |
| `src/x_ite/Components/Navigation/Collision.js` | 8 | 3 | 1 | 0 | node:Collision COMPLETE |
| `src/x_ite/Components/Navigation/LOD.js` | 8 | 3 | 2 | 0 | node:LOD COMPLETE |
| `src/x_ite/Components/Navigation/ViewpointGroup.js` | 7 | 3 | 0 | 0 | node:ViewpointGroup COMPLETE |
| `src/x_ite/Components/Picking/PickableGroup.js` | 5 | 3 | 0 | 0 | node:PickableGroup COMPLETE |
| `src/x_ite/Components/Picking/VolumePickSensor.js` | 3 | 2 | 0 | 0 | node:VolumePickSensor COMPLETE |
| `src/x_ite/Components/PointingDeviceSensor/SphereSensor.js` | 5 | 2 | 2 | 0 | node:SphereSensor COMPLETE |
| `src/x_ite/Components/PointingDeviceSensor/X3DTouchSensorNode.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Browser/Navigation/NoneViewer.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/EnvironmentalSensor/ProximitySensor.js` | 4 | 1 | 1 | 0 | node:ProximitySensor COMPLETE |
| `src/x_ite/Components/EnvironmentalSensor/VisibilitySensor.js` | 3 | 1 | 0 | 0 | node:VisibilitySensor COMPLETE |
| `src/x_ite/Components/KeyDeviceSensor/StringSensor.js` | 2 | 1 | 0 | 0 | node:StringSensor COMPLETE |
| `src/x_ite/Components/Navigation/Billboard.js` | 4 | 1 | 0 | 0 | node:Billboard COMPLETE |
| `src/x_ite/Components/Picking/LinePickSensor.js` | 3 | 1 | 1 | 0 | node:LinePickSensor COMPLETE |
| `src/x_ite/Components/Picking/PointPickSensor.js` | 3 | 1 | 1 | 0 | node:PointPickSensor COMPLETE |
| `src/x_ite/Components/Picking/PrimitivePickSensor.js` | 3 | 1 | 1 | 0 | node:PrimitivePickSensor COMPLETE |
| `src/x_ite/Components/PointingDeviceSensor/X3DDragSensorNode.js` | 1 | 1 | 0 | 1 | none |

## Priority 4 browser and WASM host

31 files, 274 functions without a Rust symbol.

| X_ITE file | functions | no Rust symbol | name candidate only | no test | ledger entries |
| --- | --- | --- | --- | --- | --- |
| `src/x_ite/Browser/X3DBrowser.js` | 79 | 54 | 0 | 0 | sub:browser.sai-api COMPLETE |
| `src/x_ite/Browser/Core/ContextMenu.js` | 38 | 34 | 0 | 38 | none |
| `src/x_ite/Browser/Core/X3DCoreContext.js` | 35 | 32 | 1 | 35 | sub:browser.keyboard-ui PARTIAL |
| `src/x_ite/Browser/X3DBrowserContext.js` | 27 | 25 | 0 | 27 | none |
| `src/x_ite/Browser/Core/BrowserOptions.js` | 26 | 19 | 3 | 0 | sub:browser.options COMPLETE |
| `src/x_ite/Browser/Networking/X3DNetworkingContext.js` | 18 | 15 | 0 | 18 | none |
| `src/x_ite/Browser/DOMIntegration.js` | 16 | 13 | 0 | 16 | sub:browser.dom-integration DIFFERENT |
| `src/x_ite/Browser/Core/BrowserTimings.js` | 15 | 9 | 0 | 13 | none |
| `src/x_ite/Features.js` | 9 | 9 | 0 | 9 | none |
| `src/x_ite/X3DCanvasElement.js` | 9 | 9 | 0 | 9 | none |
| `src/x_ite/Browser/WebXR/X3DWebXRContext.js` | 14 | 8 | 0 | 12 | none |
| `src/x_ite/Browser/Legacy.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Browser/Networking/URLs.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Configuration/ProfileInfo.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/X3D.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Browser/Time/X3DTimeContext.js` | 5 | 3 | 0 | 5 | none |
| `src/x_ite/Configuration/ComponentInfo.js` | 5 | 3 | 0 | 5 | none |
| `src/x_ite/Configuration/UnitInfo.js` | 5 | 3 | 0 | 5 | none |
| `src/x_ite/Browser/Core/Context.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Browser/Core/Notification.js` | 3 | 2 | 0 | 3 | none |
| `src/x_ite/Configuration/AbstractNodesArray.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Configuration/ComponentInfoArray.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Configuration/ProfileInfoArray.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Browser/Core/Fonts.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Browser/Core/RenderingProperties.js` | 3 | 1 | 0 | 3 | none |
| `src/x_ite/Browser/WebXR/xrX3DViewer.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Configuration/ConcreteNodesArray.js` | 3 | 1 | 0 | 3 | none |
| `src/x_ite/Configuration/FieldTypesArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Configuration/SupportedProfiles.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Configuration/UnitInfoArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Namespace.js` | 1 | 1 | 0 | 1 | none |

## Priority 6 scene loading and scripting

125 files, 1270 functions without a Rust symbol.

| X_ITE file | functions | no Rust symbol | name candidate only | no test | ledger entries |
| --- | --- | --- | --- | --- | --- |
| `src/x_ite/Parser/GLTF2Parser.js` | 148 | 145 | 0 | 148 | sub:parse.gltf2 PARTIAL |
| `src/x_ite/Parser/VRMLParser.js` | 113 | 57 | 48 | 0 | sub:parse.vrml-classic COMPLETE |
| `src/x_ite/Parser/XMLParser.js` | 61 | 45 | 0 | 0 | sub:parse.xml COMPLETE |
| `src/x_ite/Base/X3DField.js` | 57 | 40 | 0 | 49 | none |
| `src/x_ite/Fields/SFQuaternion.js` | 37 | 37 | 0 | 37 | none |
| `src/x_ite/Fields/SFVecPrototypeTemplate.js` | 37 | 37 | 0 | 37 | none |
| `src/x_ite/Base/X3DTypedArrayField.js` | 42 | 36 | 0 | 40 | none |
| `src/x_ite/Fields/SFScalar.js` | 35 | 33 | 0 | 35 | none |
| `src/x_ite/Fields/SFRotation.js` | 32 | 32 | 0 | 32 | none |
| `src/x_ite/Base/X3DObjectArrayField.js` | 34 | 29 | 0 | 34 | none |
| `src/x_ite/Base/X3DBaseNode.js` | 53 | 28 | 0 | 51 | none |
| `src/x_ite/Fields/SFMatrixPrototypeTemplate.js` | 28 | 28 | 0 | 28 | none |
| `src/x_ite/Fields/SFImage.js` | 31 | 26 | 1 | 31 | none |
| `src/x_ite/Parser/OBJParser.js` | 54 | 25 | 27 | 0 | sub:parse.obj COMPLETE |
| `src/x_ite/Execution/X3DExecutionContext.js` | 60 | 22 | 1 | 60 | sub:exec.context-api PARTIAL |
| `src/x_ite/Fields/SFColor.js` | 22 | 22 | 0 | 22 | none |
| `src/x_ite/Fields/SFNode.js` | 28 | 22 | 0 | 28 | none |
| `src/x_ite/Parser/SVGParser.js` | 74 | 22 | 47 | 74 | sub:parse.svg PARTIAL |
| `src/x_ite/Fields/ArrayFields.js` | 24 | 20 | 0 | 24 | none |
| `src/x_ite/Execution/X3DScene.js` | 47 | 19 | 0 | 0 | sub:exec.scene COMPLETE |
| `src/x_ite/Base/X3DInfoArray.js` | 28 | 17 | 5 | 28 | none |
| `src/x_ite/Fields/SFColorRGBA.js` | 17 | 17 | 0 | 17 | none |
| `src/x_ite/Base/X3DChildObject.js` | 19 | 16 | 0 | 19 | none |
| `src/x_ite/Base/X3DObject.js` | 22 | 16 | 0 | 22 | none |
| `src/x_ite/Components/Core/X3DChildNode.js` | 19 | 16 | 0 | 19 | none |
| `src/x_ite/Components/Core/X3DPrototypeInstance.js` | 26 | 16 | 0 | 26 | none |
| `src/x_ite/Components/Time/X3DTimeDependentNode.js` | 29 | 15 | 0 | 29 | none |
| `src/x_ite/InputOutput/X3DGenerator.js` | 37 | 15 | 13 | 35 | none |
| `src/standard/Math/Utility/MatrixStack.js` | 14 | 14 | 0 | 14 | none |
| `src/x_ite/Components/Core/X3DNode.js` | 30 | 14 | 0 | 30 | none |
| `src/x_ite/Base/IterableWeakSet.js` | 12 | 12 | 0 | 12 | none |
| `src/x_ite/Parser/PLYBParser.js` | 14 | 12 | 0 | 14 | none |
| `src/standard/Math/Utility/BVH.js` | 12 | 11 | 0 | 12 | none |
| `src/standard/Utility/BitSet.js` | 11 | 11 | 0 | 11 | none |
| `src/x_ite/Base/X3DArrayField.js` | 11 | 11 | 0 | 11 | none |
| `src/x_ite/Execution/X3DImportedNode.js` | 18 | 11 | 0 | 18 | none |
| `src/x_ite/Parser/JSONParser.js` | 14 | 11 | 0 | 0 | sub:parse.json COMPLETE |
| `src/x_ite/Parser/X3DParser.js` | 25 | 11 | 0 | 23 | none |
| `src/x_ite/Prototype/X3DExternProtoDeclaration.js` | 18 | 11 | 0 | 16 | sub:proto.externproto PARTIAL |
| `src/x_ite/Base/X3DFieldDefinition.js` | 10 | 10 | 0 | 10 | none |
| `src/x_ite/Parser/PLYAParser.js` | 29 | 10 | 16 | 0 | sub:parse.ply COMPLETE |
| `src/standard/Utility/DataStorage.js` | 9 | 9 | 0 | 9 | none |
| `src/x_ite/Components/Networking/LoadSensor.js` | 11 | 9 | 0 | 0 | node:LoadSensor COMPLETE |
| `src/x_ite/Components/Scripting/Script.js` | 21 | 9 | 2 | 20 | node:Script PARTIAL |
| `src/x_ite/Parser/STLAParser.js` | 18 | 9 | 7 | 0 | sub:parse.stl COMPLETE |
| `src/x_ite/Components/Core/MetadataSet.js` | 8 | 7 | 0 | 0 | node:MetadataSet COMPLETE |
| `src/x_ite/Components/Core/X3DImportedNodeInstance.js` | 11 | 7 | 0 | 11 | none |
| `src/x_ite/Components/Networking/X3DUrlObject.js` | 23 | 7 | 0 | 21 | none |
| `src/x_ite/InputOutput/XMLGenerator.js` | 10 | 7 | 0 | 10 | none |
| `src/x_ite/Components/EventUtilities/X3DSequencerNode.js` | 6 | 6 | 0 | 6 | none |
| `src/x_ite/Components/Networking/InlineGeometry.js` | 15 | 6 | 0 | 0 | node:InlineGeometry COMPLETE |
| `src/x_ite/Execution/X3DExportedNode.js` | 11 | 6 | 0 | 11 | none |
| `src/x_ite/Fields/SFMatrix3.js` | 6 | 6 | 0 | 6 | none |
| `src/x_ite/Parser/GLB2Parser.js` | 7 | 6 | 0 | 7 | sub:parse.glb2 PARTIAL |
| `src/x_ite/Parser/STLBParser.js` | 8 | 6 | 0 | 8 | none |
| `src/x_ite/Prototype/X3DProtoDeclarationNode.js` | 10 | 6 | 0 | 10 | none |
| `src/x_ite/Routing/X3DRoute.js` | 16 | 6 | 0 | 16 | none |
| `src/x_ite/Routing/X3DRoutingContext.js` | 6 | 6 | 0 | 6 | none |
| `src/x_ite/Components/Core/X3DBindableNode.js` | 7 | 5 | 0 | 7 | none |
| `src/x_ite/InputOutput/FileLoader.js` | 21 | 5 | 0 | 14 | sub:io.file-loader PARTIAL |
| `src/x_ite/Prototype/X3DProtoDeclaration.js` | 10 | 5 | 0 | 10 | sub:proto.proto PARTIAL |
| `src/standard/Utility/ObjectCache.js` | 4 | 4 | 0 | 4 | none |
| `src/x_ite/Components/Networking/Anchor.js` | 9 | 4 | 1 | 0 | node:Anchor COMPLETE |
| `src/x_ite/Components/Networking/Inline.js` | 11 | 4 | 1 | 10 | node:Inline PARTIAL |
| `src/x_ite/Components/Time/TimeSensor.js` | 13 | 4 | 1 | 0 | node:TimeSensor COMPLETE |
| `src/x_ite/Execution/BindableList.js` | 6 | 4 | 0 | 4 | none |
| `src/x_ite/Execution/X3DWorld.js` | 10 | 4 | 0 | 10 | none |
| `src/x_ite/Parser/Placeholder.js` | 5 | 4 | 0 | 5 | none |
| `src/x_ite/Browser/Scripting/X3DScriptingContext.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/Annotation/IconAnnotation.js` | 4 | 3 | 0 | 4 | node:IconAnnotation DIFFERENT |
| `src/x_ite/Components/Core/MetadataBoolean.js` | 4 | 3 | 0 | 0 | node:MetadataBoolean COMPLETE |
| `src/x_ite/Components/Core/MetadataDouble.js` | 4 | 3 | 0 | 0 | node:MetadataDouble COMPLETE |
| `src/x_ite/Components/Core/MetadataFloat.js` | 4 | 3 | 0 | 0 | node:MetadataFloat COMPLETE |
| `src/x_ite/Components/Core/MetadataInteger.js` | 4 | 3 | 0 | 0 | node:MetadataInteger COMPLETE |
| `src/x_ite/Components/Core/MetadataString.js` | 4 | 3 | 0 | 0 | node:MetadataString COMPLETE |
| `src/x_ite/Components/Core/X3DMetadataObject.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Components/DIS/ReceiverPdu.js` | 4 | 3 | 0 | 0 | node:ReceiverPdu COMPLETE |
| `src/x_ite/Components/DIS/SignalPdu.js` | 4 | 3 | 0 | 0 | node:SignalPdu COMPLETE |
| `src/x_ite/Components/DIS/TransmitterPdu.js` | 4 | 3 | 0 | 0 | node:TransmitterPdu COMPLETE |
| `src/x_ite/Components/EventUtilities/BooleanSequencer.js` | 4 | 3 | 0 | 0 | node:BooleanSequencer COMPLETE |
| `src/x_ite/Components/EventUtilities/IntegerSequencer.js` | 4 | 3 | 0 | 0 | node:IntegerSequencer COMPLETE |
| `src/x_ite/Components/Scripting/X3DScriptNode.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Fields/SFVec3.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Fields/SFVec4.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/InputOutput/JSONGenerator.js` | 10 | 3 | 0 | 10 | none |
| `src/x_ite/Parser/Expressions.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Base/Events.js` | 3 | 2 | 0 | 3 | none |
| `src/x_ite/Components/Annotation/GroupAnnotation.js` | 3 | 2 | 0 | 3 | node:GroupAnnotation DIFFERENT |
| `src/x_ite/Components/Annotation/X3DAnnotationNode.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Components/DIS/DISEntityTypeMapping.js` | 3 | 2 | 0 | 0 | node:DISEntityTypeMapping COMPLETE |
| `src/x_ite/Components/EventUtilities/BooleanFilter.js` | 3 | 2 | 0 | 0 | node:BooleanFilter COMPLETE |
| `src/x_ite/Components/EventUtilities/BooleanToggle.js` | 3 | 2 | 0 | 0 | node:BooleanToggle COMPLETE |
| `src/x_ite/Components/EventUtilities/BooleanTrigger.js` | 3 | 2 | 0 | 0 | node:BooleanTrigger COMPLETE |
| `src/x_ite/Components/EventUtilities/IntegerTrigger.js` | 3 | 2 | 0 | 0 | node:IntegerTrigger COMPLETE |
| `src/x_ite/Components/EventUtilities/TimeTrigger.js` | 3 | 2 | 0 | 0 | node:TimeTrigger COMPLETE |
| `src/x_ite/Execution/BindableStack.js` | 5 | 2 | 0 | 5 | none |
| `src/x_ite/Fields/SFMatrix4.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Fields/SFNodeCache.js` | 3 | 2 | 0 | 3 | none |
| `src/x_ite/Fields/SFVec2.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Parser/X3DOptimizer.js` | 14 | 2 | 8 | 0 | sub:parse.optimizer COMPLETE |
| `src/standard/Utility/Lock.js` | 1 | 1 | 0 | 1 | none |
| `src/standard/Utility/Shuffle.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Base/FieldArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Base/FieldDefinitionArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Base/X3DCast.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Base/X3DConstants.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Browser/Scripting/evaluate.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Annotation/AnnotationLayer.js` | 2 | 1 | 0 | 2 | node:AnnotationLayer DIFFERENT |
| `src/x_ite/Components/Annotation/AnnotationTarget.js` | 2 | 1 | 0 | 2 | node:AnnotationTarget DIFFERENT |
| `src/x_ite/Components/Annotation/TextAnnotation.js` | 2 | 1 | 0 | 2 | node:TextAnnotation DIFFERENT |
| `src/x_ite/Components/Annotation/URLAnnotation.js` | 2 | 1 | 0 | 2 | node:URLAnnotation DIFFERENT |
| `src/x_ite/Components/Core/X3DInfoNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Core/X3DSensorNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/DIS/EspduTransform.js` | 2 | 1 | 0 | 0 | node:EspduTransform COMPLETE |
| `src/x_ite/Components/EventUtilities/X3DTriggerNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Networking/X3DNetworkSensorNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Networking/X3DUrlOutputObject.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Execution/ExportedNodesArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Execution/ImportedNodesArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Execution/NamedNodesArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Parser/GoldenGate.js` | 6 | 1 | 0 | 5 | sub:parse.golden-gate PARTIAL |
| `src/x_ite/Parser/HTMLSupport.js` | 5 | 1 | 2 | 0 | sub:parse.html-support COMPLETE |
| `src/x_ite/Prototype/ExternProtoDeclarationArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Prototype/ProtoDeclarationArray.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Routing/RouteArray.js` | 1 | 1 | 0 | 1 | none |

## Priority 7 text, media and audio

33 files, 153 functions without a Rust symbol.

| X_ITE file | functions | no Rust symbol | name candidate only | no test | ledger entries |
| --- | --- | --- | --- | --- | --- |
| `src/x_ite/Components/Sound/SpatialSound.js` | 17 | 15 | 0 | 17 | node:SpatialSound DIFFERENT |
| `src/x_ite/Browser/Text/X3DTextGeometry.js` | 18 | 13 | 0 | 18 | none |
| `src/x_ite/Browser/Sound/AudioElement.js` | 15 | 10 | 4 | 15 | none |
| `src/x_ite/Components/Sound/OscillatorSource.js` | 11 | 9 | 1 | 0 | node:OscillatorSource COMPLETE |
| `src/x_ite/Components/Sound/X3DSoundProcessingNode.js` | 13 | 9 | 0 | 13 | none |
| `src/x_ite/Components/Sound/DynamicsCompressor.js` | 8 | 7 | 0 | 0 | node:DynamicsCompressor COMPLETE |
| `src/x_ite/Components/Sound/X3DSoundChannelNode.js` | 13 | 7 | 0 | 13 | none |
| `src/x_ite/Components/Sound/X3DSoundSourceNode.js` | 14 | 7 | 0 | 14 | none |
| `src/x_ite/Browser/Sound/X3DSoundContext.js` | 12 | 6 | 0 | 12 | none |
| `src/x_ite/Components/Sound/MicrophoneSource.js` | 8 | 6 | 0 | 0 | node:MicrophoneSource COMPLETE |
| `src/x_ite/Components/Text/X3DFontStyleNode.js` | 12 | 6 | 0 | 10 | none |
| `src/x_ite/Components/Sound/Sound.js` | 11 | 5 | 0 | 11 | node:Sound PARTIAL |
| `src/x_ite/Components/Sound/X3DSoundDestinationNode.js` | 8 | 5 | 0 | 8 | none |
| `src/x_ite/Browser/Text/PolygonText.js` | 9 | 4 | 0 | 9 | none |
| `src/x_ite/Components/Sound/Delay.js` | 5 | 4 | 0 | 0 | node:Delay COMPLETE |
| `src/x_ite/Browser/Text/X3DTextContext.js` | 9 | 3 | 0 | 9 | none |
| `src/x_ite/Components/Sound/BiquadFilter.js` | 5 | 3 | 1 | 0 | node:BiquadFilter COMPLETE |
| `src/x_ite/Components/Sound/ChannelMerger.js` | 5 | 3 | 0 | 0 | node:ChannelMerger COMPLETE |
| `src/x_ite/Components/Sound/Convolver.js` | 4 | 3 | 0 | 0 | node:Convolver COMPLETE |
| `src/x_ite/Components/Sound/StreamAudioDestination.js` | 4 | 3 | 0 | 0 | node:StreamAudioDestination COMPLETE |
| `src/x_ite/Components/Text/FontStyle.js` | 4 | 3 | 0 | 0 | node:FontStyle COMPLETE |
| `src/x_ite/Components/Text/Text.js` | 10 | 3 | 0 | 0 | node:Text COMPLETE |
| `src/x_ite/Components/Sound/Analyser.js` | 6 | 2 | 2 | 0 | node:Analyser COMPLETE |
| `src/x_ite/Components/Sound/AudioClip.js` | 8 | 2 | 0 | 5 | node:AudioClip PARTIAL |
| `src/x_ite/Components/Sound/AudioDestination.js` | 5 | 2 | 0 | 5 | node:AudioDestination PARTIAL |
| `src/x_ite/Components/Sound/BufferAudioSource.js` | 12 | 2 | 4 | 12 | node:BufferAudioSource PARTIAL |
| `src/x_ite/Components/Sound/ChannelSelector.js` | 3 | 2 | 0 | 0 | node:ChannelSelector COMPLETE |
| `src/x_ite/Components/Sound/ChannelSplitter.js` | 3 | 2 | 0 | 0 | node:ChannelSplitter COMPLETE |
| `src/x_ite/Components/Sound/ListenerPointSource.js` | 3 | 2 | 0 | 0 | node:ListenerPointSource COMPLETE |
| `src/x_ite/Components/Sound/WaveShaper.js` | 3 | 2 | 0 | 0 | node:WaveShaper COMPLETE |
| `src/x_ite/Components/Sound/StreamAudioSource.js` | 2 | 1 | 0 | 0 | node:StreamAudioSource COMPLETE |
| `src/x_ite/Components/Sound/X3DSoundNode.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Components/Text/FontLibrary.js` | 5 | 1 | 0 | 5 | node:FontLibrary PARTIAL |

## Priority 8 other

28 files, 137 functions without a Rust symbol.

| X_ITE file | functions | no Rust symbol | name candidate only | no test | ledger entries |
| --- | --- | --- | --- | --- | --- |
| `src/x_ite/Browser/Geometry3D/IcoSphere.js` | 15 | 15 | 0 | 15 | none |
| `src/x_ite/Browser/Geometry3D/CylinderOptions.js` | 13 | 11 | 0 | 13 | none |
| `src/x_ite/Browser/Geometry3D/ConeOptions.js` | 12 | 10 | 0 | 12 | none |
| `src/x_ite/Browser/Interpolation/CatmullRomSplineInterpolator1.js` | 9 | 9 | 0 | 9 | none |
| `src/x_ite/Browser/Interpolation/CatmullRomSplineInterpolatorTemplate.js` | 9 | 9 | 0 | 9 | none |
| `src/x_ite/Browser/Geospatial/GeospatialObject.js` | 9 | 8 | 0 | 9 | none |
| `src/x_ite/Browser/KeyDeviceSensor/X3DKeyDeviceSensorContext.js` | 8 | 8 | 0 | 8 | none |
| `src/x_ite/Browser/Geometry2D/Circle2DOptions.js` | 7 | 7 | 0 | 7 | none |
| `src/x_ite/Browser/Geometry2D/X3DGeometry2DContext.js` | 9 | 7 | 0 | 9 | none |
| `src/x_ite/Browser/Geometry3D/QuadSphereOptions.js` | 9 | 7 | 0 | 9 | none |
| `src/x_ite/Browser/Geometry2D/Disk2DOptions.js` | 6 | 6 | 0 | 6 | none |
| `src/x_ite/Browser/Geometry3D/X3DGeometry3DContext.js` | 8 | 6 | 0 | 8 | none |
| `src/x_ite/Browser/Geometry3D/IcoSphereOptions.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Browser/X_ITE/ScatterSamples.js` | 5 | 5 | 0 | 5 | none |
| `src/x_ite/Browser/NURBS/NURBS.js` | 9 | 4 | 1 | 9 | none |
| `src/x_ite/Browser/Geospatial/Geocentric.js` | 4 | 3 | 0 | 4 | none |
| `src/x_ite/Components/GaussianSplats/X3DGaussianSplatsNode.js` | 3 | 3 | 0 | 3 | none |
| `src/x_ite/Browser/Geometry2D/Rectangle2DOptions.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Browser/ParticleSystems/X3DParticleSystemsContext.js` | 2 | 2 | 0 | 2 | none |
| `src/x_ite/Components/GaussianSplats/GaussianSplats.js` | 4 | 2 | 0 | 4 | node:GaussianSplats PARTIAL |
| `src/x_ite/Browser/Geometry2D/Arc2DOptions.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Browser/Geometry2D/ArcClose2DOptions.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Browser/Geometry3D/BoxOptions.js` | 3 | 1 | 0 | 3 | none |
| `src/x_ite/Browser/Grouping/X3DBBoxNode.js` | 3 | 1 | 0 | 3 | none |
| `src/x_ite/Browser/Interpolation/CatmullRomSplineInterpolator.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Browser/X_ITE/ExtensionKeys.js` | 1 | 1 | 0 | 1 | none |
| `src/x_ite/Browser/X_ITE/X3DX_ITEContext.js` | 4 | 1 | 0 | 4 | none |
| `src/x_ite/Components.js` | 1 | 1 | 0 | 1 | none |

Total: 4035 functions without a Rust symbol in 563 files.

Priority 1 of the directive (missing function mappings) is this whole file. Priority 5 (WebGL2 and WebGPU) is a property of the Rust renderer backends and has no X_ITE source area of its own.
