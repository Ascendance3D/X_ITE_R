//! HttpFetcher against the local route server (std only, 127.0.0.1): the response contract (final URL, status, reason,
//! headers, content type, body), redirects, and every refusal. The pinned page's behavior for the same routes is compared in
//! crates/x3d-viewer/tests/file_http.rs; this file checks the parts that page cannot show (the response object itself, limits,
//! timeouts, requests that must not be made).

use std::collections::BTreeMap;
use std::time::Duration;
use x3d_http::route_server::{Route, RouteServer};
use x3d_http::{HttpError, HttpFetcher, HttpOptions, NETWORK_ERROR};
use x3d_runtime::{Fetcher, Response};

fn options() -> HttpOptions {
    HttpOptions {
        proxy_from_env: false,
        total_timeout: Duration::from_secs(20),
        ..HttpOptions::default()
    }
}

fn redirect(status: u16, location: &str) -> Route {
    Route::new(status, &[("Location", location)], b"")
}

fn table(entries: Vec<(&str, Route)>) -> BTreeMap<String, Route> {
    entries
        .into_iter()
        .map(|(n, r)| (n.to_string(), r))
        .collect()
}

fn url(server: &RouteServer, path: &str) -> url::Url {
    url::Url::parse(&format!("{}{path}", server.origin)).unwrap()
}

fn base_routes() -> Vec<(&'static str, Route)> {
    vec![
        (
            "ok",
            Route::new(
                200,
                &[
                    ("Content-Type", "model/vrml; charset=utf-8"),
                    ("X-Custom", "abc"),
                ],
                b"body",
            ),
        ),
        ("r301", redirect(301, "/h/ok")),
        ("r302", redirect(302, "ok")),
        ("r303", redirect(303, "/h/ok?x=1#F")),
        ("r307", redirect(307, "/h/ok")),
        ("r308", redirect(308, "/h/ok")),
    ]
}

#[test]
fn the_response_has_final_url_status_headers_content_type_and_body() {
    let server = RouteServer::start(table(base_routes()));
    let fetcher = HttpFetcher::new(options());
    let response = fetcher.get(&url(&server, "/h/ok")).unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.status_text, "OK");
    assert_eq!(response.url, url(&server, "/h/ok"));
    assert!(!response.redirected);
    assert_eq!(response.body, b"body");
    assert_eq!(response.header("X-Custom").as_deref(), Some("abc"));
    assert_eq!(response.header("content-length").as_deref(), Some("4"));
    assert_eq!(response.mime_type().as_deref(), Some("model/vrml"));
    assert_eq!(
        response.header("content-type").as_deref(),
        Some("model/vrml; charset=utf-8")
    );
    assert_eq!(fetcher.fetch(&url(&server, "/h/ok")).unwrap(), b"body");
}

#[test]
fn all_five_redirect_statuses_are_followed_with_a_get() {
    let server = RouteServer::start(table(base_routes()));
    let fetcher = HttpFetcher::new(options());

    for name in ["r301", "r302", "r303", "r307", "r308"] {
        let response = fetcher.get(&url(&server, &format!("/h/{name}"))).unwrap();

        assert_eq!(response.status, 200, "{name}");
        assert!(response.redirected, "{name}");
        assert_eq!(response.body, b"body", "{name}");
        assert!(response.url.path().ends_with("/h/ok"), "{name}");
    }

    assert!(server.requests().iter().all(|r| r.starts_with("GET ")));
}

#[test]
fn the_final_url_of_a_redirect_is_the_last_one_and_keeps_the_query() {
    let server = RouteServer::start(table(base_routes()));
    let response = HttpFetcher::new(options())
        .get(&url(&server, "/h/r303"))
        .unwrap();

    assert_eq!(response.url.path(), "/h/ok");
    assert_eq!(response.url.query(), Some("x=1"));
}

#[test]
fn a_chain_of_20_redirects_is_followed_and_the_21st_fails() {
    let mut routes = vec![("end", Route::new(200, &[], b"end"))];
    let names: Vec<String> = (0..21).map(|n| format!("c{n}")).collect();

    for (n, name) in names.iter().enumerate() {
        let next = if n == 20 {
            "/h/end".to_string()
        } else {
            format!("/h/c{}", n + 1)
        };

        routes.push((name.as_str(), redirect(302, &next)));
    }

    let server = RouteServer::start(table(routes));
    let fetcher = HttpFetcher::new(options());
    // c1 -> ... -> c20 -> end is 20 redirects; c0 -> ... is 21.
    let ok = fetcher.get(&url(&server, "/h/c1")).unwrap();

    assert_eq!(ok.body, b"end");

    let failed = fetcher.get(&url(&server, "/h/c0"));

    assert!(matches!(failed, Err(HttpError::Network(_))), "{failed:?}");
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/c0")).unwrap_err(),
        NETWORK_ERROR
    );
}

