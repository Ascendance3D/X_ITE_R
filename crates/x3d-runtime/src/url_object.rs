//! The part of X3DUrlObject.js that Inline nodes and EXTERNPROTO declarations share: the auto
//! refresh arithmetic. Each owner keeps its own state (`nodes::UrlState`, `extern_proto::ExternState`)
//! and calls these functions; the arithmetic exists once.

/// X3DUrlObject.set_autoRefresh__: seconds until the next refresh, `elapsed` seconds after the
/// load completed. A refresh that is overdue waits for the next multiple of the interval.
pub fn refresh_interval(auto_refresh: f64, elapsed: f64) -> f64 {
    let auto_refresh = auto_refresh.max(0.0);
    let interval = auto_refresh - elapsed;

    if interval < 0.0 {
        (elapsed / auto_refresh).ceil() * auto_refresh - elapsed
    } else {
        interval
    }
}

/// X3DUrlObject.setAutoRefreshTimer: the due time (ms) of the one timer, `None` when there is none.
/// `auto_refresh <= 0` has no timer, and `limit` (autoRefreshTimeLimit, seconds, 0 = no limit)
/// stops the refreshes once the node is older than `limit - interval`.
pub fn refresh_timer_due(
    now_ms: f64,
    start_ms: f64,
    auto_refresh: f64,
    limit: f64,
    interval: f64,
) -> Option<f64> {
    if auto_refresh <= 0.0 {
        return None;
    }

    if limit > 0.0 && (now_ms - start_ms) / 1000.0 > limit - interval {
        return None;
    }

    Some(now_ms + interval * 1000.0)
}
