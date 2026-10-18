//! Components/Networking/LoadSensor.js: watches the load state of the X3DUrlObject nodes in its
//! `children` (VRML `watchList`) and reports isActive, isLoaded, progress and loadTime.
//! The `timeOut` timer is a virtual-time timeout like X3DTimeDependentNode's setTimeout.

use crate::*;
use x3d_fields::jsmath;

#[derive(Debug, Default)]
pub struct LoadSensorState {
    /// this .urlObjects, in children order.
    pub url_objects: Vec<NodeId>,
    pub aborted: bool,
}

/// X3DCast (X3DConstants .X3DUrlObject, node): the concrete node types that inherit X3DUrlObject.
/// InlineGeometry is not in the list although it is one: X3DCast goes through `getInnerNode ()`, which is the
/// loaded geometry (or throws), so a LoadSensor never watches an InlineGeometry (REA batch 08, lane net).
pub fn is_url_object(name: &str) -> bool {
    matches!(
        name,
        "Anchor"
            | "AudioClip"
            | "BufferAudioSource"
            | "DISEntityTypeMapping"
            | "FontLibrary"
            | "FontStyle"
            | "GeoMetadata"
            | "IconAnnotation"
            | "ImageCubeMapTexture"
            | "ImageTexture"
            | "ImageTexture3D"
            | "ImageTextureAtlas"
            | "Inline"
            | "MovieTexture"
            | "PackagedShader"
            | "ScreenFontStyle"
            | "Script"
            | "ShaderPart"
            | "ShaderProgram"
    )
}

impl Runtime {
    fn load_sensor_state(&mut self, node: NodeId) -> Option<&mut LoadSensorState> {
        match &mut self.nrt_mut(node).state {
            nodes::State::LoadSensor(s) => Some(s),
            _ => None,
        }
    }

    fn ls_set(&mut self, node: NodeId, name: &str, value: FieldValue) {
        if let Some(k) = self.pre(node, name) {
            self.set_value(k, value);
        }
    }

    /// LoadSensor.initialize.
    pub(crate) fn load_sensor_initialize(&mut self, node: NodeId) {
        self.interest(node, "enabled", Cb::LoadSensorSetEnabled);
        self.interest(node, "timeOut", Cb::LoadSensorSetTimeOut);
        self.interest(node, "children", Cb::LoadSensorSetChildren);

        self.load_sensor_reset(node);
    }

    pub(crate) fn load_sensor_interest(&mut self, node: NodeId, cb: Cb, k: FKey) {
        match cb {
            Cb::LoadSensorSetEnabled => {
                if self.get_bool(node, "enabled") {
                    self.load_sensor_reset(node);
                } else {
                    self.load_sensor_abort(node);
                    self.load_sensor_remove(node);
                }
            }
            Cb::LoadSensorSetTimeOut => self.load_sensor_set_time_out(node),
            Cb::LoadSensorSetChildren => self.load_sensor_reset(node),
            Cb::LoadSensorLoadState => {
                // set_loadState__ (urlObject): the url object is the owner of the field.
                if self.url_load_state(k.0) == nodes::LoadState::NotStarted {
                    return;
                }

                self.load_sensor_count(node);
            }
            _ => {}
        }
    }

    /// set_timeOut__.
    fn load_sensor_set_time_out(&mut self, node: NodeId) {
        if !self.get_bool(node, "isActive") {
            return;
        }

        let time_out = self.get_f64(node, "timeOut");

        self.load_sensor_clear_timeout(node);

        if let Some(s) = self.load_sensor_state(node) {
            s.aborted = false;
        }

        if time_out > 0.0 {
            let due = self.now + time_out;
            self.add_timeout(node, "abort", due);
        }
    }

    fn load_sensor_clear_timeout(&mut self, node: NodeId) {
        self.remove_timeout(node, "abort");
    }