#[test]
fn the_redirect_limit_is_an_option() {
    let server = RouteServer::start(table(vec![
        ("a", redirect(302, "/h/b")),
        ("b", redirect(302, "/h/c")),
        ("c", Route::new(200, &[], b"c")),
    ]));
    let tight = HttpFetcher::new(HttpOptions {
        max_redirects: 1,
        ..options()
    });

    assert!(tight.get(&url(&server, "/h/a")).is_err());
    assert!(tight.get(&url(&server, "/h/b")).is_ok());
}

#[test]
fn a_redirect_loop_fails() {
    let server = RouteServer::start(table(vec![
        ("a", redirect(302, "/h/b")),
        ("b", redirect(302, "/h/a")),
        ("self", redirect(307, "/h/self")),
    ]));
    let fetcher = HttpFetcher::new(options());

    for name in ["a", "self"] {
        assert_eq!(
            fetcher.fetch(&url(&server, &format!("/h/{name}"))),
            Err(NETWORK_ERROR.to_string()),
            "{name}"
        );
    }

    // Each loop stopped at the limit: the first request and 20 redirects followed, then the 21st redirect fails unrequested.
    assert_eq!(server.requests().len(), 2 * 21);
}

/// The `foreign` hand-off reads no body (FileLoader.js 304-312): a page larger than the body limit, or cut short after its
/// head, still comes back as a response with its headers; without the hand-off the same routes fail as a body does.
#[test]
fn a_page_for_the_foreign_hand_off_is_not_read() {
    let big = vec![b'x'; 5000];
    let mut cut = Route::new(
        200,
        &[("Content-Type", "text/html; charset=utf-8")],
        b"0123456789012345678901234567890123456789",
    );
    cut.truncate_to = Some(10);
    let server = RouteServer::start(table(vec![
        (
            "big",
            Route::new(200, &[("Content-Type", "application/xhtml+xml")], &big),
        ),
        ("cut", cut),
        (
            "vrml",
            Route::new(200, &[("Content-Type", "model/vrml")], &big),
        ),
    ]));
    let fetcher = HttpFetcher::new(HttpOptions {
        max_body_bytes: 1000,
        ..options()
    });
    let unread = &x3d_runtime::FOREIGN_MIME_TYPES;

    for name in ["big", "cut"] {
        let response = fetcher
            .fetch_response_unless(&url(&server, &format!("/h/{name}")), unread)
            .unwrap_or_else(|e| panic!("{name}: {e}"));

        assert_eq!(response.status, 200, "{name}");
        assert!(response.body.is_empty(), "{name}");
    }

    // A mime type outside `unread` is read, and the limit holds.
    assert_eq!(
        fetcher.fetch_response_unless(&url(&server, "/h/vrml"), unread),
        Err("Error: The response body is larger than 1000 bytes".to_string())
    );
    // Negative control: without the hand-off list, the same pages are read and fail.
    assert_eq!(
        fetcher.fetch_response_unless(&url(&server, "/h/big"), &[]),
        Err("Error: The response body is larger than 1000 bytes".to_string())
    );
    assert_eq!(
        fetcher.fetch_response_unless(&url(&server, "/h/cut"), &[]),
        Err("TypeError: network error".to_string())
    );
}

/// An origin map never leaves HTTP and HTTPS.
#[test]
fn an_origin_map_maps_http_origins_only() {
    assert!(x3d_http::OriginMap::parse("http://oracle=http://127.0.0.1:1").is_ok());
    assert!(x3d_http::OriginMap::parse("http://oracle=file:///tmp").is_err());
    assert!(x3d_http::OriginMap::parse("ftp://oracle=http://127.0.0.1:1").is_err());
}

