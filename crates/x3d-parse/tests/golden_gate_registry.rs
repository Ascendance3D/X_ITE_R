//! GoldenGate parser registry against the pinned X_ITE 16.5.0, `src/x_ite/Parser/GoldenGate.js` (oracle/x_ite-pin).
//! Line numbers below are that file's. The contract is a list-semantics one (add, remove by identity, copy on get,
//! first valid parser wins, an exception rejects, a fixed text when nothing matches), so the checks are against the
//! source rather than a recorded run; the format dispatch results of the default list are compared by
//! crates/x3d-viewer/tests/rea_batch09_sai.rs (parse-formats, parse-xml-mfstring, parse-xml-unknown).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use x3d_parse::{
    GoldenGate, GoldenGateParser, LoadError, ParseError, ParseOptions, ParserInput, ParserRef,
    ParserRegistry, builtin, no_handler_message, same_parser,
};
use x3d_scene::Scene;

const NAMES: [&str; 11] = [
    "XMLParser",
    "VRMLParser",
    "JSONParser",
    "GLTF2Parser",
    "GLB2Parser",
    "OBJParser",
    "STLAParser",
    "STLBParser",
    "PLYAParser",
    "PLYBParser",
    "SVGParser",
];

const OPTS: ParseOptions = ParseOptions { vrml1: false };
const URL: &str = "http://example.test/a.dat";
// Accepted by no built-in parser.
const NOISE: &str = "this is not any known format";

#[derive(Clone, Copy)]
enum Mode {
    /// `getInput` undefined or `isValid` false.
    Skip,
    /// Parses into a scene whose `profile` is the marker.
    Accept,
    /// Throws.
    Throw,
}

struct Probe {
    marker: &'static str,
    mode: Mode,
    calls: AtomicUsize,
    /// What the last call saw: (string, byte count, world url).
    seen: Mutex<Option<(Option<String>, usize, String)>>,
}

