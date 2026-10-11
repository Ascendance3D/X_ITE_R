//! Node construction, setup and interest callbacks (X3DBaseNode.setup / initialize and the
//! `set_xxx__` methods of the node classes). Only behavior that is observable without frames,
//! rendering or a live scene is ported; see each module.

pub mod event_utils;
pub mod interp;
pub mod time;

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
}

/// X3DCast-like test against the X3D abstract type hierarchy for the types the runtime handles.
pub fn is_interpolator(name: &str) -> bool {
    matches!(
        name,
        "PositionInterpolator"
            | "OrientationInterpolator"
            | "ColorInterpolator"
            | "ScalarInterpolator"
            | "CoordinateInterpolator"
            | "PositionInterpolator2D"
            | "CoordinateInterpolator2D"
            | "NormalInterpolator"
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
        // is not loaded calls protoNode .requestImmediateLoad () (once per declaration).
        if let NodeKind::Instance(ProtoRef::Extern(e)) = self.scene.node(node).kind {
            self.extern_request_load(e);
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
            "Script" => self.nrt_mut(node).state = State::Script(Default::default()),
            "Inline" | "ImageTexture" | "MovieTexture" | "AudioClip" => self.nrt_mut(node).state = State::UrlObject(Default::default()),
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
            "Inline" | "ImageTexture" | "MovieTexture" | "AudioClip" => self.inline_initialize(node),
            "Background" => {
                self.interest(node, "set_bind", Cb::BindableSetBind);
                self.background_initialize(node)
            }
            "Viewpoint" | "OrthoViewpoint" | "NavigationInfo" | "Fog" | "LocalFog" => self.interest(node, "set_bind", Cb::BindableSetBind),
            "FontStyle" | "ScreenFontStyle" => self.font_style_initialize(node),
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

    /// Background.initialize: six private ImageTexture nodes, `xxxUrl .addFieldInterest
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
            Cb::UrlSetLoad | Cb::UrlSetUrl => self.url_object_changed(node, i.cb),
            Cb::ScriptField(_) | Cb::ScriptEventsProcessed | Cb::ScriptPrepareEvents => self.script_interest(node, i.cb, k),
            Cb::GroupAddChildren => self.group_add_children(node),
            Cb::GroupRemoveChildren => self.group_remove_children(node),
            Cb::FollowerSetValue | Cb::FollowerSetDestination => {}
            Cb::TimeSensorSetPrepare => self.time_sensor_set_prepare(node),
            Cb::TimeSensorSetTime => self.time_sensor_set_time(node),
            Cb::BindableSetBind => {
                // X3DBindableNode.set_bind__: modificationCount = ++modificationCount.
                self.bind_counter += 1;
                self.bind_modifications.insert(node, self.bind_counter);
            }
        }
    }
}
