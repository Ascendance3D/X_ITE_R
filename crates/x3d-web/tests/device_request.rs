//! The adapter request order of the web build (issue #142, rule BR3.1): the supported, unsupported and rejected cases.

use std::cell::RefCell;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use x3d_web::device_request::{
    ADAPTER_REQUEST_TRIES, Attempt, CHECKED_WGPU_VERSION,
    WGPU_REQUESTS_COMPATIBILITY_FEATURE_LEVEL, request_with_compatibility, request_with_retries,
};

/// Runs a future that never waits.
fn run<T>(f: impl Future<Output = T>) -> T {
    let mut f = pin!(f);
    let mut cx = Context::from_waker(Waker::noop());

    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("the request waited"),
    }
}

/// `request` that records its attempts and answers from `answers` in order.
fn recorded(
    answers: Vec<Result<&'static str, &'static str>>,
) -> (
    RefCell<Vec<Attempt>>,
    RefCell<Vec<Result<&'static str, &'static str>>>,
) {
    (
        RefCell::new(vec![]),
        RefCell::new(answers.into_iter().rev().collect()),
    )
}

#[test]
fn supported_requests_the_compatibility_level_first_and_logs_nothing() {
    let (seen, answers) = recorded(vec![Ok("compat adapter")]);
    let out = run(request_with_compatibility(true, |a| {
        seen.borrow_mut().push(a);
        let r = answers.borrow_mut().pop().unwrap().map_err(String::from);
        async move { r.map(String::from) }
    }))
    .unwrap();

    assert_eq!(*seen.borrow(), vec![Attempt::Compatibility]);
    assert_eq!(out.value, "compat adapter");
    assert_eq!(out.used, Attempt::Compatibility);
    assert!(out.diagnostics.is_empty());
}

#[test]
fn unsupported_keeps_the_current_request_alone_and_logs_nothing() {
    let (seen, answers) = recorded(vec![Ok("core adapter")]);
    let out = run(request_with_compatibility(false, |a| {
        seen.borrow_mut().push(a);
        let r = answers.borrow_mut().pop().unwrap().map_err(String::from);
        async move { r.map(String::from) }
    }))
    .unwrap();

    assert_eq!(*seen.borrow(), vec![Attempt::Current]);
    assert_eq!(out.used, Attempt::Current);
    assert!(out.diagnostics.is_empty());
}

#[test]
fn a_rejected_compatibility_request_retries_once_with_the_current_one_and_logs_why() {
    let (seen, answers) = recorded(vec![
        Err("featureLevel is not supported"),
        Ok("core adapter"),
    ]);
    let out = run(request_with_compatibility(true, |a| {
        seen.borrow_mut().push(a);
        let r = answers.borrow_mut().pop().unwrap().map_err(String::from);
        async move { r.map(String::from) }
    }))
    .unwrap();

    assert_eq!(
        *seen.borrow(),
        vec![Attempt::Compatibility, Attempt::Current]
    );
    assert_eq!(out.value, "core adapter");
    assert_eq!(out.used, Attempt::Current);
    assert_eq!(out.diagnostics.len(), 1);
    assert!(out.diagnostics[0].contains("featureLevel is not supported"));
    assert!(out.diagnostics[0].contains("retrying once"));
}

#[test]
fn an_adapter_that_lacks_the_feature_is_the_same_failure() {
    // The request closure reports a returned adapter without the feature level as an error.
    let (seen, answers) = recorded(vec![
        Err("the adapter lacks the compatibility feature level"),
        Ok("core"),
    ]);
    let out = run(request_with_compatibility(true, |a| {
        seen.borrow_mut().push(a);
        let r = answers.borrow_mut().pop().unwrap().map_err(String::from);
        async move { r.map(String::from) }
    }))
    .unwrap();

    assert_eq!(out.used, Attempt::Current);
    assert_eq!(seen.borrow().len(), 2);
}

