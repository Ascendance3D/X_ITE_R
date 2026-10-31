//! DepthMode in the interactive viewer's pointing pass (issue #79): the shape under the pointer is the one the pointing
//! buffer ends with, and X3DRenderObject.pointing enables the appearance's DepthMode around each shape's draw. Two boxes
//! face the camera, NEAR in front of FAR; FAR is drawn second. Without a DepthMode NEAR wins; the modes below move the
//! win to FAR (or hide NEAR) exactly as GL's depth state does for the draw.
//!
//! The embedded viewer picks with the pointing buffer on its renderer's device. Skipped when no adapter exists.

mod common;

use common::gpu::device;
use std::path::{Path, PathBuf};
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;
const W: u32 = 96;
const H: u32 = 72;

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-depthmode-pick-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// NEAR at z = 0 and FAR at z = -1, FAR second in drawing order, each with the given DepthMode field text.
fn world(d: &Path, name: &str, near: &str, far: &str) -> PathBuf {
    let p = d.join(format!("{name}.x3dv"));

    std::fs::write(
        &p,
        format!(
            "#X3D V4.0 utf8\nPROFILE Immersive\nCOMPONENT X_ITE:1\nViewpoint {{ position 0 0 3 }}\n\
             Transform {{ children DEF NEAR Shape {{ appearance Appearance {{ {near} }} geometry Box {{ size 1 1 0.1 }} }} }}\n\
             Transform {{ translation 0 0 -1 children DEF FAR Shape {{ appearance Appearance {{ {far} }} geometry Box {{ size 1 1 0.1 }} }} }}\n"
        ),
    )
    .unwrap();
    p
}

fn mode(fields: &str) -> String {
    format!("depthMode DepthMode {{ {fields} }}")
}

/// The DEF name of the shape under the center of the canvas, or "" for no hit.
fn center_hit(v: &mut EmbeddedViewer) -> String {
    let core = v.core().unwrap();
    let h = core.probe_pick(W as f64 / 2.0 + 0.5, H as f64 / 2.0 + 0.5);

    match (&h.shape, h.id) {
        (Some(s), true) => x3d_viewer::walk::rt_at(&core.rt, &s.path)
            .map(|rt| rt.scene.nodes[s.node as usize].name.clone())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

#[test]
fn depth_mode_decides_the_picked_shape() {
    let Some((d, q)) = device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let dir = dir();
        let cases = [
            ("default", String::new(), String::new(), "NEAR"),
            ("default DepthMode", mode(""), mode(""), "NEAR"),
            (
                "far depthTest FALSE",
                String::new(),
                mode("depthTest FALSE"),
                "FAR",
            ),
            (
                "near depthMask FALSE",
                mode("depthMask FALSE"),
                String::new(),
                "FAR",
            ),
            (
                "near depthFunction NEVER",
                mode("depthFunction \"NEVER\""),
                String::new(),
                "FAR",
            ),
            (
                "far depthFunction ALWAYS",
                String::new(),
                mode("depthFunction \"ALWAYS\""),
                "FAR",
            ),
            (
                "far depthFunction GREATER",
                String::new(),
                mode("depthFunction \"GREATER\""),
                "FAR",
            ),
            (
                "far depthFunction LESS",
                String::new(),
                mode("depthFunction \"LESS\""),
                "NEAR",
            ),
            (
                "far depthRange 0 0",
                String::new(),
                mode("depthRange 0 0"),
                "FAR",
            ),
            (
                "near depthRange 1 1",
                mode("depthRange 1 1"),
                String::new(),
                "FAR",
            ),
        ];

        for (k, (name, near, far, expect)) in cases.iter().enumerate() {
            let path = world(&dir, &format!("w{k}"), near, far);
            let mut v = EmbeddedViewer::new(d.clone(), q.clone(), (W, H));

            v.load(
                &path,
                LoadOptions {
                    root: None,
                    start_time: Some(T0),
                },
            )
            .unwrap();

            for s in 1..4 {
                v.step(T0 + s as f64 / 30.0);
            }

            assert_eq!(center_hit(&mut v), *expect, "{name}");
        }
    });
}