/// A redirect never leaves HTTP and HTTPS: every other scheme fails the request, and the target is never read.
#[test]
fn a_redirect_to_another_scheme_fails_without_a_second_request() {
    let targets = [
        "file:///etc/hostname",
        "data:text/plain,hello",
        "ftp://127.0.0.1/x",
        "javascript:alert(1)",
        "ecmascript:x",
        "blob:http://127.0.0.1/none",
        "about:blank",
        "ws://127.0.0.1/x",
        "wss://127.0.0.1/x",
        "mailto:a@example.invalid",
        "urn:synthetic:thing",
        "http://[::1",
        "http://",
    ];
    let routes: Vec<(String, Route)> = targets
        .iter()
        .enumerate()
        .map(|(i, t)| (format!("t{i}"), redirect(302, t)))
        .collect();
    let server = RouteServer::start(routes.iter().map(|(n, r)| (n.clone(), r.clone())).collect());
    let fetcher = HttpFetcher::new(options());

    for (i, target) in targets.iter().enumerate() {
        assert_eq!(
            fetcher.fetch(&url(&server, &format!("/h/t{i}"))),
            Err(NETWORK_ERROR.to_string()),
            "{target}"
        );
    }

    // One request for each redirect response, none for a target.
    assert_eq!(server.requests().len(), targets.len());
}

#[test]
fn a_redirect_to_a_url_with_credentials_fails() {
    let server = RouteServer::start(table(vec![
        ("user", redirect(302, "http://user@127.0.0.1:9/h/ok")),
        ("both", redirect(302, "http://user:pw@127.0.0.1:9/h/ok")),
        ("empty-user", redirect(302, "http://:pw@127.0.0.1:9/h/ok")),
    ]));
    let fetcher = HttpFetcher::new(options());

    for name in ["user", "both", "empty-user"] {
        assert_eq!(
            fetcher.fetch(&url(&server, &format!("/h/{name}"))),
            Err(NETWORK_ERROR.to_string()),
            "{name}"
        );
    }

    assert_eq!(server.requests().len(), 3);
}

#[test]
fn a_request_url_with_credentials_or_another_scheme_is_refused_before_any_connection() {
    let fetcher = HttpFetcher::new(options());

    for text in [
        "http://user:pw@127.0.0.1:9/x",
        "http://user@127.0.0.1:9/x",
        "file:///etc/hostname",
        "data:text/plain,x",
        "ftp://127.0.0.1/x",
        "ws://127.0.0.1/x",
    ] {
        let u = url::Url::parse(text).unwrap();

        assert!(
            matches!(fetcher.get(&u), Err(HttpError::Network(_))),
            "{text}"
        );
        assert_eq!(fetcher.fetch(&u), Err(NETWORK_ERROR.to_string()), "{text}");
    }
}

#[test]
fn a_3xx_without_a_location_is_a_failed_response_not_a_redirect() {
    let server = RouteServer::start(table(vec![
        ("none", Route::new(301, &[], b"")),
        ("multiple", redirect(300, "/h/ok")),
        ("notmod", redirect(304, "/h/ok")),
        ("ok", Route::new(200, &[], b"ok")),
    ]));
    let fetcher = HttpFetcher::new(options());

    assert_eq!(
        fetcher.fetch(&url(&server, "/h/none")),
        Err("Error: Moved Permanently".to_string())
    );
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/multiple")),
        Err("Error: Multiple Choices".to_string())
    );
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/notmod")),
        Err("Error: Not Modified".to_string())
    );
    // The status of a failed response is in the response of `get`.
    let none = fetcher.get(&url(&server, "/h/none")).unwrap();

    assert_eq!((none.status, none.is_ok()), (301, false));
    assert!(none.body.is_empty());
}

#[test]
fn a_failed_status_is_error_status_text_and_get_keeps_status_and_headers() {
    let server = RouteServer::start(table(vec![
        ("s404", Route::new(404, &[("X-Why", "none")], b"not here")),
        ("s500", Route::new(500, &[], b"")),
        ("s503", Route::new(503, &[], b"")),
    ]));
    let fetcher = HttpFetcher::new(options());

    assert_eq!(
        fetcher.fetch(&url(&server, "/h/s404")),
        Err("Error: Not Found".to_string())
    );
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/s500")),
        Err("Error: Internal Server Error".to_string())
    );
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/s503")),
        Err("Error: Service Unavailable".to_string())
    );
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/missing")),
        Err("Error: Not Found".to_string())
    );

    let response = fetcher.get(&url(&server, "/h/s404")).unwrap();

    assert_eq!(response.status, 404);
    assert_eq!(response.header("x-why").as_deref(), Some("none"));
}

/// `Response::check` is `checkResponse`: the status text, or the status when the text is empty.
#[test]
fn check_response_uses_the_status_text_or_else_the_status() {
    let mut response = Response::ok(url::Url::parse("http://a.invalid/").unwrap(), vec![]);

    response.status = 599;
    response.status_text = String::new();
    assert_eq!(response.clone().check().unwrap_err(), "Error: 599");
    response.status_text = "Odd".into();
    assert_eq!(response.clone().check().unwrap_err(), "Error: Odd");
    response.status = 204;
    assert!(response.check().is_ok());
}