    /// count ().
    fn load_sensor_count(&mut self, node: NodeId) {
        let Some(s) = self.load_sensor_state(node) else {
            return;
        };
        let url_objects = s.url_objects.clone();
        let aborted = s.aborted;

        if url_objects.is_empty() {
            self.ls_set(node, "isActive", FieldValue::SFBool(false));
            self.ls_set(node, "isLoaded", FieldValue::SFBool(false));
            self.ls_set(node, "progress", FieldValue::SFFloat(0.0));
            return;
        }

        let states: Vec<_> = url_objects.iter().map(|&n| self.url_load_state(n)).collect();
        let complete = states.iter().filter(|s| **s == nodes::LoadState::Complete).count();
        let failed = states.iter().filter(|s| **s == nodes::LoadState::Failed).count();
        let loaded = complete == url_objects.len();
        // SFFloat stores Math.fround of the double quotient.
        let progress = jsmath::fround(complete as f64 / url_objects.len() as f64);

        if aborted || failed > 0 || loaded {
            self.load_sensor_clear_timeout(node);

            self.ls_set(node, "isActive", FieldValue::SFBool(false));
            self.ls_set(node, "isLoaded", FieldValue::SFBool(loaded));
            self.ls_set(node, "progress", FieldValue::SFFloat(progress));

            if loaded {
                let now = self.now;
                self.ls_set(node, "loadTime", FieldValue::SFTime(now));
            }
        } else if self.get_bool(node, "isActive") {
            self.ls_set(node, "progress", FieldValue::SFFloat(progress));
        } else {
            self.ls_set(node, "isActive", FieldValue::SFBool(true));
            self.ls_set(node, "progress", FieldValue::SFFloat(progress));

            self.load_sensor_set_time_out(node);
        }
    }

    /// abort () (also the timeOut callback).
    pub(crate) fn load_sensor_abort(&mut self, node: NodeId) {
        self.load_sensor_clear_timeout(node);

        if let Some(s) = self.load_sensor_state(node) {
            s.aborted = true;
        }

        if self.get_bool(node, "enabled") {
            self.load_sensor_count(node);
        }
    }

    /// reset ().
    fn load_sensor_reset(&mut self, node: NodeId) {
        self.load_sensor_remove(node);

        if !self.get_bool(node, "enabled") {
            return;
        }

        let children: Vec<NodeId> = match self.pre_value(node, "children") {
            Some(FieldValue::MFNode(list)) => list.iter().flatten().copied().collect(),
            _ => vec![],
        };
        let url_objects: Vec<NodeId> = children.into_iter().filter_map(|n| self.cast_url_object(n)).collect();

        for &u in &url_objects {
            let k = self.add_private_field(u, "loadState", access::OUTPUT_ONLY, FieldValue::SFInt32(url_state_value(self.url_load_state(u))));
            self.add_interest(k, node, Cb::LoadSensorLoadState);
        }

        if let Some(s) = self.load_sensor_state(node) {
            s.url_objects = url_objects;
        }

        self.load_sensor_count(node);
    }

    /// remove ().
    fn load_sensor_remove(&mut self, node: NodeId) {
        self.load_sensor_clear_timeout(node);

        let url_objects = self.load_sensor_state(node).map(|s| std::mem::take(&mut s.url_objects)).unwrap_or_default();

        for u in url_objects {
            if let Some(k) = self.private_field(u, "loadState") {
                self.remove_interest(k, node, Cb::LoadSensorLoadState);
            }
        }
    }

    /// X3DCast (X3DConstants .X3DUrlObject, node): getInnerNode () of a PROTO instance is the
    /// inner node of the first root node of its body (none without a body).
    fn cast_url_object(&self, node: NodeId) -> Option<NodeId> {
        let mut n = node;

        for _ in 0..256 {
            if let NodeKind::Instance(_) = self.scene.node(n).kind {
                n = self.scene.ctx(self.instance_body(n)?).root_nodes.first().copied().flatten()?;
                continue;
            }

            return self.builtin_name(n).is_some_and(is_url_object).then_some(n);
        }

        None
    }
}

/// X3DConstants NOT_STARTED_STATE … FAILED_STATE.
pub fn url_state_value(state: nodes::LoadState) -> i32 {
    match state {
        nodes::LoadState::NotStarted => 0,
        nodes::LoadState::InProgress => 1,
        nodes::LoadState::Complete => 2,
        nodes::LoadState::Failed => 3,
    }
}
