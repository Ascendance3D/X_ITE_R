//! REA batch 09 cloud: Browser members whose effect is the browser's, not the scene's (owner decision 2026-10-20: real
//! effects). The X_ITE oracle cannot observe them (its page owns the canvas): Browser.bindViewpoint binds the viewpoint,
//! getActiveViewpoint then reports it, Browser.resize resizes the browser, Browser.dispose stops it. No GPU is needed.

use x3d_fields::FieldValue;
use x3d_viewer::viewer::Viewer3d;

const WORLD: &str = r#"#VRML V2.0 utf8
DEF V1 Viewpoint { description "one" }
DEF V2 Viewpoint { description "two" }
DEF S Script {
  eventIn SFBool bind
  eventIn SFBool check
  eventIn SFBool size
  eventIn SFBool stop
  directOutput TRUE
  url "javascript:
    function bind (v) { Browser .bindViewpoint (Browser .getActiveLayer (), Browser .currentScene .getNamedNode ('V2')); }
    function check (v) { print ('active ' + Browser .getActiveViewpoint () .getName () + ' ' + (Browser .activeViewpoint !== null)); }
    function size (v) { Browser .resize (320, 200); }
    function stop (v) { Browser .dispose (); }
  "
}
"#;

fn send(v: &mut Viewer3d, name: &str) {
    let s = v.rt.scene.get_named_node(x3d_scene::ROOT, "S").unwrap();
    let k = v.rt.get_field(s, name).unwrap();

    v.rt.set_value(k, FieldValue::SFBool(true));
}

#[test]
fn bind_viewpoint_resize_and_dispose_reach_the_browser() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("browser_requests_b09");
    std::fs::create_dir_all(&dir).unwrap();
    let world = dir.join("world.wrl");
    std::fs::write(&world, WORLD).unwrap();

    let t0 = 946684800.0;
    let mut v = Viewer3d::load_opts(&world, None, t0, 64, 64, None, true).expect("viewer load");
    let mut t = t0;
    let mut steps = |v: &mut Viewer3d, n: usize| {
        for _ in 0..n {
            t += 1.0 / 60.0;
            v.step(t);
        }
    };

    steps(&mut v, 3);
    assert_eq!(v.active_viewpoint_name(), "V1");

    send(&mut v, "bind");
    steps(&mut v, 2);
    assert_eq!(v.active_viewpoint_name(), "V2", "{:?}", v.rt.log);

    send(&mut v, "check");
    steps(&mut v, 1);
    // The viewer shows prints in its status line; the recorded console keeps them.
    assert_eq!(v.rt.console.last().map(String::as_str), Some("log: active V2 true"), "{:?}", v.rt.log);

    send(&mut v, "size");
    steps(&mut v, 1);
    assert_eq!((v.width_px(), v.height_px()), (320, 200));
    assert_eq!(v.resize_request, Some((320, 200)));

    send(&mut v, "stop");
    steps(&mut v, 1);
    assert!(v.disposed);

    let now = v.rt.now;
    steps(&mut v, 3);
    assert_eq!(v.rt.now, now, "a disposed browser does not advance");
}
