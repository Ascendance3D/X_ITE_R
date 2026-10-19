//! Node construction, setup and interest callbacks (X3DBaseNode.setup / initialize and the
//! `set_xxx__` methods of the node classes). Only behavior that is observable without frames,
//! rendering or a live scene is ported; see each module.

pub mod event_utils;
pub mod followers;
pub mod interp;
pub mod key_sensor;
pub mod load_sensor;
pub mod time;
pub mod transform_sensor;
pub mod pick_sensor;
pub mod geo;
pub mod texture_projector;
pub mod nurbs;
pub mod viewpoint_group;
pub mod hanim;
pub mod particle_systems;
pub mod rigid_body_physics;
pub mod sound;
pub mod image_texture_3d;
pub mod shaders;

use crate::*;

#[derive(Debug, Default)]
pub enum State {
    #[default]
    None,
    Time(time::TimeState),
    Sequencer { index: i64 },
    ColorInterp { hsv: Vec<[f64; 3]> },
    Script(script::ScriptState),
    UrlObject(UrlState),
    Follower(Box<dyn followers::Follower>),
    /// CatmullRomSplineInterpolator T0 / T1 (n components each).
    Spline { t0: Vec<Vec<f64>>, t1: Vec<Vec<f64>> },
    /// SquadInterpolator inner control points.
    Squad { s: Vec<x3d_fields::math::Rotation4> },
    LoadSensor(load_sensor::LoadSensorState),
    /// ComposedCubeMapTexture.textureNodes: the face textures by slot (front, back, left, right,
    /// top, bottom at initialize; see [`Runtime::composed_cube_faces`]).
    ComposedCube([Option<NodeId>; 6]),
    /// X3DTextureProjectorNode.textureNode.
    Projector { texture: Option<NodeId> },
    /// ComposedTexture3D.textureNodes.
    Composed3D(Vec<NodeId>),
    /// IsoSurfaceVolumeData and SegmentedVolumeData capture `voxels` during initialize ().
    Volume { voxels: Option<NodeId> },
    /// RenderedTexture: checkUpdate, lastUpdate (Date.now () ms), the time of the first
    /// renderTexture (`None` before), and the headlights depth map passes left in the
    /// DependentRenderer's global lights (`leaks`; `display_leaks`: those of this frame's pass).
    Rendered { check_update: bool, last_update_ms: f64, first_ms: Option<f64>, leaks: u32, display_leaks: u32 },
    /// The RigidBodyPhysics nodes: Bullet objects, members of the node classes (rigid_body_physics.rs).
    Physics(rigid_body_physics::PhysState),
    /// ParticleSystem: the particle buffer and counters (the emitters and physics models it
    /// hosts have no state of their own).
    ParticleSystem(Box<particle_systems::PsState>),
}

/// X3DUrlObject load state (X3DConstants NOT_STARTED_STATE … FAILED_STATE).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoadState {
    #[default]
    NotStarted,
    InProgress,
    Complete,
    Failed,
}

#[derive(Debug, Default)]
pub struct UrlState {
    pub load_state: LoadState,
    /// Identity of the current load request (an Inline's FileLoader). It grows whenever a request
    /// is replaced or cancelled (FileLoader.abort), so a result of an older request is recognized.
    pub generation: u64,
    /// `Date .now ()` (ms) at construction and when the load state last became COMPLETE / FAILED
    /// (X3DUrlObject [_autoRefreshStartTime] / [_autoRefreshCompleteTime]).
    pub start_ms: f64,
    pub complete_ms: f64,
    /// Due time (ms) of the auto refresh timer, if one is set.
    pub timer: Option<f64>,
    /// The scene of this node was shut down (Scripts called `shutdown ()`, interests removed).
    pub scene_closed: bool,
    /// The `url` of the last load request (requestImmediateLoad set IN_PROGRESS_STATE): what an
    /// ImageTexture shows. A url change without a load (LoadUrlObjects FALSE) keeps it.
    pub requested_url: Option<Vec<String>>,
}

