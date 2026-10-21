//! Event engine unit tests (no Script engine): router loops, TimeSensor initialize in a scene
//! that never goes live, X3DInterpolatorNode setup. Expected values follow the pinned X_ITE;
//! oracle/synthetic-x6 checks the same scenes against it in Chromium.

use x3d_fields::FieldValue;
use x3d_fields::math::Vector3;
use x3d_runtime::{FROZEN_TIME, Fetcher, Runtime};
use x3d_fields::NodeId;

struct NoFetch;

impl Fetcher for NoFetch {
    fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
        Err("TypeError: Failed to fetch".into())
    }
}

fn run(body: &str) -> Runtime {
    let text = format!("#VRML V2.0 utf8\n{body}");
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.wrl").unwrap();
    let mut rt = Runtime::new(scene, Box::new(NoFetch));

    rt.load_world();
    rt
}

fn node(rt: &Runtime, name: &str) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, name).unwrap()
}

fn field(rt: &Runtime, name: &str, field: &str) -> FieldValue {
    let n = node(rt, name);
    let loc = rt.scene.get_field(n, field).unwrap();
    rt.value((n, loc)).clone()
}

fn vec3(x: f64, y: f64, z: f64) -> FieldValue {
    FieldValue::SFVec3f(Vector3::new(x, y, z))
}

#[test]
fn time_sensor_starts_at_load_and_sends_first_frame_events() {
    let rt = run("DEF T TimeSensor { cycleInterval 2.5 loop TRUE } DEF Off TimeSensor { loop TRUE enabled FALSE }");

    assert_fv(field(&rt, "T", "isActive"), FieldValue::SFBool(true));
    assert_fv(field(&rt, "T", "cycleTime"), FieldValue::SFTime(FROZEN_TIME));
    assert_fv(field(&rt, "T", "time"), FieldValue::SFTime(FROZEN_TIME));
    // X3DTimeDependentNode: fraction = fract ((now - startTime) / cycleInterval).
    assert_fv(field(&rt, "T", "fraction_changed"), FieldValue::SFFloat((FROZEN_TIME / 2.5) % 1.0));
    assert_fv(field(&rt, "Off", "isActive"), FieldValue::SFBool(false));
}

#[test]
fn interpolator_setup_resizes_key_value_and_routes_first_fraction() {
    let rt = run(
        "DEF T TimeSensor { cycleInterval 4 loop TRUE }
         DEF Short PositionInterpolator { key [ 0 0.5 1 ] keyValue [ 1 2 3 ] }
         DEF P PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0, 4 8 12 ] }
         DEF X Transform { }
         ROUTE T.fraction_changed TO P.set_fraction
         ROUTE P.value_changed TO X.set_translation",
    );

    // set_key__ → resize (keyValue, key .length, last value): a short keyValue is padded
    // with copies of its last element, and the padding is printed by toVRMLString.
    assert_fv(field(&rt, "Short", "keyValue"), FieldValue::MFVec3f(vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]));
    // FROZEN_TIME / 4 is a whole number: fraction 0.
    assert_fv(field(&rt, "X", "translation"), vec3(0.0, 0.0, 0.0));
}

#[test]
fn route_loop_is_broken_once_per_field() {
    let rt = run(
        "DEF T TimeSensor { cycleInterval 3 loop TRUE }
         DEF P PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0, 3 3 3 ] }
         DEF A Transform { } DEF B Transform { }
         ROUTE T.fraction_changed TO P.set_fraction
         ROUTE P.value_changed TO A.set_translation
         ROUTE A.translation_changed TO B.set_translation
         ROUTE B.translation_changed TO A.set_translation",
    );

    let f = (FROZEN_TIME / 3.0) % 1.0;
    let expected = FieldValue::SFVec3f(Vector3::new(3.0 * f, 3.0 * f, 3.0 * f));

    assert_fv(field(&rt, "A", "translation"), expected.clone());
    assert_fv(field(&rt, "B", "translation"), expected);
}

#[test]
fn initialized_volume_types_capture_voxels_once() {
    for (world, url) in [
        (include_str!("../../../oracle/synthetic-rea-b07-vol/stepped/rt-iso.wrl"), "rt-iso.wrl"),
        (include_str!("../../../oracle/synthetic-rea-b07-vol/stepped/rt-segmented.wrl"), "rt-segmented.wrl"),
    ] {
        let scene = x3d_parse::parse_bytes(world.as_bytes(), &format!("http://oracle/{url}")).unwrap();
        let mut rt = Runtime::new(scene, Box::new(NoFetch));
        rt.load_world();
        let volume = node(&rt, "VD");
        let initial = node(&rt, "T_LUM");

        assert!(matches!(rt.nrt(volume).map(|n| &n.state), Some(x3d_runtime::nodes::State::Volume { voxels: Some(v) }) if *v == initial), "{url}");

        let field = rt.scene.get_field(volume, "voxels").unwrap();
        rt.scene.field_mut(volume, field).value = FieldValue::SFNode(None);

        assert!(matches!(rt.value((volume, field)), FieldValue::SFNode(None)), "{url}");
        assert!(matches!(rt.nrt(volume).map(|n| &n.state), Some(x3d_runtime::nodes::State::Volume { voxels: Some(v) }) if *v == initial), "{url}");
    }
}

#[test]
fn missing_inline_reports_like_file_loader() {
    let rt = run(r#"Inline { url [ "a.wrl" "http://dead.example/b.wrl" ] }"#);

    assert_eq!(
        rt.log,
        vec![
            "warn: Inline: Couldn't load URL 'http://oracle/a.wrl', trying URL 2 of 2. TypeError: Failed to fetch".to_string(),
            "error: Inline: Couldn't load any of the 2 URLs, tried in this order:".to_string(),
        ]
    );
}

#[track_caller]
fn assert_fv(actual: FieldValue, expected: FieldValue) {
    assert!(actual.equals(&expected), "{actual:?} != {expected:?}");
}
