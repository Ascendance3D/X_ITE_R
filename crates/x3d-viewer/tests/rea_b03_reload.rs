//! REA batch 03: the pick sensors' Ammo state through Inline unload / reload and world reloads. The
//! sensors of vpick-inline-sensor.wrl live in an Inline that a KeySensor unloads ('s') and loads
//! again ('r'). Each load creates new nodes; the state Viewer3d keeps for them (PickShapes: the cone
//! and mesh shapes per geometry node and the two compound AABBs per sensor) must go with the nodes,
//! so the state stays bounded however often the Inline or the world is loaded, and the sensors
//! still report contacts after each reload. Stepping only (like x3d-drive), no GPU.

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

    /// The `IP_a_count` of the Script (isActive events of the PointPickSensor inside the Inline).
    fn active_count(&self) -> i32 {
        let rt = &self.v.rt;
        let r = rt.scene.get_named_node(0, "R").expect("R");
        match rt.get_field(r, "IP_a_count").map(|k| rt.value(k)) {
            Some(x3d_fields::FieldValue::SFInt32(n)) => *n,
            other => panic!("IP_a_count {other:?}"),
        }
    }
}

#[test]
fn rea_b03_pick_state_is_released_on_reload() {
    spawn_big_stack(|| {
        let oracle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
        let world = oracle.join("synthetic-rea-b03/vpick-inline.wrl");
        let mut d = Driver::load(&world, &oracle);
        d.frames(130);

        let first = d.v.pick_shapes.len();
        assert!(first > 0, "the pick sensors keep no state");
        assert!(d.active_count() > 0, "the PointPickSensor never became active");
        let mut sizes = vec![];

        for cycle in 0..30 {
            d.key("s");
            d.frames(5);
            d.key("r");
            d.frames(130);
            let before = d.active_count();
            d.frames(130);
            assert!(d.active_count() > before, "cycle {cycle}: no pick events after the reload");
            sizes.push(d.v.pick_shapes.len());
        }

        for _ in 0..5 {
            d = Driver::load(&world, &oracle);
            d.frames(130);
            sizes.push(d.v.pick_shapes.len());
        }

        eprintln!("pick state: first {first}, after reloads {sizes:?}");
        let max = *sizes.iter().max().unwrap();
        assert!(max <= first, "pick state grows with reloads: first {first}, then {sizes:?}");
    });
}