#[test]
fn when_the_retry_fails_too_its_error_is_returned_and_no_third_request_is_made() {
    let (seen, answers) = recorded(vec![Err("compat rejected"), Err("no WebGPU adapter: none")]);
    let err = run(request_with_compatibility(true, |a| {
        seen.borrow_mut().push(a);
        let r = answers.borrow_mut().pop().unwrap().map_err(String::from);
        async move { r.map(String::from) }
    }))
    .unwrap_err();

    assert_eq!(err, "no WebGPU adapter: none");
    assert_eq!(
        *seen.borrow(),
        vec![Attempt::Compatibility, Attempt::Current]
    );
}

#[test]
fn unsupported_failure_is_the_current_requests_error_unchanged() {
    let (seen, answers) = recorded(vec![Err("no WebGPU adapter: none")]);
    let err = run(request_with_compatibility(false, |a| {
        seen.borrow_mut().push(a);
        let r = answers.borrow_mut().pop().unwrap().map_err(String::from);
        async move { r.map(String::from) }
    }))
    .unwrap_err();

    assert_eq!(err, "no WebGPU adapter: none");
    assert_eq!(seen.borrow().len(), 1);
}

/// The constant says what wgpu offers; this pins the version it was checked against (Cargo.lock), so that an upgrade
/// makes someone look at `RequestAdapterOptions` again and decide whether the constant can turn true.
#[test]
fn the_wgpu_check_is_for_the_locked_version() {
    let lock =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).unwrap();
    let wgpu = lock
        .split("[[package]]")
        .find(|p| p.contains("\nname = \"wgpu\"\n"))
        .expect("wgpu in Cargo.lock");

    assert!(
        wgpu.contains(&format!("version = \"{CHECKED_WGPU_VERSION}\"")),
        "wgpu changed from {CHECKED_WGPU_VERSION}: check whether RequestAdapterOptions has a feature level now, then update \
         WGPU_REQUESTS_COMPATIBILITY_FEATURE_LEVEL and CHECKED_WGPU_VERSION"
    );
    assert!(!WGPU_REQUESTS_COMPATIBILITY_FEATURE_LEVEL);
}

/// Chrome on Linux answers the first adapter request of a page with no adapter; the next request gets it. One request
/// only left the page without an engine (the X_ITE_R Browser Preview smoke test in Google Chrome 156).
#[test]
fn a_failed_adapter_request_is_asked_again() {
    let calls = RefCell::new(0);
    let (result, lines) = run(request_with_retries(ADAPTER_REQUEST_TRIES, || {
        *calls.borrow_mut() += 1;
        let n = *calls.borrow();
        async move {
            if n == 1 {
                Err("no adapter".to_string())
            } else {
                Ok("adapter")
            }
        }
    }));

    assert_eq!(result, Ok("adapter"));
    assert_eq!(*calls.borrow(), 2);
    assert_eq!(
        lines,
        vec![format!(
            "X_ITE_R: WebGPU adapter request 1 of {ADAPTER_REQUEST_TRIES} failed (no adapter); asking again."
        )]
    );
}

#[test]
fn adapter_requests_stop_at_the_limit_with_the_last_reason() {
    let calls = RefCell::new(0);
    let (result, lines) = run(request_with_retries(3, || {
        *calls.borrow_mut() += 1;
        let n = *calls.borrow();
        async move { Err::<&str, _>(format!("no adapter {n}")) }
    }));

    assert_eq!(result, Err("no adapter 3".to_string()));
    assert_eq!(*calls.borrow(), 3);
    assert_eq!(lines.len(), 2);
}

#[test]
fn a_first_adapter_is_requested_once() {
    let calls = RefCell::new(0);
    let (result, lines) = run(request_with_retries(3, || {
        *calls.borrow_mut() += 1;
        async { Ok::<_, String>("adapter") }
    }));

    assert_eq!(result, Ok("adapter"));
    assert_eq!(*calls.borrow(), 1);
    assert!(lines.is_empty());
    assert_eq!(ADAPTER_REQUEST_TRIES, 3);
}
