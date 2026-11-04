//! The `Fetcher` response contract (`fetch_response`: status, headers, content type, final URL) as FileLoader.js reads it, without HTTP:
//! the `text/html` and `application/xhtml+xml` hand-off to `foreign`, with the requested URL as href; the exact comparison of the mime
//! type; a fetcher with only `fetch` (every fetcher before the contract) is read as status 200 without headers. The comparison with the
//! pinned page over real HTTP is crates/x3d-viewer/tests/file_http.rs.

use x3d_runtime::loader::{FileLoader, LoadHooks, LoadResult, Loaded};
use x3d_runtime::{Fetcher, Response, Runtime};

const WORLD: &str = "http://oracle/w/world.wrl";
const BODY: &[u8] = b"#VRML V2.0 utf8\nGroup { }\n";

/// Answers `/ok` (model/vrml), `/page` (the given content type) and `/moved` (a redirect to `/page`, final URL `/final/page`).
struct Canned(&'static str);

impl Fetcher for Canned {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        self.fetch_response(url).map(|r| r.body)
    }

    fn fetch_response(&self, url: &url::Url) -> Result<Response, String> {
        let mut response = Response::ok(url.clone(), BODY.to_vec());

        match url.path() {
            "/w/ok" => response
                .headers
                .push(("content-type".into(), "model/vrml".into())),
            "/w/page" => response
                .headers
                .push(("content-type".into(), self.0.into())),
            "/w/moved" => {
                response.url = url::Url::parse("http://oracle/final/page").unwrap();
                response.redirected = true;
                response
                    .headers
                    .push(("content-type".into(), self.0.into()));
            }
            "/w/gone" => {
                response.status = 404;
                response.status_text = "Not Found".into();
                response
                    .headers
                    .push(("content-type".into(), "text/html".into()));
                return response.check();
            }
            _ => {}
        }

        Ok(response)
    }
}

fn load(content_type: &'static str, url: &str, parameters: &[&str]) -> (LoadResult, usize) {
    let scene = x3d_parse::parse_bytes(BODY, WORLD).unwrap();
    let mut rt = Runtime::new(scene, Box::new(Canned(content_type)));
    let loader = FileLoader {
        type_name: String::new(),
        base: WORLD.to_string(),
        data_as_string: true,
    };
    let hooks = LoadHooks {
        bind_viewpoint: true,
        foreign: true,
        parameters: parameters.iter().map(|s| s.to_string()).collect(),
    };
    let mut accepted = 0;
    let result = rt.load_document_hooks(&loader, &[url.to_string()], &hooks, |_, loaded| {
        if let Loaded::Data(..) = loaded {
            accepted += 1;
        }

        Ok(())
    });

    (result, accepted)
}

#[test]
fn text_html_and_xhtml_go_to_foreign_with_the_requested_url() {
    for ct in [
        "text/html",
        "application/xhtml+xml",
        "text/html; charset=utf-8",
        "text/html;x",
    ] {
        let (result, accepted) = load(ct, "page", &[]);

        assert_eq!(
            result,
            LoadResult::Foreign("http://oracle/w/page".into(), String::new()),
            "{ct}"
        );
        assert_eq!(accepted, 0, "{ct}");
    }
}

#[test]
fn the_href_is_the_requested_url_not_the_final_response_url() {
    let (result, _) = load("text/html", "moved?q=1#F", &["target=_self"]);

    assert_eq!(
        result,
        LoadResult::Foreign("http://oracle/w/moved?q=1#F".into(), "_self".into())
    );
}

/// The comparison is exact: the case, a space before the semicolon, another type, and a missing header are not the hand-off.
#[test]
fn only_the_exact_mime_types_are_handed_off() {
    for ct in [
        "TEXT/HTML",
        "text/html ;charset=x",
        "text/plain",
        "model/vrml",
        "application/xhtml",
        "",
    ] {
        let (result, accepted) = load(ct, "page", &[]);

        assert_eq!(result, LoadResult::Loaded, "{ct:?}");
        assert_eq!(accepted, 1, "{ct:?}");
    }
}

#[test]
fn a_failed_status_fails_before_the_content_type_is_read() {
    let (result, accepted) = load("text/html", "gone", &[]);

    assert_eq!(result, LoadResult::Failed);
    assert_eq!(accepted, 0);
}

/// FileLoader.js hands over only when `foreign` was given: createX3DFromURL of a Script has none.
#[test]
fn without_the_foreign_hook_a_html_response_is_read_like_any_other() {
    let scene = x3d_parse::parse_bytes(BODY, WORLD).unwrap();
    let mut rt = Runtime::new(scene, Box::new(Canned("text/html")));
    let loader = FileLoader {
        type_name: String::new(),
        base: WORLD.to_string(),
        data_as_string: true,
    };
    let mut accepted = 0;
    let result = rt.load_document_hooks(
        &loader,
        &["page".to_string()],
        &LoadHooks::default(),
        |_, _| {
            accepted += 1;
            Ok(())
        },
    );

    assert_eq!((result, accepted), (LoadResult::Loaded, 1));
}

/// A fetcher that has only `fetch` still works: its default `fetch_response` is status 200 and no header.
#[test]
fn a_fetcher_with_only_fetch_is_never_handed_off() {
    struct Old;

    impl Fetcher for Old {
        fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
            Ok(BODY.to_vec())
        }
    }

    let scene = x3d_parse::parse_bytes(BODY, WORLD).unwrap();
    let mut rt = Runtime::new(scene, Box::new(Old));
    let loader = FileLoader {
        type_name: String::new(),
        base: WORLD.to_string(),
        data_as_string: true,
    };
    let hooks = LoadHooks {
        bind_viewpoint: true,
        foreign: true,
        parameters: vec![],
    };
    let result = rt.load_document_hooks(&loader, &["x".to_string()], &hooks, |_, _| Ok(()));

    assert_eq!(result, LoadResult::Loaded);
}