impl Probe {
    fn new(marker: &'static str, mode: Mode) -> Arc<Probe> {
        Arc::new(Probe {
            marker,
            mode,
            calls: AtomicUsize::new(0),
            seen: Mutex::new(None),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl GoldenGateParser for Probe {
    fn name(&self) -> &str {
        self.marker
    }

    fn parse(&self, input: &ParserInput<'_>) -> Result<Option<Scene>, LoadError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.seen.lock().unwrap() = Some((
            input.string().map(str::to_string),
            input.bytes().len(),
            input.world_url().to_string(),
        ));

        match self.mode {
            Mode::Skip => Ok(None),
            Mode::Accept => {
                let mut scene = Scene::new(input.world_url());

                scene.profile = Some(self.marker.into());
                Ok(Some(scene))
            }
            Mode::Throw => Err(LoadError::Parse(ParseError {
                message: format!("{} threw", self.marker),
                line: 0,
            })),
        }
    }
}

fn names(list: &[ParserRef]) -> Vec<String> {
    list.iter().map(|p| p.name().to_string()).collect()
}

fn profile(r: Result<Scene, LoadError>) -> String {
    r.expect("a scene").profile.unwrap_or_default()
}

// GoldenGate.js 19-31.
#[test]
fn default_list_is_the_pinned_order() {
    let list = ParserRegistry::new().get_parsers();

    assert_eq!(names(&list), NAMES);

    // The named accessors are the entries of the list, by identity.
    let named = [
        builtin::xml(),
        builtin::vrml(),
        builtin::json(),
        builtin::gltf2(),
        builtin::glb2(),
        builtin::obj(),
        builtin::stla(),
        builtin::stlb(),
        builtin::plya(),
        builtin::plyb(),
        builtin::svg(),
    ];

    for (a, b) in list.iter().zip(&named) {
        assert!(same_parser(a, b));
    }

    // Two registries share the singletons.
    assert!(same_parser(
        &ParserRegistry::new().get_parsers()[0],
        &list[0]
    ));
}

// 33-36: `this .#parsers .push (... args)` appends in argument order.
#[test]
fn add_parsers_appends_in_order() {
    let (a, b) = (Probe::new("a", Mode::Skip), Probe::new("b", Mode::Skip));
    let mut r = ParserRegistry::new();

    r.add_parsers([a.clone() as ParserRef, b.clone() as ParserRef]);

    let list = r.get_parsers();

    assert_eq!(list.len(), 13);
    assert_eq!(names(&list[11..]), ["a", "b"]);

    // addParsers () with no argument pushes nothing.
    r.add_parsers([]);
    assert_eq!(r.get_parsers().len(), 13);
}

// 35: `push` does not look for the parser first, so a duplicate is a second entry.
#[test]
fn duplicates_are_permitted_and_each_is_tried() {
    let p = Probe::new("p", Mode::Skip);
    let pr: ParserRef = p.clone();
    let mut r = ParserRegistry::new();

    r.add_parsers([pr.clone(), pr.clone()]);
    r.add_parsers([pr.clone()]);

    let list = r.get_parsers();

    assert_eq!(list.len(), 14);
    assert!(list[11..].iter().all(|x| same_parser(x, &pr)));

    // Input no built-in takes reaches all three copies, each given a new `new Parser (scene)` in JavaScript: three calls.
    assert!(matches!(
        r.parse_string_with(NOISE, URL, &OPTS),
        Err(LoadError::NoHandler)
    ));
    assert_eq!(p.calls(), 3);
}

// 38-41: `filter (parser => !args .includes (parser))` drops every copy, not one.
#[test]
fn remove_parsers_removes_every_copy() {
    let p = Probe::new("p", Mode::Skip);
    let q = Probe::new("q", Mode::Skip);
    let (pr, qr): (ParserRef, ParserRef) = (p, q);
    let mut r = ParserRegistry::empty();

    r.add_parsers([pr.clone(), qr.clone(), pr.clone(), pr.clone(), qr.clone()]);
    r.remove_parsers([&pr]);

    assert_eq!(names(&r.get_parsers()), ["q", "q"]);

    // Several arguments at once; an absent parser and no argument are no-ops.
    r.remove_parsers([&pr]);
    assert_eq!(r.get_parsers().len(), 2);
    r.remove_parsers([]);
    assert_eq!(r.get_parsers().len(), 2);
    r.remove_parsers([&pr, &qr]);
    assert!(r.get_parsers().is_empty());
}

// 40: `includes` is `===`: the same class, not an equal one. A clone of the Arc is the same parser; a new allocation of the
// same value is not.
#[test]
fn identity_is_the_allocation() {
    let first = Probe::new("same-name", Mode::Skip);
    let second = Probe::new("same-name", Mode::Skip);
    let (a, b): (ParserRef, ParserRef) = (first, second);

    assert!(same_parser(&a, &a.clone()));
    assert!(!same_parser(&a, &b));

    let mut r = ParserRegistry::empty();

    r.add_parsers([a.clone(), b.clone()]);
    r.remove_parsers([&a.clone()]);

    let left = r.get_parsers();

    assert_eq!(left.len(), 1);
    assert!(same_parser(&left[0], &b));
}

// 40: a built-in is removed by identity too, and re-added at the end like any other class.
#[test]
fn builtin_parsers_remove_and_readd_by_identity() {
    let mut r = ParserRegistry::new();

    r.remove_parsers([&builtin::stla(), &builtin::plyb()]);
    assert_eq!(
        names(&r.get_parsers()),
        [
            "XMLParser",
            "VRMLParser",
            "JSONParser",
            "GLTF2Parser",
            "GLB2Parser",
            "OBJParser",
            "STLBParser",
            "PLYAParser",
            "SVGParser"
        ]
    );

    r.add_parsers([builtin::stla()]);
    assert_eq!(names(&r.get_parsers()).last().unwrap(), "STLAParser");
}

// 43-46: `slice ()`.
#[test]
fn get_parsers_returns_a_copy() {
    let mut r = ParserRegistry::new();
    let mut copy = r.get_parsers();

    copy.clear();
    copy.push(Probe::new("x", Mode::Accept));
    assert_eq!(names(&r.get_parsers()), NAMES);

    // And the other way: later registry changes do not reach a copy taken before.
    let before = r.get_parsers();

    r.remove_parsers([&builtin::xml()]);
    r.add_parsers([Probe::new("y", Mode::Skip) as ParserRef]);
    assert_eq!(names(&before), NAMES);
}

// 50-67: `for (const Parser of GoldenGate .#parsers)`, the first valid parser returns.
#[test]
fn first_valid_parser_wins_in_registration_order() {
    let early = Probe::new("early", Mode::Accept);
    let late = Probe::new("late", Mode::Accept);
    let mut r = ParserRegistry::empty();

    r.add_parsers([early.clone() as ParserRef, late.clone() as ParserRef]);
    assert_eq!(profile(r.parse_string_with(NOISE, URL, &OPTS)), "early");
    assert_eq!(early.calls(), 1);
    assert_eq!(late.calls(), 0, "a parser after the winner is not asked");

    // Reversing the registration reverses the winner.
    let mut r = ParserRegistry::empty();

    r.add_parsers([late.clone() as ParserRef, early.clone() as ParserRef]);
    assert_eq!(profile(r.parse_string_with(NOISE, URL, &OPTS)), "late");
}

// 63-64: `if (!parser .isValid ()) continue;` and 58-59 (undefined input): an invalid parser is passed over.
#[test]
fn invalid_parsers_are_skipped_in_order() {
    let skip = Probe::new("skip", Mode::Skip);
    let win = Probe::new("win", Mode::Accept);
    let mut r = ParserRegistry::empty();

    r.add_parsers([skip.clone() as ParserRef, win.clone() as ParserRef]);
    assert_eq!(profile(r.parse_string_with(NOISE, URL, &OPTS)), "win");
    assert_eq!(skip.calls(), 1);
}

// Appended after the defaults, a parser is reached only by input the defaults leave over; placed before them (remove,
// add, add the defaults back) it wins over every default.
#[test]
fn added_parser_precedence_against_the_defaults() {
    let custom = Probe::new("custom", Mode::Accept);
    let x3dv = "#X3D V4.0 utf8\nPROFILE Full\n";
    let mut r = ParserRegistry::new();

    r.add_parsers([custom.clone() as ParserRef]);
    assert_eq!(
        r.parse_string_with(x3dv, URL, &OPTS).unwrap().encoding,
        "VRML"
    );
    assert_eq!(custom.calls(), 0);
    assert_eq!(profile(r.parse_string_with(NOISE, URL, &OPTS)), "custom");

    let defaults = r.get_parsers()[..11].to_vec();

    r.remove_parsers(defaults.iter());
    r.add_parsers(defaults.iter().cloned());

    // Now custom is first.
    let mut r2 = ParserRegistry::empty();

    r2.add_parsers([custom.clone() as ParserRef]);
    r2.add_parsers(defaults);
    assert_eq!(profile(r2.parse_string_with(x3dv, URL, &OPTS)), "custom");
}

// Removing a default stops it handling its format: the rest of the list is untouched.
#[test]
fn removing_a_default_parser_removes_its_format() {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"></svg>";
    let mut r = ParserRegistry::new();

    assert!(r.parse_string_with(svg, URL, &OPTS).is_ok());
    r.remove_parsers([&builtin::svg()]);
    assert!(matches!(
        r.parse_string_with(svg, URL, &OPTS),
        Err(LoadError::NoHandler)
    ));

    // The XML parser and the SVG parser share one parsed document; SVG alone still gets it when XML is gone.
    let mut r = ParserRegistry::new();

    r.remove_parsers([&builtin::xml()]);
    assert!(r.parse_string_with(svg, URL, &OPTS).is_ok());
}

// 70-77: the exception becomes `reject (error)` (a thrown error when no `reject`); no later parser runs.
#[test]
fn a_parser_exception_rejects_the_operation() {
    let boom = Probe::new("boom", Mode::Throw);
    let after = Probe::new("after", Mode::Accept);
    let mut r = ParserRegistry::empty();

    r.add_parsers([boom.clone() as ParserRef, after.clone() as ParserRef]);

    match r.parse_string_with(NOISE, URL, &OPTS) {
        Err(LoadError::Parse(e)) => assert_eq!(e.message, "boom threw"),
        other => panic!("{:?}", other.map(|_| ())),
    }

    assert_eq!(after.calls(), 0);

    // A built-in's own parse failure takes the same road: VRMLParser throws on a bad statement and the later parsers are
    // not tried.
    let tail = Probe::new("tail", Mode::Accept);
    let mut r = ParserRegistry::new();

    r.add_parsers([tail.clone() as ParserRef]);
    assert!(matches!(
        r.parse_string_with("#VRML V2.0 utf8\nProfile Full\n", URL, &OPTS),
        Err(LoadError::Parse(_))
    ));
    assert_eq!(tail.calls(), 0);
}

// 81-84.
#[test]
fn no_matching_parser_is_the_pinned_text() {
    let r = ParserRegistry::empty();
    let e = r.parse_string_with("anything", URL, &OPTS).err().unwrap();

    assert!(matches!(e, LoadError::NoHandler));
    assert_eq!(
        no_handler_message(URL),
        "Couldn't parse X3D. No suitable file handler found for 'http://example.test/a.dat'."
    );
    assert_eq!(
        no_handler_message("data:text/plain,hi"),
        "Couldn't parse X3D. No suitable file handler found for 'data:' URL."
    );
    // `startsWith ("data:")`, not a substring or a case-folded match.
    assert_eq!(
        no_handler_message("DATA:x"),
        "Couldn't parse X3D. No suitable file handler found for 'DATA:x'."
    );
    assert_eq!(
        no_handler_message("http://h/data:x"),
        "Couldn't parse X3D. No suitable file handler found for 'http://h/data:x'."
    );
    assert_eq!(
        no_handler_message(""),
        "Couldn't parse X3D. No suitable file handler found for ''."
    );

    // The default list has no handler for text of no format either.
    assert!(matches!(
        ParserRegistry::new().parse_string_with(NOISE, URL, &OPTS),
        Err(LoadError::NoHandler)
    ));
}

// 87-97: parsers are handed the converted inputs; the string input is undefined for bytes that decode to nothing
// (111-113), the ArrayBuffer input exists only for bytes (143-149).
#[test]
fn parsers_see_the_converted_inputs() {
    let p = Probe::new("p", Mode::Skip);
    let mut r = ParserRegistry::empty();

    r.add_parsers([p.clone() as ParserRef]);
    let _ = r.parse_string_with("hello", URL, &OPTS);
    assert_eq!(
        p.seen.lock().unwrap().clone(),
        Some((Some("hello".into()), 0, URL.into()))
    );

    // A BOM and text, as bytes: the decoded string loses the BOM, the bytes keep it.
    let _ = r.parse_bytes_with(b"\xEF\xBB\xBFhi", URL, &OPTS);
    assert_eq!(
        p.seen.lock().unwrap().clone(),
        Some((Some("hi".into()), 5, URL.into()))
    );

    // Non-empty bytes that decode to nothing: no string.
    let _ = r.parse_bytes_with(b"\xEF\xBB\xBF", URL, &OPTS);
    assert_eq!(p.seen.lock().unwrap().clone(), Some((None, 3, URL.into())));
}

// The default registry reproduces the former hard-wired dispatch for one input per format. The recorded comparison with
// X_ITE is crates/x3d-viewer/tests/rea_batch09_sai.rs; this guards the order in this crate.
#[test]
fn default_registry_dispatch_is_unchanged() {
    let cases: [(&str, &str, &str); 7] = [
        ("#X3D V4.0 utf8\nPROFILE Full\n", "VRML", "vrml"),
        (
            "<X3D profile='Full' version='4.0'><Scene/></X3D>",
            "XML",
            "xml",
        ),
        (
            "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{}}}",
            "JSON",
            "json",
        ),
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n", "", "obj"),
        (
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"2\" height=\"2\"></svg>",
            "",
            "svg",
        ),
        (
            "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid s\n",
            "",
            "stl",
        ),
        (
            "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n",
            "",
            "ply",
        ),
    ];

    for (text, encoding, what) in cases {
        let via_registry = ParserRegistry::new()
            .parse_string_with(text, URL, &OPTS)
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        let via_api = x3d_parse::parse_string(text, URL).unwrap();

        assert_eq!(
            via_registry.to_vrml_string(),
            via_api.to_vrml_string(),
            "{what}"
        );

        if !encoding.is_empty() {
            assert_eq!(via_registry.encoding, encoding, "{what}");
        }
    }
}

/// The process-wide registry is shared by every test of this binary, so these run one at a time and put it back.
static GLOBAL: Mutex<()> = Mutex::new(());

struct Restore(Vec<ParserRef>);

impl Drop for Restore {
    fn drop(&mut self) {
        let now = GoldenGate::get_parsers();

        GoldenGate::remove_parsers(now.iter());
        GoldenGate::add_parsers(self.0.drain(..));
    }
}

#[test]
fn process_wide_registry_drives_the_load_entry_points() {
    let _lock = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    let _restore = Restore(GoldenGate::get_parsers());

    assert_eq!(names(&GoldenGate::get_parsers()), NAMES);
    assert!(matches!(
        x3d_parse::parse_string(NOISE, URL),
        Err(LoadError::NoHandler)
    ));

    let p = Probe::new("global", Mode::Accept);
    let pr: ParserRef = p.clone();

    GoldenGate::add_parsers([pr.clone(), pr.clone()]);
    assert_eq!(GoldenGate::get_parsers().len(), 13);
    assert_eq!(profile(x3d_parse::parse_string(NOISE, URL)), "global");
    assert_eq!(
        profile(x3d_parse::parse_bytes(NOISE.as_bytes(), URL)),
        "global"
    );

    // Removal takes both copies, and the entry points stop reaching it.
    GoldenGate::remove_parsers([&pr]);
    assert_eq!(names(&GoldenGate::get_parsers()), NAMES);
    assert!(matches!(
        x3d_parse::parse_string(NOISE, URL),
        Err(LoadError::NoHandler)
    ));

    // A removed built-in is gone from the entry points.
    GoldenGate::remove_parsers([&builtin::vrml()]);
    assert!(matches!(
        x3d_parse::parse_string("#X3D V4.0 utf8\nPROFILE Full\n", URL),
        Err(LoadError::NoHandler)
    ));

    // getParsers is a copy here too.
    let mut copy = GoldenGate::get_parsers();

    copy.clear();
    assert_eq!(GoldenGate::get_parsers().len(), 10);
}
