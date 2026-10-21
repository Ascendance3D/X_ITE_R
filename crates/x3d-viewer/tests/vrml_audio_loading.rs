//! VRML97 AudioClip loading and Sound.source resolution: the load states, repeated loads, and the
//! scene changes that the X_ITE oracle cases (`vrml_audio_oracle`) do not print. The expected values are
//! the ones X_ITE 16.5.0 recorded for the same worlds in oracle/rea-batch05-baseline (the warning
//! text, `duration_changed` 0.25 / 0.27210416666666665 / -1) and X_ITE's states for the same code paths
//! (AudioClip.loadData: COMPLETE_STATE after setArrayBuffer, FAILED_STATE after `dataCallback (null)`).
//! Stepping only: no GPU, no audio device.

use std::path::{Path, PathBuf};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::nodes::LoadState;
use x3d_viewer::embed::spawn_big_stack;
use x3d_viewer::viewer::{Viewer3d, ORACLE_PAGE_ATTRIBUTES};

const T0: f64 = 946684800.0;
const WAV_DURATION: f64 = 0.25;
const MONO_DURATION: f64 = 0.27210416666666665;
const FALLBACK_WARNING: &str = "/bad.mid', trying URL 2 of 2. EncodingError: Unable to decode audio data";

/// A temporary folder with the oracle media (tone48k-stereo.wav, tone22k-mono.wav, bad.mid, bad2.mid) and `world`.
fn world(name: &str, text: &str) -> (PathBuf, PathBuf) {
    let media = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-rea-b05/media");
    let dir = std::env::temp_dir().join(format!("vrml-audio-loading-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    for f in ["tone48k-stereo.wav", "tone22k-mono.wav", "bad.mid", "bad2.mid"] {
        std::fs::copy(media.join(f), dir.join(f)).unwrap();
    }

    let path = dir.join("world.wrl");
    std::fs::write(&path, format!("#VRML V2.0 utf8\nViewpoint {{ position 0 0 0 }}\n{text}")).unwrap();
    (dir, path)
}

fn load(dir: &Path, path: &Path) -> Viewer3d {
    Viewer3d::load_attributes(path, Some(dir), T0, 64, 64, None, true, ORACLE_PAGE_ATTRIBUTES).unwrap()
}

fn steps(v: &mut Viewer3d, n: usize) {
    for _ in 0..n {
        let now = v.rt.now + 1.0 / 60.0;
        v.date_now = now;
        v.step(now);
    }
}

fn node(v: &Viewer3d, name: &str) -> NodeId {
    v.rt.scene.get_named_node(0, name).unwrap_or_else(|| panic!("no node {name}"))
}

fn duration(v: &Viewer3d, clip: NodeId) -> f64 {
    match v.rt.pre_value(clip, "duration_changed") {
        Some(FieldValue::SFTime(t)) => *t,
        other => panic!("duration_changed {other:?}"),
    }
}

fn has_media(v: &Viewer3d, clip: NodeId) -> bool {
    v.rt.sound.get(&clip).is_some_and(|s| s.media.is_some())
}

fn set_url(v: &mut Viewer3d, clip: NodeId, urls: &[&str]) {
    let k = v.rt.pre(clip, "url").unwrap();
    v.rt.set_value(k, FieldValue::MFString(urls.iter().map(|u| u.to_string()).collect()));
}

/// X_ITE's warning for a first candidate `bad.mid` that does not decode.
fn fallback_warning(v: &Viewer3d) -> bool {
    v.rt.log.iter().any(|l| l.starts_with("warn: AudioClip: Couldn't load URL '") && l.ends_with(FALLBACK_WARNING))
}

fn clip_world(name: &str, url: &str) -> (PathBuf, Viewer3d) {
    let (dir, path) = world(name, &format!("DEF S Sound {{ minFront 20 minBack 20 maxFront 40 maxBack 40 location 0 0 -5 source DEF C AudioClip {{ url {url} loop TRUE }} }}\n"));
    let v = load(&dir, &path);
    (dir, v)
}

#[test]
fn vrml_audio_clip_url_fallback_states() {
    spawn_big_stack(|| {
        // First URL fails decoding, the second is valid: COMPLETE, the second file's duration, X_ITE's warning.
        let (_, mut v) = clip_world("fallback", r#"[ "bad.mid" "tone48k-stereo.wav" ]"#);
        steps(&mut v, 2);
        let c = node(&v, "C");
        assert_eq!(v.rt.url_load_state(c), LoadState::Complete);
        assert_eq!(duration(&v, c), WAV_DURATION);
        assert!(has_media(&v, c), "no media element after the fallback");
        assert!(fallback_warning(&v), "no fallback warning in {:?}", v.rt.log);
        assert!(!v.rt.log.iter().any(|l| l.starts_with("error:")), "an error was logged: {:?}", v.rt.log);

        // Every URL fails decoding: FAILED (not IN_PROGRESS), duration_changed -1, no media element.
        let (_, mut v) = clip_world("all-bad", r#"[ "bad.mid" "bad2.mid" ]"#);
        steps(&mut v, 2);
        let c = node(&v, "C");
        assert_eq!(v.rt.url_load_state(c), LoadState::Failed);
        assert_eq!(duration(&v, c), -1.0);
        assert!(!has_media(&v, c));
        assert!(v.rt.log.iter().any(|l| l == "error: AudioClip: Couldn't load any of the 2 URLs, tried in this order:"), "{:?}", v.rt.log);

        // One URL that fails decoding: FAILED, duration_changed -1.
        let (_, mut v) = clip_world("one-bad", r#""bad.mid""#);
        steps(&mut v, 2);
        let c = node(&v, "C");
        assert_eq!(v.rt.url_load_state(c), LoadState::Failed);
        assert_eq!(duration(&v, c), -1.0);

        // The fetch fails before decoding: the next URL.
        let (_, mut v) = clip_world("missing", r#"[ "missing.wav" "tone22k-mono.wav" ]"#);
        steps(&mut v, 2);
        let c = node(&v, "C");
        assert_eq!(v.rt.url_load_state(c), LoadState::Complete);
        assert_eq!(duration(&v, c), MONO_DURATION);

        // The first URL is valid: no warning.
        let (_, mut v) = clip_world("first-ok", r#"[ "tone48k-stereo.wav" "bad.mid" ]"#);
        steps(&mut v, 2);
        let c = node(&v, "C");
        assert_eq!(v.rt.url_load_state(c), LoadState::Complete);
        assert_eq!(duration(&v, c), WAV_DURATION);
        assert!(!v.rt.log.iter().any(|l| l.contains("Couldn't load")), "{:?}", v.rt.log);

        // An empty URL list: unloadData (duration_changed -1), no media element.
        let (_, mut v) = clip_world("empty", "[ ]");
        steps(&mut v, 2);
        let c = node(&v, "C");
        assert_eq!(duration(&v, c), -1.0);
        assert!(!has_media(&v, c));
    });
}

#[test]
fn vrml_audio_clip_repeated_loads_leave_no_stale_audio() {
    spawn_big_stack(|| {
        let (_, mut v) = clip_world("repeat", r#""tone48k-stereo.wav""#);
        steps(&mut v, 2);
        let c = node(&v, "C");
        assert_eq!(v.rt.url_load_state(c), LoadState::Complete);
        assert!(has_media(&v, c));

        // A new url that cannot be decoded: the old element goes (setMediaElement (null)), FAILED, -1.
        set_url(&mut v, c, &["bad.mid"]);
        steps(&mut v, 4);
        assert_eq!(v.rt.url_load_state(c), LoadState::Failed);
        assert_eq!(duration(&v, c), -1.0);
        assert!(!has_media(&v, c), "the old clip is still loaded after a failed load");

        // Back to a valid file through a failing one: COMPLETE with the new file's duration.
        set_url(&mut v, c, &["bad2.mid", "tone22k-mono.wav"]);
        steps(&mut v, 4);
        assert_eq!(v.rt.url_load_state(c), LoadState::Complete);
        assert_eq!(duration(&v, c), MONO_DURATION);
        assert!(has_media(&v, c));
    });
}

/// `clip-url-change`: the url changes while the frames run, with decodes gated like the oracle page
/// (x3d-drive's `defer_audio_decodes`). X_ITE's frame for the result depends on real time, so only the
/// outcome is checked: the warning, then the second file's duration.
#[test]
fn vrml_audio_clip_url_change_falls_back() {
    spawn_big_stack(|| {
        let (_, mut v) = clip_world("url-change", r#""tone48k-stereo.wav""#);
        steps(&mut v, 2);
        let c = node(&v, "C");
        v.rt.defer_audio_decodes = true;
        set_url(&mut v, c, &["bad.mid", "tone22k-mono.wav"]);

        for _ in 0..6 {
            v.rt.flush_audio_decodes();
            steps(&mut v, 1);
        }

        assert_eq!(v.rt.url_load_state(c), LoadState::Complete);
        assert_eq!(duration(&v, c), MONO_DURATION);
        assert!(fallback_warning(&v), "no fallback warning in {:?}", v.rt.log);

        // Gated, every candidate failing: FAILED after the last decode.
        set_url(&mut v, c, &["bad.mid", "bad2.mid"]);

        for _ in 0..6 {
            v.rt.flush_audio_decodes();
            steps(&mut v, 1);
        }

        assert_eq!(v.rt.url_load_state(c), LoadState::Failed);
        assert_eq!(duration(&v, c), -1.0);
        assert!(!has_media(&v, c));
    });
}

const PROTO: &str = r#"PROTO MyClip [ exposedField MFString url "tone48k-stereo.wav" exposedField SFBool loop TRUE ] {
  AudioClip { url IS url loop IS loop }
}
"#;

fn children(v: &Viewer3d, sound: &str) -> Vec<NodeId> {
    let s = node(v, sound);
    v.rt.sound.get(&s).expect("Sound state").child_nodes.clone()
}

fn inner(v: &Viewer3d, name: &str) -> NodeId {
    v.rt.inner_node(node(v, name)).expect("inner node")
}

#[test]
fn vrml_sound_source_resolves_proto_instances() {
    spawn_big_stack(|| {
        let (dir, path) = world(
            "proto",
            &format!(
                "{PROTO}DEF DIRECT Sound {{ source DEF D AudioClip {{ url \"tone48k-stereo.wav\" loop TRUE }} }}\n\
                 DEF WRAPPED Sound {{ source DEF P MyClip {{ }} }}\n\
                 DEF IS_URL Sound {{ source DEF Q MyClip {{ url [ \"bad.mid\" \"tone22k-mono.wav\" ] }} }}\n\
                 DEF INVALID Sound {{ source Group {{ }} }}\n\
                 DEF BOTH Sound {{ source DEF B MyClip {{ }} children DEF CH AudioClip {{ url \"tone22k-mono.wav\" }} }}\n\
                 DEF ONLY_CHILDREN Sound {{ children DEF CH2 AudioClip {{ url \"tone22k-mono.wav\" }} }}\n"
            ),
        );
        let mut v = load(&dir, &path);
        steps(&mut v, 2);

        // Direct AudioClip.
        assert_eq!(children(&v, "DIRECT"), vec![node(&v, "D")]);
        // PROTO-wrapped AudioClip: the instance's inner node (X3DCast).
        let p = inner(&v, "P");
        assert_ne!(p, node(&v, "P"));
        assert_eq!(children(&v, "WRAPPED"), vec![p]);
        assert_eq!(v.rt.url_load_state(p), LoadState::Complete);
        // A url through IS falls back like a direct clip.
        let q = inner(&v, "Q");
        assert_eq!(children(&v, "IS_URL"), vec![q]);
        assert_eq!(v.rt.url_load_state(q), LoadState::Complete);
        assert_eq!(duration(&v, q), MONO_DURATION);
        // Not an X3DSoundSourceNode.
        assert!(children(&v, "INVALID").is_empty());
        // source first, then the sound children.
        assert_eq!(children(&v, "BOTH"), vec![inner(&v, "B"), node(&v, "CH")]);
        assert_eq!(children(&v, "ONLY_CHILDREN"), vec![node(&v, "CH2")]);
        // The wrapped clip's audio source feeds the Sound's gain like a direct one.
        for s in ["DIRECT", "WRAPPED"] {
            let st = v.rt.sound.get(&node(&v, s)).unwrap();
            assert!(st.gain_node.is_some() && st.vrml.is_some(), "{s} has no audio graph");
        }
    });
}

#[test]
fn vrml_sound_source_change_and_removal() {
    spawn_big_stack(|| {
        let (dir, path) = world(
            "source-change",
            &format!(
                "{PROTO}DEF G Group {{ children DEF SND Sound {{ minFront 20 minBack 20 maxFront 40 maxBack 40 location 3 0 0 source DEF A AudioClip {{ url \"tone48k-stereo.wav\" loop TRUE }} }} }}\n\
                 DEF Q MyClip {{ }}\n"
            ),
        );
        let mut v = load(&dir, &path);
        steps(&mut v, 2);
        let snd = node(&v, "SND");
        assert_eq!(children(&v, "SND"), vec![node(&v, "A")]);
        assert!(v.rt.sound_destinations.contains(&snd), "the Sound is not a sound destination");

        // Source change to a PROTO instance while the world runs.
        let k = v.rt.pre(snd, "source").unwrap();
        v.rt.set_value(k, FieldValue::SFNode(Some(node(&v, "Q"))));
        steps(&mut v, 2);
        assert_eq!(children(&v, "SND"), vec![inner(&v, "Q")]);

        // Source NULL: no children, not visible, so not traversed: no sound destination.
        v.rt.set_value(k, FieldValue::SFNode(None));
        steps(&mut v, 3);
        assert!(children(&v, "SND").is_empty());
        assert!(!v.rt.sound_destinations.contains(&snd));

        // Back to the PROTO clip, then the Sound leaves the scene graph: it stops being a destination.
        v.rt.set_value(k, FieldValue::SFNode(Some(node(&v, "Q"))));
        steps(&mut v, 3);
        assert!(v.rt.sound_destinations.contains(&snd));
        let g = node(&v, "G");
        let kc = v.rt.pre(g, "children").unwrap();
        v.rt.set_value(kc, FieldValue::MFNode(vec![]));
        steps(&mut v, 3);
        assert!(!v.rt.sound_destinations.contains(&snd), "a removed Sound is still a sound destination");
    });
}
