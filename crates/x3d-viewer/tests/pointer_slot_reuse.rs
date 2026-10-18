//! R0.3-D-FIX-1 `VIEWER_POINTER_STALE_SLOT_ALIASING`: the pointer forgets the nodes the runtime
//! releases. A drag, a press or a hover that began on a node of an unloaded Inline ends with it; the
//! node of the next scene that takes the same slot starts inactive and is not driven by the old
//! gesture (ISO/IEC 14772-1 4.6.7.4: a sensor is active only from a button press over its geometry).
//! Newly authored worlds in a temp directory; time is virtual.

use std::path::{Path, PathBuf};
use x3d_fields::{FieldValue, NodeId};
use x3d_scene::{CtxId, ROOT};
use x3d_viewer::viewer::{UiEvent, Viewer3d};

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap();
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-pointer-slot-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path, name: &str, text: &str) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

struct World {
    v: Viewer3d,
    t: f64,
}

impl World {
    fn open(d: &Path, main: &str) -> World {
        let p = write(d, "main.wrl", main);
        let mut w = World { v: Viewer3d::load_opts(&p, Some(d), 100.0, 256, 256, Some(60.0), true).expect("load"), t: 100.0 };
        w.steps(4);
        w
    }

    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.t += 1.0 / 60.0;
            self.v.step(self.t);
        }
    }

    fn named(&self, ctx: CtxId, name: &str) -> NodeId {
        self.v.rt.scene.contexts[ctx].named_nodes.get(name).unwrap_or_else(|| panic!("no {name}"))
    }

    fn inline_ctx(&self) -> CtxId {
        self.v.rt.inline_scenes[&self.named(ROOT, "Inl")]
    }

    fn field(&mut self, node: NodeId, name: &str, value: FieldValue) {
        let k = self.v.rt.get_field(node, name).unwrap();
        self.v.rt.set_value(k, value);
    }

    /// Unload the Inline, then load `file` into it: the new scene takes the released slots.
    fn swap(&mut self, file: &str) {
        let inl = self.named(ROOT, "Inl");

        self.field(inl, "load", FieldValue::SFBool(false));
        self.steps(2);
        self.field(inl, "url", FieldValue::MFString(vec![file.into()]));
        self.field(inl, "load", FieldValue::SFBool(true));
        self.steps(3);
    }

    /// A field's value, printed (`FieldValue` has no `PartialEq`).
    fn value(&self, node: NodeId, name: &str) -> String {
        format!("{:?}", self.v.rt.pre_value(node, name).unwrap_or_else(|| panic!("no field {name}")))
    }

    fn bool(&self, node: NodeId, name: &str) -> bool {
        self.v.rt.get_bool(node, name)
    }

    fn input(&mut self, e: UiEvent) {
        self.v.input(e);
    }
}

/// A scene with an Anchor around a drag sensor of type `sensor` and a box under the pointer.
fn anchored(sensor: &str, n: u32) -> String {
    format!("#VRML V2.0 utf8\nDEF A{n} Anchor {{\n  description \"Anchor{n}\"\n  url \"target{n}.wrl\"\n  children [\n    DEF S{n} {sensor} {{ }}\n    Shape {{ geometry Box {{ size 4 4 4 }} }}\n  ]\n}}\n")
}

/// The output field a drag sensor sends while it is dragged.
fn output(sensor: &str) -> &'static str {
    match sensor {
        "PlaneSensor" => "translation_changed",
        _ => "rotation_changed",
    }
}