/// X3DCast-like test against the X3D abstract type hierarchy for the types the runtime handles.
pub fn is_interpolator(name: &str) -> bool {
    matches!(
        name,
        "PositionInterpolator"
            | "GeoPositionInterpolator"
            | "OrientationInterpolator"
            | "ColorInterpolator"
            | "ScalarInterpolator"
            | "CoordinateInterpolator"
            | "PositionInterpolator2D"
            | "CoordinateInterpolator2D"
            | "NormalInterpolator"
            | "EaseInEaseOut"
            | "SplinePositionInterpolator"
            | "SplinePositionInterpolator2D"
            | "SplineScalarInterpolator"
            | "SquadOrientationInterpolator"
    )
}

/// `set_x .addFieldInterest (x)` made by geometry nodes in initialize (Components/*/<Type>.js).
const GEOMETRY_FIELD_INTERESTS: &[(&str, &[(&str, &str)])] = &[
    ("Extrusion", &[("set_crossSection", "crossSection"), ("set_orientation", "orientation"), ("set_scale", "scale"), ("set_spine", "spine")]),
    ("ElevationGrid", &[("set_height", "height")]),
    ("GeoElevationGrid", &[("set_height", "height")]),
    (
        "IndexedFaceSet",
        &[("set_colorIndex", "colorIndex"), ("set_coordIndex", "coordIndex"), ("set_normalIndex", "normalIndex"), ("set_texCoordIndex", "texCoordIndex")],
    ),
    ("IndexedLineSet", &[("set_colorIndex", "colorIndex"), ("set_coordIndex", "coordIndex")]),
    ("IndexedTriangleFanSet", &[("set_index", "index")]),
    ("IndexedTriangleSet", &[("set_index", "index")]),
    ("IndexedTriangleStripSet", &[("set_index", "index")]),
    ("IndexedQuadSet", &[("set_index", "index")]),
];