#[test]
fn a_repeated_header_is_joined_and_the_mime_type_is_cut_at_the_first_semicolon() {
    let mut response = Response::ok(url::Url::parse("http://a.invalid/").unwrap(), vec![]);

    response.headers = vec![
        ("content-type".into(), "text/html ;charset=x".into()),
        ("x-r".into(), "one".into()),
        ("X-R".into(), "two".into()),
    ];
    assert_eq!(response.header("x-r").as_deref(), Some("one, two"));
    assert_eq!(response.mime_type().as_deref(), Some("text/html "));
    assert_eq!(response.header("missing"), None);
    response.headers.clear();
    assert_eq!(response.mime_type(), None);
}

#[test]
fn content_encoding_gzip_is_decoded_once() {
    use std::io::Write;

    let gz = |data: &[u8]| {
        let mut e = flate2::write::GzEncoder::new(vec![], flate2::Compression::default());

        e.write_all(data).unwrap();
        e.finish().unwrap()
    };
    let once = gz(b"plain text");
    let twice = gz(&once);
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let routes = x3d_http::route_server::parse_routes(&format!(
        r#"{{ "once": {{ "headers": {{ "Content-Encoding": "gzip" }}, "bodyHex": "{}" }},
              "twice": {{ "headers": {{ "Content-Encoding": "gzip" }}, "bodyHex": "{}" }},
              "file": {{ "headers": {{ "Content-Type": "application/gzip" }}, "bodyHex": "{}" }} }}"#,
        hex(&once),
        hex(&twice),
        hex(&once)
    ))
    .unwrap();
    let server = RouteServer::start(routes);
    let fetcher = HttpFetcher::new(options());

    assert_eq!(
        fetcher.fetch(&url(&server, "/h/once")).unwrap(),
        b"plain text"
    );
    // A body that is still a gzip file after the transport decoding is left to the loader's gunzip.
    assert_eq!(fetcher.fetch(&url(&server, "/h/twice")).unwrap(), once);
    assert_eq!(fetcher.fetch(&url(&server, "/h/file")).unwrap(), once);
}

#[test]
fn a_body_over_the_limit_is_refused_by_length_and_by_reading() {
    let big = vec![b'x'; 5000];
    let server = RouteServer::start(table(vec![
        ("big", Route::new(200, &[], &big)),
        // No usable length: chunk-less body whose Content-Length is wrong is a protocol error, so use the decoded size.
        ("small", Route::new(200, &[], b"small")),
    ]));
    let fetcher = HttpFetcher::new(HttpOptions {
        max_body_bytes: 1000,
        ..options()
    });

    assert!(matches!(
        fetcher.get(&url(&server, "/h/big")),
        Err(HttpError::BodyTooLarge(1000))
    ));
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/big")),
        Err("Error: The response body is larger than 1000 bytes".to_string())
    );
    assert_eq!(fetcher.fetch(&url(&server, "/h/small")).unwrap(), b"small");
}

#[test]
fn a_decoded_body_over_the_limit_is_refused_when_the_compressed_body_is_small() {
    use std::io::Write;

    let mut e = flate2::write::GzEncoder::new(vec![], flate2::Compression::best());

    e.write_all(&vec![b'a'; 200_000]).unwrap();

    let gz = e.finish().unwrap();

    assert!(gz.len() < 1000);

    let hex: String = gz.iter().map(|x| format!("{x:02x}")).collect();
    let server = RouteServer::start(
        x3d_http::route_server::parse_routes(&format!(
            r#"{{ "bomb": {{ "headers": {{ "Content-Encoding": "gzip" }}, "bodyHex": "{hex}" }} }}"#
        ))
        .unwrap(),
    );
    let fetcher = HttpFetcher::new(HttpOptions {
        max_body_bytes: 10_000,
        ..options()
    });

    assert!(matches!(
        fetcher.get(&url(&server, "/h/bomb")),
        Err(HttpError::BodyTooLarge(10_000))
    ));
}

#[test]
fn a_slow_response_times_out_and_says_so() {
    let mut slow = Route::new(200, &[], b"late");

    slow.delay = Duration::from_millis(1500);

    let server = RouteServer::start(table(vec![("slow", slow)]));
    let by_read = HttpFetcher::new(HttpOptions {
        read_timeout: Duration::from_millis(200),
        ..options()
    });
    let by_total = HttpFetcher::new(HttpOptions {
        total_timeout: Duration::from_millis(300),
        ..options()
    });

    for fetcher in [by_read, by_total] {
        assert!(matches!(
            fetcher.get(&url(&server, "/h/slow")),
            Err(HttpError::Timeout)
        ));
        assert_eq!(
            fetcher.fetch(&url(&server, "/h/slow")),
            Err("Error: The request timed out".to_string())
        );
    }
}

