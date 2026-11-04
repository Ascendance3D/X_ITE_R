//! The choice of the pinned recording of a case (`common::reference`): the NVIDIA recording by default, a platform recording
//! only when the platform is named and the case has one. No GPU needed.

mod common;

use common::reference::{js_json_for, pair_dir_for};
use std::path::Path;

fn make(root: &Path, dir: &str) {
    let d = root.join(dir);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("js.json"), "{}").unwrap();
}

#[test]
fn a_platform_recording_is_used_only_when_the_platform_is_named_and_the_case_has_one() {
    let root = std::env::temp_dir().join(format!("reference-platform-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    make(&root, "oracle/base/a-msaa0");
    make(&root, "oracle/base/b-msaa0");
    std::fs::create_dir_all(root.join("oracle/base-apple-m1-baseline")).unwrap();
    std::fs::write(
        root.join("oracle/base-apple-m1-baseline/b-msaa0.json"),
        "{}",
    )
    .unwrap();

    let nvidia = |n: &str| root.join(format!("oracle/base/{n}-msaa0/js.json"));
    let m1 = |n: &str| root.join(format!("oracle/base-apple-m1-baseline/{n}-msaa0.json"));

    // No platform: always the NVIDIA recording, also for a case that has a platform recording.
    assert_eq!(js_json_for(&root, "base", "b", 0, None), nvidia("b"));
    // The platform is named: the platform recording where it exists, the NVIDIA recording for the other cases.
    assert_eq!(
        js_json_for(&root, "base", "b", 0, Some("apple-m1")),
        m1("b")
    );
    assert_eq!(
        js_json_for(&root, "base", "a", 0, Some("apple-m1")),
        nvidia("a")
    );
    // Another MSAA level has its own directory name: no platform recording there.
    assert_eq!(
        js_json_for(&root, "base", "b", 4, Some("apple-m1")),
        root.join("oracle/base/b-msaa4/js.json")
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
#[should_panic(expected = "unknown platform")]
fn an_unknown_platform_stops_the_test() {
    // X3D_REFERENCE_PLATFORM is read by `platform()`; set it only in this process, in a test of its own.
    // SAFETY: this test binary has no other thread that reads the environment at the same time.
    unsafe { std::env::set_var("X3D_REFERENCE_PLATFORM", "no-such-platform") };
    let _ = common::reference::platform();
}

#[test]
fn a_particle_case_uses_the_flat_platform_files_only_when_both_are_there() {
    let root = std::env::temp_dir().join(format!("reference-pair-{}", std::process::id()));
    let flat = root.join("oracle/pb-apple-m1-baseline");
    std::fs::create_dir_all(&flat).unwrap();
    make(&root, "oracle/pb/c-x");
    std::fs::write(root.join("oracle/pb/c-x/js.f32"), "").unwrap();
    std::fs::write(flat.join("c-x.json"), "{\"p\":1}").unwrap(); // json only
    std::fs::write(flat.join("c-y.json"), "{\"p\":2}").unwrap();
    std::fs::write(flat.join("c-y.f32"), [1u8, 2, 3, 4]).unwrap();

    // No platform: the NVIDIA directory.
    assert_eq!(
        pair_dir_for(&root, "pb", "c-y", None),
        root.join("oracle/pb/c-y")
    );
    // js.json alone is not used (the f32 records would come from another recording).
    assert_eq!(
        pair_dir_for(&root, "pb", "c-x", Some("apple-m1")),
        root.join("oracle/pb/c-x")
    );
    // Both files: copied unchanged as js.json and js.f32.
    let d = pair_dir_for(&root, "pb", "c-y", Some("apple-m1"));
    assert_eq!(std::fs::read(d.join("js.json")).unwrap(), b"{\"p\":2}");
    assert_eq!(std::fs::read(d.join("js.f32")).unwrap(), [1u8, 2, 3, 4]);

    std::fs::remove_dir_all(&d).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}
