//! REA batch 04: the runtime state of the batch 04 nodes through Inline unload / reload and world
//! reloads. reload-inline.wrl loads reload-inline-content.wrl (a NurbsSet with a member surface, a
//! NurbsPositionInterpolator, a ViewpointGroup with its private ProximitySensor, an HAnimHumanoid
//! with a running HAnimMotion and an HAnimPose) in an Inline that a KeySensor unloads ('s') and
//! loads again ('r'). Each load creates new nodes; the maps the runtime keeps for them
//! (nurbs_scales, nurbs_set_members, viewpoint_groups, hanim_humanoids / poses / motions) must go
//! with the nodes, so their size stays bounded however often the Inline is loaded, and the motion
//! still turns the imported joint after each reload. Stepping only (like x3d-drive), no GPU.

use std::path::{Path, PathBuf};
use x3d_viewer::embed::spawn_big_stack;
use x3d_viewer::viewer::{UiEvent, Viewer3d, ORACLE_PAGE_ATTRIBUTES};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;

struct Driver {
    v: Viewer3d,
    now: f64,
}

impl Driver {
    fn load(world: &Path, oracle: &Path) -> Driver {
        let mut v = Viewer3d::load_attributes(world, Some(oracle), T0, 64, 64, None, false, ORACLE_PAGE_ATTRIBUTES).unwrap();
        v.frame_rate = 1.0 / DT;

        for _ in 0..8 {
            v.step(T0);
        }

        Driver { v, now: T0 }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.now += DT;
            self.v.date_now = self.now;
            self.v.step(self.now);
        }
    }

    fn key(&mut self, key: &str) {
        self.v.input(UiEvent::KeyDown { key: key.into() });
        self.v.input(UiEvent::KeyUp { key: key.into() });
        self.frames(3);
    }

    fn rotations(&self) -> i32 {
        let rt = &self.v.rt;
        let r = rt.scene.get_named_node(0, "R").expect("R");
        match rt.get_field(r, "rotations").map(|k| rt.value(k)) {
            Some(x3d_fields::FieldValue::SFInt32(n)) => *n,
            other => panic!("rotations {other:?}"),
        }
    }

    /// Entries of the batch 04 runtime maps.
    fn state(&self) -> [usize; 6] {
        let rt = &self.v.rt;
        [rt.nurbs_scales.len(), rt.nurbs_set_members.len(), rt.viewpoint_groups.len(), rt.hanim_humanoids.len(), rt.hanim_poses.len(), rt.hanim_motions.len()]
    }

    /// Live scene nodes (the private TimeSensors, interpolators and ProximitySensor included).
    fn live_nodes(&self) -> usize {
        let s = &self.v.rt.scene;
        s.nodes.len() - s.free_nodes.len()
    }
}

#[test]
fn rea_b04_runtime_state_is_released_on_reload() {
    spawn_big_stack(|| {
        let oracle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
        let world = oracle.join("synthetic-rea-b04/reload-inline.wrl");
        let mut d = Driver::load(&world, &oracle);
        d.frames(40);

        let first = d.state();
        assert!(first.iter().all(|n| *n > 0), "a batch 04 node keeps no state: {first:?}");
        assert!(d.rotations() > 0, "the HAnimMotion never turned the joint");
        let mut states = vec![];
        let mut live = vec![];

        for cycle in 0..30 {
            d.key("s");
            d.frames(5);
            let unloaded = d.state();
            assert!(unloaded.iter().all(|n| *n == 0), "cycle {cycle}: state kept after the unload: {unloaded:?}");
            d.key("r");
            d.frames(10);
            let before = d.rotations();
            d.frames(20);
            assert!(d.rotations() > before, "cycle {cycle}: the motion stopped after the reload");
            states.push(d.state());
            live.push(d.live_nodes());
        }

        // Every cycle loads the same content: the live node count settles and does not grow.
        assert!(live[29] <= live[1], "live nodes grow with Inline reloads: {live:?}");

        for _ in 0..5 {
            d = Driver::load(&world, &oracle);
            d.frames(40);
            states.push(d.state());
        }

        eprintln!("batch 04 state: first {first:?}, after reloads {states:?}, live nodes {live:?}");

        for s in &states {
            assert!(s.iter().zip(first.iter()).all(|(a, b)| a <= b), "state grows with reloads: first {first:?}, then {states:?}");
        }
    });
}
