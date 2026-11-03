//! MovieTexture with an MPEG-1 movie (issue #90, ISO/IEC 19775-1 18.4.2): the native load path decodes `.mpg`, `.mpeg`,
//! `.mpe` and `.m1v` urls; the compatibility option [`Runtime::mpeg1_unplayable`] keeps the behavior of the pinned
//! X_ITE 16.5.0 (a `<video>` element that Chrome cannot play MPEG-1 in: the file is an unplayable video and fails with
//! the usual diagnostic). The decoder itself is tested in `mpeg1_decode.rs`; the fixtures are the ones described there.

use std::collections::HashMap;

use x3d_fields::NodeId;
use x3d_runtime::{Fetcher, Runtime, nodes::LoadState};

struct Files(HashMap<String, Vec<u8>>);

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        self.0
            .get(url.path())
            .cloned()
            .ok_or_else(|| "Failed to fetch".to_string())
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/data/mpeg1/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// A world with one MovieTexture `M` (url list `urls`) served from `files`.
fn world(urls: &str, files: &[(&str, Vec<u8>)], unplayable: bool) -> Runtime {
    let text = format!(
        "#VRML V2.0 utf8\nShape {{ appearance Appearance {{ texture DEF M MovieTexture {{ url [{urls}] loop TRUE }} }} geometry Box {{}} }}"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.wrl").unwrap();
    let files = files
        .iter()
        .map(|(n, d)| (format!("/{n}"), d.clone()))
        .collect();
    let mut rt = Runtime::new(scene, Box::new(Files(files)));

    rt.mpeg1_unplayable = unplayable;
    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    rt
}

fn movie(rt: &Runtime) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, "M").unwrap()
}

fn failed_with_the_usual_diagnostic(rt: &Runtime, file: &str) {
    assert_eq!(rt.url_load_state(movie(rt)), LoadState::Failed);
    assert!(!rt.movie_has_frames(movie(rt)));
    assert!(
        rt.log.iter().any(|l| l.contains(&format!(
            "Couldn't load video 'http://oracle/{file}': error."
        ))),
        "{:?}",
        rt.log
    );
}

#[test]
fn the_compatibility_option_is_off_by_default() {
    let scene = x3d_parse::parse_bytes(b"#VRML V2.0 utf8\n", "http://oracle/t.wrl").unwrap();

    assert!(!Runtime::new(scene, Box::new(Files(HashMap::new()))).mpeg1_unplayable);
}

#[test]
fn an_mpeg1_systems_movie_loads_as_frames() {
    let rt = world(
        "\"clip.mpg\"",
        &[("clip.mpg", fixture("sys-av-64x48.mpg"))],
        false,
    );
    let (w, h, rgba, shown) = rt.movie_texture_frame(movie(&rt)).unwrap();

    assert_eq!(rt.url_load_state(movie(&rt)), LoadState::Complete);
    assert_eq!((w, h, rgba.len(), shown), (64, 48, 64 * 48 * 4, 0));
    assert!(rt.log.iter().all(|l| !l.contains("MPEG-1")), "{:?}", rt.log);
}

#[test]
fn a_video_only_movie_loads_by_each_mpeg1_extension() {
    for name in ["a.m1v", "a.MPEG", "dir/a.mpe", "a.mpg"] {
        let file = name.to_string();
        let rt = world(
            &format!("\"{file}\""),
            &[(&file, fixture("intra-64x48.m1v"))],
            false,
        );

        assert_eq!(rt.url_load_state(movie(&rt)), LoadState::Complete, "{name}");
        assert_eq!(
            rt.movie_texture_frame(movie(&rt)).map(|f| (f.0, f.1)),
            Some((64, 48))
        );
    }
}

#[test]
fn the_compatibility_option_treats_mpeg1_as_unplayable_like_the_pinned_page() {
    let rt = world(
        "\"clip.mpg\"",
        &[("clip.mpg", fixture("sys-av-64x48.mpg"))],
        true,
    );

    failed_with_the_usual_diagnostic(&rt, "clip.mpg");
    assert!(rt.log.iter().all(|l| !l.contains("MPEG-1 movie")));
}

#[test]
fn undecodable_mpeg1_data_fails_with_the_usual_diagnostic_and_the_reason() {
    for (data, reason) in [
        (vec![0x55; 3000], "not an MPEG-1 stream"),
        (
            vec![0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 1, 0x89, 0xC3, 0xF8],
            "MPEG-2 streams are not supported",
        ),
        (
            fixture("ipb-64x48.m1v")[..9].to_vec(),
            "truncated MPEG-1 data",
        ),
    ] {
        let rt = world("\"bad.mpg\"", &[("bad.mpg", data)], false);

        failed_with_the_usual_diagnostic(&rt, "bad.mpg");
        assert!(
            rt.log
                .iter()
                .any(|l| l.contains("MPEG-1 movie 'http://oracle/bad.mpg'") && l.contains(reason)),
            "{reason}: {:?}",
            rt.log
        );
    }
}

#[test]
fn the_url_list_falls_through_a_bad_mpeg1_file_to_the_next_candidate() {
    let rt = world(
        "\"bad.mpg\" \"good.m1v\"",
        &[
            ("bad.mpg", vec![1, 2, 3]),
            ("good.m1v", fixture("ip-64x48.m1v")),
        ],
        false,
    );

    assert_eq!(rt.url_load_state(movie(&rt)), LoadState::Complete);
    assert!(rt.movie_has_frames(movie(&rt)));
}

#[test]
fn a_missing_mpeg1_file_fails_and_the_scene_keeps_running() {
    let mut rt = world("\"nothing.mpg\"", &[], false);

    failed_with_the_usual_diagnostic(&rt, "nothing.mpg");

    for _ in 0..5 {
        rt.run_tasks();
    }

    assert_eq!(rt.url_load_state(movie(&rt)), LoadState::Failed);
}