/// The QA reproduction, inverted. The button goes down over a drag sensor in an Anchor of an
/// Inline's scene. The Inline unloads and loads a scene whose Anchor and sensor take the same
/// slots; then the mouse moves and goes up.
///
/// The drag sensor is the innermost sensor of the box: it replaces the Anchor's own TouchSensor
/// and the world's sensor around the Inline (renderObject.getSensors () .at (-1); REA batch 02
/// fixtures pointer-anchor-sensors and pointer-nested-sensors check this against X_ITE), so
/// neither of those is active or fires.
fn held_drag_across_slot_reuse(sensor: &'static str) {
    big(move || {
        let d = dir(sensor);

        write(&d, "sub_1.wrl", &anchored(sensor, 1));
        write(&d, "sub_2.wrl", &anchored(sensor, 2));

        let mut w = World::open(&d, "#VRML V2.0 utf8\nViewpoint { position 0 0 10 }\nGroup { children [ DEF Outer PlaneSensor { } DEF Inl Inline { url \"sub_1.wrl\" } ] }\n");
        let outer = w.named(ROOT, "Outer");
        let (a1, s1) = (w.named(w.inline_ctx(), "A1"), w.named(w.inline_ctx(), "S1"));

        w.input(UiEvent::MouseMove { x: 128.0, y: 128.0 });
        w.input(UiEvent::MouseDown { button: 0, x: 128.0, y: 128.0 });
        assert!(w.bool(s1, "isActive"), "{sensor}: the press activates S1");
        assert!(!w.bool(outer, "isActive"), "the innermost sensor replaces the world's sensor");

        w.swap("sub_2.wrl");

        let (a2, s2) = (w.named(w.inline_ctx(), "A2"), w.named(w.inline_ctx(), "S2"));

        assert_eq!((a2, s2), (a1, s1), "{sensor}: precondition: the new Anchor and sensor took the old slots");
        assert!(!w.bool(s2, "isActive"));

        let before = w.value(s2, output(sensor));
        let outer_before = w.value(outer, "translation_changed");

        w.input(UiEvent::MouseMove { x: 180.0, y: 140.0 });
        assert!(!w.bool(s2, "isActive"), "{sensor}: S2 was never pressed");
        assert_eq!(w.value(s2, output(sensor)), before, "{sensor}: the old drag drove S2");
        assert!(!w.bool(outer, "isActive"), "the world's sensor was never pressed");
        assert_eq!(w.value(outer, "translation_changed"), outer_before, "the world's sensor got the drag");

        w.input(UiEvent::MouseUp { button: 0, x: 180.0, y: 140.0 });
        w.steps(1);
        assert!(!w.bool(s2, "isActive"), "{sensor}: S2 after the release");
        assert_eq!(w.value(s2, output(sensor)), before, "{sensor}: the release drove S2");
        assert!(!w.bool(outer, "isActive"), "the world's sensor after the release");
        assert!(w.v.anchors.is_empty(), "{sensor}: the old press activated the new Anchor: {:?}", w.v.anchors.iter().map(|a| &a.description).collect::<Vec<_>>());

        // A fresh click on the new scene works as usual.
        w.input(UiEvent::MouseMove { x: 128.0, y: 128.0 });
        w.input(UiEvent::MouseDown { button: 0, x: 128.0, y: 128.0 });
        assert!(w.bool(s2, "isActive"), "{sensor}: a new press activates S2");
        w.input(UiEvent::MouseMove { x: 150.0, y: 110.0 });
        assert_ne!(w.value(s2, output(sensor)), before, "{sensor}: the new drag drives S2");
        w.input(UiEvent::MouseUp { button: 0, x: 150.0, y: 110.0 });
        assert!(!w.bool(s2, "isActive"));

        // The drag sensor replaces the Anchor's TouchSensor: the Anchor does not fire.
        assert!(w.v.anchors.is_empty(), "{sensor}: the drag sensor's click activated the Anchor: {:?}", w.v.anchors.iter().map(|a| &a.description).collect::<Vec<_>>());

        let _ = std::fs::remove_dir_all(&d);
    });
}

#[test]
fn a_held_plane_sensor_does_not_drive_the_node_that_takes_its_slot() {
    held_drag_across_slot_reuse("PlaneSensor");
}

#[test]
fn a_held_cylinder_sensor_does_not_drive_the_node_that_takes_its_slot() {
    held_drag_across_slot_reuse("CylinderSensor");
}

#[test]
fn a_held_sphere_sensor_does_not_drive_the_node_that_takes_its_slot() {
    held_drag_across_slot_reuse("SphereSensor");
}

