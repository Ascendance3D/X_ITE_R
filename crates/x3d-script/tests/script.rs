//! Script field marshalling on QuickJS, checked directly (the synthetic-x6 oracle checks the
//! same behavior against the pinned X_ITE in Chromium).

use x3d_fields::FieldValue;
use x3d_runtime::{Fetcher, Runtime};
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

    rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
    rt.load_world();
    rt
}

fn named(rt: &Runtime, name: &str) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, name).unwrap()
}

fn field(rt: &Runtime, node: &str, field: &str) -> FieldValue {
    let n = named(rt, node);
    let loc = rt.scene.get_field(n, field).unwrap();
    rt.value((n, loc)).clone()
}

#[test]
fn sfint32_truncates_and_sfbool_coerces() {
    let rt = run(r#"DEF S Script { field SFInt32 i 0 field SFInt32 j 0 field SFBool b TRUE field SFString s ""
        url "javascript: function initialize () { i = 2.9; j = -2.9; b = 0; s = 42; }" }"#);

    assert_fv(field(&rt, "S", "i"), FieldValue::SFInt32(2));
    assert_fv(field(&rt, "S", "j"), FieldValue::SFInt32(-2));
    assert_fv(field(&rt, "S", "b"), FieldValue::SFBool(false));
    assert_fv(field(&rt, "S", "s"), FieldValue::SFString("42".into()));
}

#[test]
fn mf_fields_grow_and_shrink_through_index_and_length() {
    let rt = run(r#"DEF S Script { field MFInt32 a [ 1 2 3 ] field MFVec3f v [ 1 0 0 ]
        url "javascript: function initialize () { a [5] = 9; a .length = 4; v [1] = new SFVec3f (0, 2, 0); v [0] .z = 7; }" }"#);

    assert_fv(field(&rt, "S", "a"), FieldValue::MFInt32(vec![1, 2, 3, 0]));
    assert_fv(field(&rt, "S", "v"), FieldValue::MFVec3f(vec![1.0, 0.0, 7.0, 0.0, 2.0, 0.0]));
}

/// A length or index near 2^32 must not allocate: it is a RangeError and the field keeps its value.
/// (Before, `a .length = -1` grew the array until the kernel killed the process.) The pinned X_ITE allocates the typed array
/// for 2^32 - 1 and the page runs out of memory: the RangeError here is a kept safety difference. A fractional length is not
/// an error in X_ITE (X3DTypedArrayField.resize stores it); oracle case fldx-edge compares that with X_ITE.
#[test]
fn huge_mf_length_or_index_is_a_range_error_not_an_allocation() {
    let rt = run(r#"DEF S Script { field MFInt32 a [ 1 2 3 ] field MFVec3f v [ ] field SFString r ""
        url "javascript:
          function initialize () {
            var o = [];
            try { a .length = -1; } catch (e) { o .push (e .name); }
            try { a .length = 4294967295; } catch (e) { o .push (e .name); }
            try { a [4294967295] = 1; } catch (e) { o .push (e .name); }
            try { v [1e12] = new SFVec3f (1, 2, 3); } catch (e) { o .push (e .name); }
            r = o .join ();
          }" }"#);

    assert_fv(field(&rt, "S", "r"), FieldValue::SFString("RangeError,RangeError,RangeError,RangeError".into()));
    assert_fv(field(&rt, "S", "a"), FieldValue::MFInt32(vec![1, 2, 3]));
    assert_fv(field(&rt, "S", "v"), FieldValue::MFVec3f(vec![]));
}

#[test]
fn node_fields_are_live_and_events_reach_routes() {
    let rt = run(r#"DEF A Transform { } DEF B Transform { }
        ROUTE A.translation_changed TO B.set_translation
        DEF S Script { directOutput TRUE field SFNode a USE A
        url "javascript: function initialize () { a .translation = new SFVec3f (1, 2, 3); }" }"#);

    let expected = FieldValue::SFVec3f(x3d_fields::math::Vector3::new(1.0, 2.0, 3.0));

    assert_fv(field(&rt, "A", "translation"), expected.clone());
    assert_fv(field(&rt, "B", "translation"), expected);
}

#[test]
fn script_errors_use_x_ite_wording() {
    let rt = run(r#"DEF S Script { field SFNode n NULL url "javascript: function initialize () { n .translation; }" }"#);

    assert!(rt.log.iter().any(|l| l == "error: JavaScript Error in Script 'S', in function 'initialize'"), "{:?}", rt.log);
    assert!(rt.log.iter().any(|l| l == "error: TypeError: Cannot read properties of null (reading 'translation')"), "{:?}", rt.log);
}

#[test]
fn browser_time_is_frozen() {
    let rt = run(r#"DEF S Script { field SFTime t 0 url "javascript: function initialize () { t = Browser .getCurrentTime (); }" }"#);

    assert_fv(field(&rt, "S", "t"), FieldValue::SFTime(946684800.0));
}

#[track_caller]
fn assert_fv(actual: FieldValue, expected: FieldValue) {
    assert!(actual.equals(&expected), "{actual:?} != {expected:?}");
}