impl Runtime {
    /// Constructor side effects: private child objects (X3DBaseNode.addChildObjects).
    pub fn construct_node(&mut self, node: NodeId) {
        // X3DPrototypeInstance constructor → construct (): an instance of an extern proto that
        // is not loaded asks for the load (once per declaration); a loaded one takes the PROTO's
        // field defaults now and realizes its body at setup.
        if let NodeKind::Instance(ProtoRef::Extern(_)) = self.scene.node(node).kind {
            self.extern_instance_construct(node);
            return;
        }

        let Some(name) = self.builtin_name(node) else {
            return;
        };

        match name {
            "TimeSensor" => {
                self.add_private_field(node, "initialized", access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
                self.add_private_field(node, "evenLive", access::INPUT_OUTPUT, FieldValue::SFBool(false));
                self.add_private_field(node, "range", access::INPUT_OUTPUT, FieldValue::MFFloat(vec![0.0, 0.0, 1.0]));
                self.nrt_mut(node).state = State::Time(time::TimeState::default());
            }
            "IntegerSequencer" | "BooleanSequencer" => self.nrt_mut(node).state = State::Sequencer { index: -1 },
            "ColorInterpolator" => self.nrt_mut(node).state = State::ColorInterp { hsv: vec![] },
            "Script" => {
                self.add_private_field(node, "loadState", access::OUTPUT_ONLY, FieldValue::SFInt32(0));
                self.add_private_field(node, "loadData", access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
                self.nrt_mut(node).state = State::Script(Default::default())
            }
            "LoadSensor" => self.nrt_mut(node).state = State::LoadSensor(Default::default()),
            n if followers::make(n).is_some() => self.nrt_mut(node).state = State::Follower(followers::make(n).unwrap()),
            "Inline" | "InlineGeometry" => {
                self.add_private_field(node, "loadState", access::OUTPUT_ONLY, FieldValue::SFInt32(0));
                self.add_private_field(node, "loadData", access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
                let now_ms = self.now * 1000.0;
                self.nrt_mut(node).state = State::UrlObject(UrlState { start_ms: now_ms, complete_ms: now_ms, ..Default::default() });
            }
            "ComposedCubeMapTexture" => self.nrt_mut(node).state = State::ComposedCube([None; 6]),
            "HAnimHumanoid" => self.hanim_humanoid_construct(node),
            n if sound::is_sound_node(n) => self.sound_construct(node, n),
            "RenderedTexture" => self.nrt_mut(node).state = State::Rendered { check_update: false, last_update_ms: -1.0, first_ms: None, leaks: 0, display_leaks: 0 },
            n if rigid_body_physics::is_physics_node(n) => self.physics_construct(node, n),
            "ParticleSystem" => self.nrt_mut(node).state = State::ParticleSystem(Default::default()),
            "ComposedTexture3D" => {
                self.add_private_field(node, "update", access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
                self.nrt_mut(node).state = State::Composed3D(vec![]);
            }
            "FontLibrary" => {
                self.add_private_field(node, "loadState", access::OUTPUT_ONLY, FieldValue::SFInt32(0));
                self.add_private_field(node, "loadData", access::OUTPUT_ONLY, FieldValue::SFTime(0.0));
                self.nrt_mut(node).state = State::UrlObject(Default::default())
            }
            // The SpecularGlossinessMaterial constructor.
            "SpecularGlossinessMaterial" => self.log_warn("SpecularGlossinessMaterial is deprecated, please use PhysicalMaterial instead."),
            "ComposedShader" => self.composed_shader_construct(node),
            "ImageTexture" | "MovieTexture" | "AudioClip" | "ImageCubeMapTexture" | "ImageTextureAtlas" | "ImageTexture3D" | "ShaderPart" => {
                self.add_private_field(node, "loadState", access::OUTPUT_ONLY, FieldValue::SFInt32(0));
                self.nrt_mut(node).state = State::UrlObject(Default::default())
            }
            _ => {}
        }
    }

    /// X3DBaseNode.setup (with the overrides that run code before it, e.g. X3DInterpolatorNode).
    pub fn setup_node(&mut self, node: NodeId) {
        if self.nrt(node).is_some_and(|n| n.setup_started) {
            return;
        }

        if let Some(name) = self.builtin_name(node) {
            // X3DInterpolatorNode.setup: set_key__ (); set_fraction__ (); before X3DBaseNode.setup.
            if is_interpolator(name) {
                self.interp_set_key(node);
                self.interp_set_fraction(node);
            }
        }

        // X3DBaseNode.setup: untaint every field, then initialize.
        self.nrt_mut(node).setup_started = true;

        let n = self.scene.node(node);
        let keys: Vec<FKey> = (0..n.fields.len())
            .map(FieldLoc::Pre)
            .chain((0..n.user_fields.len()).map(FieldLoc::User))
            .chain((0..n.private_fields.len()).map(FieldLoc::Priv))
            .map(|l| (node, l))
            .collect();

        for k in keys {
            self.set_field_tainted(k, false);
        }

        self.initialize_node(node);
        self.nrt_mut(node).initialized = true;
    }

    fn initialize_node(&mut self, node: NodeId) {
        if let NodeKind::Instance(_) = self.scene.node(node).kind {
            self.instance_initialize(node);
            return;
        }

        let Some(name) = self.builtin_name(node) else {
            return;
        };

        match name {
            "IsoSurfaceVolumeData" | "SegmentedVolumeData" => {
                let voxels = self.pre_value(node, "voxels").and_then(|v| match v {
                    FieldValue::SFNode(n) => *n,
                    _ => None,
                });
                self.nrt_mut(node).state = State::Volume { voxels };
            }
            "TimeSensor" => self.time_sensor_initialize(node),
            n if is_interpolator(n) => self.interp_initialize(node, n),
            "BooleanFilter" => self.interest(node, "set_boolean", Cb::BooleanFilterSetBoolean),
            "BooleanToggle" => self.interest(node, "set_boolean", Cb::BooleanToggleSetBoolean),
            "BooleanTrigger" => self.interest(node, "set_triggerTime", Cb::BooleanTriggerSetTriggerTime),
            "IntegerTrigger" => self.interest(node, "set_boolean", Cb::IntegerTriggerSetBoolean),
            "TimeTrigger" => self.interest(node, "set_boolean", Cb::TimeTriggerSetBoolean),
            "IntegerSequencer" | "BooleanSequencer" => {
                self.interest(node, "set_fraction", Cb::SequencerSetFraction);
                self.interest(node, "previous", Cb::SequencerSetPrevious);
                self.interest(node, "next", Cb::SequencerSetNext);
                self.interest(node, "key", Cb::SequencerSetKey);
                self.interest(node, "keyValue", Cb::SequencerSetKey);
            }
            "Script" => self.script_initialize(node),
            "LoadSensor" => self.load_sensor_initialize(node),
            "KeySensor" | "StringSensor" => self.key_sensor_initialize(node),
            "TransformSensor" => self.transform_sensor_initialize(node),
            "LinePickSensor" | "PointPickSensor" | "PrimitivePickSensor" | "VolumePickSensor" => self.pick_sensor_initialize(node),
            // X3DLayerNode: addChildren / removeChildren as X3DGroupingNode, and the objectType
            // forwarded to the layer's PickableGroup.
            "Layer" | "LayoutLayer" => {
                self.interest(node, "addChildren", Cb::GroupAddChildren);
                self.interest(node, "removeChildren", Cb::GroupRemoveChildren);
                self.interest(node, "objectType", Cb::LayerSetObjectType);
            }
            // WorldInfo.initialize: `if (this .isLive ()) executionContext .addWorldInfo (this)`.
            // isLive () is the node's own live flag (X3DBaseNode [_live], true until dispose), not
            // the scene's, so the node is added at once. X_ITE never disposes scene nodes one by
            // one; the entry goes when the node is released (retention cleanup).
            "WorldInfo" => self.world_info_set_live(node, true),
            // GeoProximitySensor: `_geoCenter .addFieldInterest (this ._center)`.
            "GeoProximitySensor" => self.interest(node, "geoCenter", Cb::GeoCenterToCenter),
            n if followers::make(n).is_some() => self.follower_initialize(node),
            // X3DTexture2DNode.initialize (clearTexture), then X3DUrlObject's.
            "ImageTexture" | "MovieTexture" => {
                self.set_texture_2d_size(node, 1, 1);
                self.inline_initialize(node)
            }
            "Inline" | "InlineGeometry" | "AudioClip" | "ImageCubeMapTexture" | "ImageTextureAtlas" | "ImageTexture3D" => self.inline_initialize(node),
            "FontLibrary" => self.font_library_initialize(node),
            "ShaderPart" => self.shader_part_initialize(node),
            "ComposedShader" => self.composed_shader_initialize(node),
            "Appearance" => self.appearance_shaders_initialize(node),
            "PixelTexture" => {
                self.set_texture_2d_size(node, 1, 1);
                self.interest(node, "image", Cb::PixelTextureSetImage);
                self.pixel_texture_set_image(node);
            }
            "TextureProjector" | "TextureProjectorParallel" => self.texture_projector_initialize(node),
            "ComposedTexture3D" => self.composed_3d_initialize(node),
            "RenderedTexture" => self.rendered_texture_initialize(node),
            "ComposedCubeMapTexture" => {
                // ComposedCubeMapTexture.initialize: the interests fill slots 0 1 2 3 5 4 (top and
                // bottom swapped), the initial set_texture__ calls slots 0 … 5.
                for (i, (field, slot)) in [("frontTexture", 0), ("backTexture", 1), ("leftTexture", 2), ("rightTexture", 3), ("topTexture", 5), ("bottomTexture", 4)].into_iter().enumerate() {
                    self.interest(node, field, Cb::ComposedCubeSetTexture(slot));
                    let face = self.pre_value(node, field).and_then(|v| match v {
                        FieldValue::SFNode(n) => *n,
                        _ => None,
                    });
                    if let State::ComposedCube(faces) = &mut self.nrt_mut(node).state {
                        faces[i] = face;
                    }
                }
            }
            "Background" => {
                self.interest(node, "set_bind", Cb::BindableSetBind);
                self.background_initialize(node)
            }
            "Viewpoint" | "OrthoViewpoint" | "GeoViewpoint" => {
                self.interest(node, "set_bind", Cb::BindableSetBind);
                self.interest(node, "isBound", Cb::ViewpointIsBound);

                if name == "GeoViewpoint" {
                    self.geo_viewpoint_initialize(node);
                }
            }
            "NavigationInfo" | "Fog" | "LocalFog" => self.interest(node, "set_bind", Cb::BindableSetBind),
            "GeoLOD" => self.geo_lod_initialize(node),
            "ViewpointGroup" => self.viewpoint_group_initialize(node),
            "HAnimHumanoid" => self.hanim_humanoid_initialize(node),
            "ParticleSystem" => self.particle_system_initialize(node),
            n if rigid_body_physics::is_physics_node(n) => self.physics_initialize(node, n),
            "VolumeEmitter" => self.volume_emitter_initialize(node),
            "PolylineEmitter" => self.polyline_emitter_initialize(node),
            n if sound::is_sound_node(n) => self.sound_initialize(node, n),
            "HAnimPose" => self.hanim_pose_initialize(node),
            "HAnimMotion" => self.hanim_motion_initialize(node),
            "FontStyle" | "ScreenFontStyle" => self.font_style_initialize(node),
            "Contour2D" | "NurbsSet" | "NurbsTrimmedSurface" | "NurbsPositionInterpolator" | "NurbsOrientationInterpolator" | "NurbsSurfaceInterpolator" => self.nurbs_initialize(node, name),
            _ if !GEOMETRY_FIELD_INTERESTS.iter().any(|(t, _)| *t == name) => {
                // X3DGroupingNode.initialize
                if self.pre(node, "addChildren").is_some() && self.pre(node, "removeChildren").is_some() && self.pre(node, "children").is_some() {
                    self.interest(node, "addChildren", Cb::GroupAddChildren);
                    self.interest(node, "removeChildren", Cb::GroupRemoveChildren);
                }
            }
            _ => {
                // e.g. Extrusion.initialize: this ._set_spine .addFieldInterest (this ._spine).
                for (_, pairs) in GEOMETRY_FIELD_INTERESTS.iter().filter(|(t, _)| *t == name) {
                    for (from, to) in pairs.iter() {
                        if let (Some(f), Some(t)) = (self.pre(node, from), self.pre(node, to)) {
                            self.add_field_interest(f, t);
                        }
                    }
                }
            }
        }
    }

    /// ComposedCubeMapTexture.textureNodes: the face texture of each slot, in the order
    /// X3DEnvironmentTextureNode.targets fills them (+Z, -Z, -X, +X, +Y, -Y). `None` for another
    /// node.
    pub fn composed_cube_faces(&self, node: NodeId) -> Option<[Option<NodeId>; 6]> {
        match self.nrt(node).map(|n| &n.state) {
            Some(State::ComposedCube(faces)) => Some(*faces),
            _ => None,
        }
    }

    /// Background.initialize: six private ImageTexture nodes ([`crate::Runtime::background_textures`]), `xxxUrl .addFieldInterest
    /// (texture ._url)`, `texture ._url = xxxUrl`, `texture .setup ()` (which starts the load).
    fn background_initialize(&mut self, node: NodeId) {
        let Some(&index) = self.scene.table.by_name.get("ImageTexture") else {
            return;
        };
        let ctx = self.scene.node(node).ctx;

        for name in ["frontUrl", "backUrl", "leftUrl", "rightUrl", "topUrl", "bottomUrl"] {
            let Some(from) = self.pre(node, name) else {
                continue;
            };

            let texture = self.scene.instantiate_builtin(ctx, index);
            self.nrt_mut(texture);
            self.construct_node(texture);
            self.background_textures.entry(node).or_default().push((name, texture));

            let Some(url) = self.pre(texture, "url") else {
                continue;
            };

            self.add_field_interest(from, url);

            let value = self.value(from).clone();
            self.set_raw(url, value);
            self.setup_node(texture);
        }
    }

    fn mfnode(&self, node: NodeId, name: &str) -> Vec<Option<NodeId>> {
        match self.pre_value(node, name) {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        }
    }

    /// X3DGroupingNode.set_addChildren__.
    fn group_add_children(&mut self, node: NodeId) {
        let add = self.mfnode(node, "addChildren");

        if add.is_empty() {
            return;
        }

        let add_key = self.pre(node, "addChildren").unwrap();
        let children_key = self.pre(node, "children").unwrap();

        self.set_field_tainted(add_key, true);

        // new Set (addChildren), minus the current children, in insertion order.
        let mut set: Vec<Option<NodeId>> = vec![];
        for n in add {
            if !set.contains(&n) {
                set.push(n);
            }
        }

        let children = self.mfnode(node, "children");
        set.retain(|n| !children.contains(n));

        for child in set {
            let mut v = self.mfnode(node, "children");
            v.push(child);
            self.set_value(children_key, FieldValue::MFNode(v));
        }

        self.set_raw(add_key, FieldValue::MFNode(vec![]));
        self.set_field_tainted(add_key, false);
    }

    /// X3DGroupingNode.set_removeChildren__.
    fn group_remove_children(&mut self, node: NodeId) {
        let remove = self.mfnode(node, "removeChildren");

        if remove.is_empty() {
            return;
        }

        let remove_key = self.pre(node, "removeChildren").unwrap();
        let children_key = self.pre(node, "children").unwrap();

        self.set_field_tainted(remove_key, true);

        let children = self.mfnode(node, "children");

        if !children.is_empty() {
            let filtered: Vec<Option<NodeId>> = children.into_iter().filter(|c| !remove.contains(c)).collect();
            self.set_value(children_key, FieldValue::MFNode(filtered));
        }

        self.set_raw(remove_key, FieldValue::MFNode(vec![]));
        self.set_field_tainted(remove_key, false);
    }

    /// `this ._name .addInterest (cb, this)` for a predefined field.
    pub fn interest(&mut self, node: NodeId, field: &str, cb: Cb) {
        if let Some(k) = self.pre(node, field) {
            self.add_interest(k, node, cb);
        }
    }

    /// Dispatch an interest callback. `k` is the field whose event called it.
    pub fn call_interest(&mut self, i: Interest, k: FKey) {
        let node = i.node;

        match i.cb {
            Cb::TimeSetLoop => self.time_set_loop(node),
            Cb::TimeSetEnabled => self.time_set_enabled(node),
            Cb::TimeSetStartTime => self.time_set_start_time(node),
            Cb::TimeSetPauseTime => self.time_set_pause_time(node),
            Cb::TimeSetResumeTime => self.time_set_resume_time(node),
            Cb::TimeSetStopTime => self.time_set_stop_time(node),
            Cb::TimeSensorSetCycleInterval => self.time_sensor_set_cycle_interval(node),
            Cb::TimeSensorSetRange => self.time_sensor_set_range(node),
            Cb::InterpSetFraction => self.interp_set_fraction(node),
            Cb::InterpSetKey => self.interp_set_key(node),
            Cb::InterpSetKeyValue => self.interp_set_key_value(node),
            Cb::SplineSetKeyVelocity => self.spline_set_key_velocity(node),
            Cb::SplineSetNormalizeVelocity => self.spline_set_normalize_velocity(node),
            Cb::BooleanFilterSetBoolean
            | Cb::BooleanToggleSetBoolean
            | Cb::BooleanTriggerSetTriggerTime
            | Cb::IntegerTriggerSetBoolean
            | Cb::TimeTriggerSetBoolean
            | Cb::SequencerSetFraction
            | Cb::SequencerSetPrevious
            | Cb::SequencerSetNext
            | Cb::SequencerSetKey
            | Cb::SequencerSetKeyValue => self.event_utils_call(node, i.cb),
            Cb::UrlSetLoad | Cb::UrlSetUrl | Cb::UrlLoadData | Cb::UrlSetAutoRefresh => self.url_object_changed(node, i.cb),
            Cb::InlineGeometrySolid => self.inline_geometry_set_solid(node),
            Cb::InlineGeometrySmooth => self.inline_geometry_set_smooth(node),
            Cb::FontLibrarySetFamily => self.font_library_set_family(node),
            Cb::FontStyleReload => self.tasks.push_back(Task::UrlLoad(node, 0)),
            Cb::SceneLoadCount => self.scene_load_count(node),
            Cb::RouteReconnect(id) => self.reconnect_route(id),
            Cb::ScriptField(_) | Cb::ScriptEventsProcessed | Cb::ScriptPrepareEvents => self.script_interest(node, i.cb, k),
            Cb::LoadSensorSetEnabled | Cb::LoadSensorSetTimeOut | Cb::LoadSensorSetChildren | Cb::LoadSensorLoadState => self.load_sensor_interest(node, i.cb, k),
            Cb::KeySensorSetEnabled => self.key_set_enabled(node),
            Cb::TransformSensorSetEnabled => self.transform_sensor_set_enabled(node),
            Cb::PickSensorSetLive => self.pick_sensor_set_live(node),
            Cb::LayerSetObjectType => {
                let types = self.pick_sensor_object_type(node);
                self.layer_object_types.insert(node, types);
            }
            Cb::GeoCenterToCenter => {
                if let (Some(v), Some(k)) = (self.pre_value(node, "geoCenter").cloned(), self.pre(node, "center")) {
                    self.set_value(k, v);
                }
            }
            Cb::ComposedCubeSetTexture(slot) => {
                let face = match self.value(k) {
                    FieldValue::SFNode(n) => *n,
                    _ => None,
                };
                if let State::ComposedCube(faces) = &mut self.nrt_mut(node).state {
                    faces[slot as usize] = face;
                }
            }
            Cb::ProjectorSetTexture => self.texture_projector_set_texture(node),
            Cb::ProjectorSetTextureNode => self.texture_projector_set_texture_node(node),
            Cb::PixelTextureSetImage => self.pixel_texture_set_image(node),
            Cb::Composed3DSetTexture => self.composed_3d_set_texture(node),
            Cb::Composed3DSliceEvent => {
                if let Some(k) = self.private_field(node, "update") {
                    self.add_event(k);
                }
            }
            Cb::Composed3DUpdate => self.composed_3d_update(node),
            Cb::RenderedResetUpdate => self.rendered_texture_reset_update(node),
            Cb::RenderedSetDimensions => self.rendered_texture_set_dimensions(node),
            Cb::NurbsFilterAdd(i) => self.nurbs_filter_add(node, i as usize),
            Cb::NurbsFilterRemove(i) => self.nurbs_filter_remove(node, i as usize),
            Cb::NurbsSetScale => self.nurbs_set_scale(node),
            Cb::ViewpointGroupSetSize => self.viewpoint_group_set_size(node),
            Cb::HAnimHumanoidSetChildren => self.hanim_humanoid_set_children(node),
            Cb::HAnimHumanoidSetMotions => self.hanim_humanoid_set_motions(node),
            Cb::HAnimHumanoidSetJoints => self.hanim_humanoid_set_joints(node),
            Cb::HAnimHumanoidSetSkinCoord => self.hanim_humanoid_set_skin_coord(node),
            Cb::HAnimHumanoidPoseFraction(pose) => self.hanim_humanoid_pose_fraction(node, pose),
            Cb::HAnimHumanoidPoseActive => self.hanim_humanoid_pose_active(node, k),
            Cb::HAnimHumanoidConnectJoints(motion) => self.hanim_humanoid_connect_joints(node, motion),
            Cb::HAnimHumanoidConnectAllJoints => self.hanim_humanoid_connect_all_joints(node),
            Cb::HAnimHumanoidJointName => self.hanim_humanoid_joint_name(node),
            Cb::HAnimHumanoidUnlock => self.hanim_humanoid_unlock(node),
            Cb::HAnimHumanoidDeprecated => {
                let field = if self.pre(node, "segments") == Some(k) { "segments" } else { "sites" };
                self.hanim_humanoid_deprecated(node, field)
            }
            Cb::HAnimPoseResetAllJoints => self.hanim_pose_reset_all_joints(node),
            Cb::HAnimPoseCommence => self.hanim_pose_commence(node),
            Cb::HAnimPoseStartTime => self.hanim_pose_set_start_time(node),
            Cb::HAnimPoseFraction => self.hanim_pose_set_fraction(node),
            Cb::HAnimPoseChildren => self.hanim_pose_set_children(node),
            Cb::HAnimMotionEnabled => self.hanim_motion_set_enabled(node),
            Cb::HAnimMotionJoints => self.hanim_motion_set_joints(node),
            Cb::HAnimMotionInterpolators => self.hanim_motion_set_interpolators(node),
            Cb::HAnimMotionNextOrPrevious(direction) => self.hanim_motion_next_or_previous(node, direction as i32, k),
            Cb::HAnimMotionFrameIndex => self.hanim_motion_set_frame_index(node),
            Cb::HAnimMotionFrameDuration => self.hanim_motion_set_frame_duration(node),
            Cb::HAnimMotionFrameIncrement => self.hanim_motion_set_frame_increment(node),
            Cb::HAnimMotionStartOrEndFrame => self.hanim_motion_set_start_or_end_frame(node),
            Cb::ViewpointGroupSetActive => self.viewpoint_group_set_active(node),
            Cb::NurbsSetGeometry => self.nurbs_set_geometry(node),
            Cb::NurbsInterpSetFraction => self.nurbs_interp_set_fraction(node),
            Cb::GroupAddChildren => self.group_add_children(node),
            Cb::GroupRemoveChildren => self.group_remove_children(node),
            Cb::FollowerSetValue | Cb::FollowerSetDestination | Cb::FollowerSetDuration | Cb::FollowerPrepare => self.follower_interest(node, i.cb),
            Cb::TimeSensorSetPrepare => self.time_sensor_set_prepare(node),
            Cb::TimeSensorSetTime => self.time_sensor_set_time(node),
            Cb::BindableSetBind => {
                // X3DBindableNode.set_bind__: modificationCount = ++modificationCount.
                self.bind_counter += 1;
                self.bind_modifications.insert(node, self.bind_counter);
            }
            Cb::GeoLodRootLoadState => self.geo_lod_set_root_load_state(node),
            Cb::GeoLodChildLoadState => self.geo_lod_set_child_load_state(node),
            Cb::Sound(cb) => self.sound_interest_call(node, cb),
            Cb::Particle(cb) => self.particle_interest(node, cb),
            Cb::Physics(cb) => self.physics_interest(node, cb),
            Cb::Shader(cb) => self.shader_interest(node, cb),
            Cb::ViewpointIsBound => {
                if let (Some(nav), Some(bound)) = (self.viewpoint_navigation_info(node), self.pre(node, "isBound")) {
                    let value = self.value(bound).clone();

                    if let Some(k) = self.pre(nav, "set_bind") {
                        self.set_value(k, value);
                    }
                }
            }
        }
    }
}
