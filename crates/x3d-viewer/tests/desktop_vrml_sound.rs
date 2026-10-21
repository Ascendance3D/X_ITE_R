//! VRML97 Sound and AudioClip (Sound.js, AudioClip.js of X_ITE 16.5.0). Found by the native desktop check: a
//! world with `Sound { source AudioClip { url "x.wav" loop TRUE } }` was silent, because neither node was ported.
//! The worlds are written here (a generated 440 Hz WAV); stepping only, no GPU, no audio device.

use std::path::Path;
use x3d_fields::FieldValue;
use x3d_viewer::embed::spawn_big_stack;
use x3d_viewer::viewer::{Viewer3d, ORACLE_PAGE_ATTRIBUTES};

const T0: f64 = 946684800.0;

fn tone(path: &Path) {
    let rate = 48000u32;
    let samples: Vec<i16> = (0..rate * 2).map(|i| (8000.0 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / rate as f64).sin()) as i16).collect();
    let mut b = Vec::new();
    let data = samples.len() as u32 * 2;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// Load a world with the viewer at `viewer_z` and a Sound at (5, 0, 0); returns
/// [stereo, front left, front right, back left, back right] gains, `duration_changed` and `isActive`.
fn run(name: &str, viewer_z: f64, spatialize: bool) -> ([f32; 5], f64, bool) {
    let dir = std::env::temp_dir().join(format!("x3d-desktop-sound-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    tone(&dir.join("tone.wav"));
    let world = dir.join("sound.wrl");
    std::fs::write(
        &world,
        format!(
            "#VRML V2.0 utf8\nViewpoint {{ position 0 0 {viewer_z} }}\nNavigationInfo {{ type \"EXAMINE\" }}\n\
             DEF S Sound {{ location 5 0 0 spatialize {} minFront 20 minBack 20 maxFront 40 maxBack 40 source DEF C AudioClip {{ url \"tone.wav\" loop TRUE }} }}\n\
             Shape {{ geometry Box {{ }} }}\n",
            if spatialize { "TRUE" } else { "FALSE" }
        ),
    )
    .unwrap();

    let mut v = Viewer3d::load_attributes(&world, Some(&dir), T0, 64, 64, None, false, ORACLE_PAGE_ATTRIBUTES).unwrap();
    let mut now = T0;

    for _ in 0..10 {
        now += 1.0 / 30.0;
        v.date_now = now;
        v.step(now);
    }

    let rt = &v.rt;
    let s = rt.scene.get_named_node(0, "S").expect("S");
    let c = rt.scene.get_named_node(0, "C").expect("C");
    let st = rt.sound.get(&s).expect("Sound state");
    let n = st.vrml.as_ref().expect("Sound audio nodes");
    let e = rt.audio.as_ref().expect("audio context");
    let gains = [n.stereo, n.front_left, n.front_right, n.back_left, n.back_right].map(|id| e.param_value(id, "gain"));
    let duration = match rt.pre_value(c, "duration_changed") {
        Some(FieldValue::SFTime(t)) => *t,
        other => panic!("duration_changed {other:?}"),
    };
    let active = matches!(rt.pre_value(c, "isActive"), Some(FieldValue::SFBool(true)));
    assert_eq!(st.child_nodes, vec![c], "the AudioClip is not the Sound's source");
    let _ = std::fs::remove_dir_all(&dir);
    (gains, duration, active)
}

#[test]
fn vrml_sound_audio_clip_plays_and_pans() {
    spawn_big_stack(|| {
        // Inside the min ellipsoid, the source on the right: pan 1, rotation 0 -> left 0, right 1.
        let (g, duration, active) = run("inside", 0.0, true);
        assert!((duration - 2.0).abs() < 1e-9, "duration_changed {duration}");
        assert!(active, "the looping AudioClip is not active");
        assert_eq!(g[0], 1.0);
        assert!(g[1].abs() < 1e-6 && (g[2] - 1.0).abs() < 1e-6 && g[3] == 0.0 && g[4] == 0.0, "pan gains {g:?}");

        // spatialize FALSE: pan 0.5 -> 0.75 on both front gains (the stereo gain carries the sound).
        let (g, ..) = run("flat", 0.0, false);
        assert!((g[1] - 0.75).abs() < 1e-6 && (g[2] - 0.75).abs() < 1e-6, "unspatialized gains {g:?}");

        // Outside the max ellipsoid: silent.
        let (g, ..) = run("outside", 100.0, true);
        assert_eq!(g, [0.0; 5], "gains outside maxFront {g:?}");

        // Between min and max: 0 < gain < 1.
        let (g, ..) = run("between", 30.0, true);
        assert!(g[0] > 0.0 && g[0] < 1.0, "attenuated gain {g:?}");
    });
}