#[test]
fn the_total_timeout_covers_all_redirects() {
    let mut slow = redirect(302, "/h/b");

    slow.delay = Duration::from_millis(300);

    let mut slow_b = redirect(302, "/h/c");

    slow_b.delay = Duration::from_millis(300);

    let server = RouteServer::start(table(vec![
        ("a", slow),
        ("b", slow_b),
        ("c", Route::new(200, &[], b"c")),
    ]));
    let fetcher = HttpFetcher::new(HttpOptions {
        total_timeout: Duration::from_millis(500),
        ..options()
    });

    assert!(matches!(
        fetcher.get(&url(&server, "/h/a")),
        Err(HttpError::Timeout)
    ));
}

#[test]
fn a_closed_connection_and_a_cut_body_are_network_errors_with_the_pages_texts() {
    let mut hangup = Route::new(200, &[], b"");

    hangup.hangup = true;

    let mut cut = Route::new(200, &[], b"0123456789012345678901234567890123456789");

    cut.truncate_to = Some(10);

    let server = RouteServer::start(table(vec![("hangup", hangup), ("cut", cut)]));
    let fetcher = HttpFetcher::new(options());

    // A closed connection before the head is `Failed to fetch`; a body that ends early is `network error` (the page's `blob ()`).
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/hangup")),
        Err(NETWORK_ERROR.to_string())
    );
    assert_eq!(
        fetcher.fetch(&url(&server, "/h/cut")),
        Err("TypeError: network error".to_string())
    );

    // Nothing listens on port 1.
    assert_eq!(
        fetcher.fetch(&url::Url::parse("http://127.0.0.1:1/x").unwrap()),
        Err(NETWORK_ERROR.to_string())
    );
}

#[test]
fn the_origin_map_sends_the_request_to_another_origin_and_keeps_the_url() {
    let server = RouteServer::start(table(base_routes()));
    let to = url::Url::parse(&server.origin).unwrap();
    let from = url::Url::parse("http://oracle/").unwrap();
    let fetcher = HttpFetcher::new(options()).with_origin_map(vec![(from, to)]);
    let asked = url::Url::parse("http://oracle/h/r301").unwrap();
    let response = fetcher.get(&asked).unwrap();

    assert_eq!(response.body, b"body");
    // The final URL is mapped back: the page never sees the address of the test server.
    assert_eq!(response.url.as_str(), "http://oracle/h/ok");
    assert_eq!(server.requests(), ["GET /h/r301", "GET /h/ok"]);

    // Another origin is not mapped: it fails like a host that does not exist.
    assert!(
        fetcher
            .get(&url::Url::parse("http://oracle2.invalid/h/ok").unwrap())
            .is_err()
    );
}

#[test]
fn a_redirect_to_the_second_origin_is_followed_over_http() {
    let server = RouteServer::start(table(vec![
        ("cross", redirect(302, "{origin2}/h/ok")),
        ("ok", Route::new(200, &[], b"second")),
    ]));
    let response = HttpFetcher::new(options())
        .get(&url(&server, "/h/cross"))
        .unwrap();

    assert_eq!(response.body, b"second");
    assert!(response.redirected);
    assert!(response.url.as_str().starts_with(&server.origin2));
}

/// The default `fetch_response` of a fetcher that has only `fetch` (every fetcher before this contract).
#[test]
fn a_fetcher_with_only_fetch_answers_status_200_without_headers() {
    struct Old;

    impl Fetcher for Old {
        fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
            Ok(b"bytes".to_vec())
        }
    }

    let u = url::Url::parse("http://a.invalid/x").unwrap();
    let response = Old.fetch_response(&u).unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.url, u);
    assert!(response.headers.is_empty());
    assert_eq!(response.mime_type(), None);
    assert_eq!(response.body, b"bytes");
}

#[test]
fn a_protocol_relative_redirect_keeps_the_scheme() {
    let server = RouteServer::start(table(vec![
        ("pr", redirect(302, "//{hostport}/h/ok")),
        ("ok", Route::new(200, &[], b"pr")),
    ]));
    let response = HttpFetcher::new(options())
        .get(&url(&server, "/h/pr"))
        .unwrap();

    assert_eq!(response.body, b"pr");
}
