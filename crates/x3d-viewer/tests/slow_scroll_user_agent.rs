//! X_ITE 16.5.0's SLOW_SCROLL test (`/Mac OS X|OculusBrowser/i .test (navigator .userAgent)`, ExamineViewer.js, LookAtViewer.js,
//! PlaneViewer.js, X3DFlyViewer.js), which `x3d_viewer::nav::slow_scroll_user_agent` reproduces. The navigation results of a
//! macOS user agent are compared with the pinned page in rea_b24_nav (the `-mac` cases).

use x3d_viewer::nav::slow_scroll_user_agent;

#[test]
fn macos_and_oculus_user_agents_scroll_slowly() {
    assert!(slow_scroll_user_agent(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
    ));
    assert!(slow_scroll_user_agent(
        "Mozilla/5.0 (X11; Linux x86_64; Quest 3) AppleWebKit/537.36 (KHTML, like Gecko) OculusBrowser/35.0 Chrome/130.0 Safari/537.36"
    ));
    // An iPhone or iPad user agent has "like Mac OS X", so it matches too, as in X_ITE.
    assert!(slow_scroll_user_agent(
        "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"
    ));
    // The regular expression has the `i` flag.
    assert!(slow_scroll_user_agent("mac os x"));
    assert!(slow_scroll_user_agent("OCULUSBROWSER"));
}

#[test]
fn other_user_agents_do_not() {
    assert!(!slow_scroll_user_agent(
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/156.0.0.0 Safari/537.36"
    ));
    assert!(!slow_scroll_user_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
    ));
    assert!(!slow_scroll_user_agent("MacOSX"));
    assert!(!slow_scroll_user_agent(""));
}