/// An Anchor alone (its hidden TouchSensor): pressed in the old scene, released over the new one.
#[test]
fn a_pressed_anchor_does_not_fire_the_anchor_that_takes_its_slot() {
    big(|| {
        let d = dir("anchor");
        let scene = |n: u32| format!("#VRML V2.0 utf8\nDEF A{n} Anchor {{ description \"Anchor{n}\" url \"target{n}.wrl\" children [ Shape {{ geometry Box {{ size 4 4 4 }} }} ] }}\n");

        write(&d, "sub_1.wrl", &scene(1));
        write(&d, "sub_2.wrl", &scene(2));

        let mut w = World::open(&d, "#VRML V2.0 utf8\nViewpoint { position 0 0 10 }\nDEF Inl Inline { url \"sub_1.wrl\" }\n");
        let a1 = w.named(w.inline_ctx(), "A1");

        w.input(UiEvent::MouseMove { x: 128.0, y: 128.0 });
        w.input(UiEvent::MouseDown { button: 0, x: 128.0, y: 128.0 });
        w.swap("sub_2.wrl");
        assert_eq!(w.named(w.inline_ctx(), "A2"), a1, "precondition: A2 took A1's slot");

        w.input(UiEvent::MouseMove { x: 128.0, y: 128.0 });
        w.input(UiEvent::MouseUp { button: 0, x: 128.0, y: 128.0 });
        assert!(w.v.anchors.is_empty(), "the old press activated A2: {:?}", w.v.anchors.iter().map(|a| &a.description).collect::<Vec<_>>());

        w.input(UiEvent::MouseDown { button: 0, x: 128.0, y: 128.0 });
        w.input(UiEvent::MouseUp { button: 0, x: 128.0, y: 128.0 });

        let anchors: Vec<&str> = w.v.anchors.iter().map(|a| a.description.as_str()).collect();
        assert_eq!(anchors, ["Anchor2"], "a fresh click activates A2 once");

        let _ = std::fs::remove_dir_all(&d);
    });
}

/// Hover: the pointer rests over a TouchSensor's box while the Inline is swapped. The TouchSensor
/// that takes the slot is a new node: the next motion over it reports `isOver TRUE` for it.
#[test]
fn a_hover_does_not_carry_over_to_the_node_that_takes_its_slot() {
    big(|| {
        let d = dir("hover");
        let scene = |n: u32| format!("#VRML V2.0 utf8\nGroup {{ children [ DEF T{n} TouchSensor {{ description \"Touch{n}\" }} Shape {{ geometry Box {{ size 4 4 4 }} }} ] }}\n");

        write(&d, "sub_1.wrl", &scene(1));
        write(&d, "sub_2.wrl", &scene(2));

        let mut w = World::open(&d, "#VRML V2.0 utf8\nViewpoint { position 0 0 10 }\nDEF Inl Inline { url \"sub_1.wrl\" }\n");
        let t1 = w.named(w.inline_ctx(), "T1");

        w.input(UiEvent::MouseMove { x: 128.0, y: 128.0 });
        assert!(w.bool(t1, "isOver"));

        w.swap("sub_2.wrl");

        let t2 = w.named(w.inline_ctx(), "T2");

        assert_eq!(t2, t1, "precondition: T2 took T1's slot");
        assert!(!w.bool(t2, "isOver"), "a new node starts not over");

        w.input(UiEvent::MouseMove { x: 129.0, y: 128.0 });
        assert!(w.bool(t2, "isOver"), "the pointer is over T2: it must be told so");

        w.input(UiEvent::MouseMove { x: 5.0, y: 5.0 });
        assert!(!w.bool(t2, "isOver"), "and that it left");

        let _ = std::fs::remove_dir_all(&d);
    });
}

/// A drag of the world's own sensor (outside the Inline) survives the Inline's swap: forgetting
/// the released nodes keeps every other sensor's state.
#[test]
fn a_held_world_sensor_survives_an_inline_swap() {
    big(|| {
        let d = dir("world-drag");

        write(&d, "sub_1.wrl", &anchored("PlaneSensor", 1));
        write(&d, "sub_2.wrl", &anchored("PlaneSensor", 2));

        let mut w = World::open(&d, "#VRML V2.0 utf8\nViewpoint { position 0 0 10 }\nGroup { children [ DEF Outer PlaneSensor { } Transform { translation -3 0 0 children Shape { geometry Box { size 2 2 2 } } } ] }\nDEF Inl Inline { url \"sub_1.wrl\" }\n");
        let outer = w.named(ROOT, "Outer");

        w.input(UiEvent::MouseMove { x: 35.0, y: 128.0 });
        w.input(UiEvent::MouseDown { button: 0, x: 35.0, y: 128.0 });
        assert!(w.bool(outer, "isActive"), "the press activates the world's sensor");

        w.swap("sub_2.wrl");

        let before = w.value(outer, "translation_changed");
        w.input(UiEvent::MouseMove { x: 50.0, y: 140.0 });
        assert!(w.bool(outer, "isActive"), "the world's sensor is still being dragged");
        assert_ne!(w.value(outer, "translation_changed"), before, "the world's sensor still gets the drag");

        w.input(UiEvent::MouseUp { button: 0, x: 50.0, y: 140.0 });
        assert!(!w.bool(outer, "isActive"), "the release ends the world's drag");

        let _ = std::fs::remove_dir_all(&d);
    });
}
