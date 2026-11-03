//! Distances of float32 values in ulps, for the tests that allow a last-digit difference to the GL oracle.

use serde_json::Value;

/// The distance of two float32 values in representable steps (`u64::MAX` for a NaN).
pub fn f32_ulps(a: f64, b: f64) -> u64 {
    let (a, b) = (a as f32, b as f32);

    if a.is_nan() || b.is_nan() {
        return u64::MAX;
    }

    // Order the bit patterns as numbers (negative values count down from zero).
    let key = |x: f32| {
        let bits = x.to_bits() as i64;

        if bits & 0x8000_0000 != 0 {
            -(bits & 0x7fff_ffff)
        } else {
            bits
        }
    };

    key(a).abs_diff(key(b))
}

/// `same_value`, with numbers allowed to differ by `max_ulps` float32 ulps; `seen` takes the largest distance found.
pub fn within_ulps(a: &Value, b: &Value, max_ulps: u64, seen: &mut u64) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            let d = if x == y { 0 } else { f32_ulps(x, y) };

            *seen = (*seen).max(d);
            d <= max_ulps
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|(p, q)| within_ulps(p, q, max_ulps, seen))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, p)| y.get(k).is_some_and(|q| within_ulps(p, q, max_ulps, seen)))
        }
        _ => a == b,
    }
}

/// The absolute tolerance of a run on a software rasterizer (issue #89), or `None` on a GPU. A run sets
/// `X3D_TEST_SOFTWARE_TOL` to the number (1e-4 in the classification record docs/particle-array-size-backends.md, fixed
/// before the llvmpipe run that uses it): a software backend rounds the depth interpolation and the float math
/// differently from the answer keys' discrete GPU. The default is strict. A bad value is a test failure, not a skip, and so
/// is the variable on a hardware adapter ([`decide_software_tol`]). A test calls this once at its start.
pub fn software_tol() -> Option<f64> {
    let raw = std::env::var("X3D_TEST_SOFTWARE_TOL").ok();
    let tol = parse_software_tol(raw.as_deref()).unwrap_or_else(|e| panic!("{e}"));

    if tol.is_none() {
        return None;
    }

    // The adapter the pointing pass would take (x3d_render::pointing_gpu::PointingGpu::new_headless): the same backend
    // choice and power preference. Without one, the variable is an error (below).
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = x3d_render::gpu::headless_backends(
        std::env::var("X3D_POINTING_BACKEND")
            .or_else(|_| std::env::var("X3D_BACKEND"))
            .ok()
            .as_deref(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }));
    // The variable says "this run is on a software rasterizer". With no adapter at all that cannot be checked, and a skip
    // would hide the misconfiguration, so it is a failure (MiniMax review of PR #358).
    let adapter = adapter.unwrap_or_else(|e| {
        panic!("X3D_TEST_SOFTWARE_TOL is set, but no adapter could be requested to check that it is a software rasterizer: {e}")
    });
    let info = adapter.get_info();

    decide_software_tol(tol, info.device_type, &info.name).unwrap_or_else(|e| panic!("{e}"))
}

/// Whether an adapter is a software rasterizer: the CPU device type, or a name that says so.
pub fn is_software_adapter(device_type: wgpu::DeviceType, name: &str) -> bool {
    let name = name.to_lowercase();

    device_type == wgpu::DeviceType::Cpu
        || ["llvmpipe", "swiftshader", "softpipe", "lavapipe"]
            .iter()
            .any(|n| name.contains(n))
}

/// `tol` as the run may use it: the variable is for a software adapter only. On a hardware adapter it is an error, so that
/// a run on the discrete GPU cannot hide a real difference behind the software limit.
pub fn decide_software_tol(
    tol: Option<f64>,
    device_type: wgpu::DeviceType,
    name: &str,
) -> Result<Option<f64>, String> {
    match tol {
        Some(t) if !is_software_adapter(device_type, name) => Err(format!(
            "X3D_TEST_SOFTWARE_TOL={t:e} is set, but the adapter {name:?} ({device_type:?}) is not a software rasterizer: unset it for a hardware run"
        )),
        other => Ok(other),
    }
}

/// The value of `X3D_TEST_SOFTWARE_TOL`: `None` when unset, an error when it is not a finite number above zero. A test calls
/// [`software_tol`] once at its start, so that a bad setting fails the run even when every compared value matches strictly
/// (a lazy read behind `within_ulps(..) || ..` would never run then).
pub fn parse_software_tol(raw: Option<&str>) -> Result<Option<f64>, String> {
    let Some(v) = raw else {
        return Ok(None);
    };

    v.trim()
        .parse::<f64>()
        .ok()
        .filter(|t| t.is_finite() && *t > 0.0)
        .map(Some)
        .ok_or_else(|| format!("X3D_TEST_SOFTWARE_TOL={v:?}: expected a positive number"))
}

/// `within_ulps` with an absolute tolerance on numbers: every number of `a` and `b` is within `tol`. `seen` takes the
/// largest difference found.
pub fn within_abs(a: &Value, b: &Value, tol: f64, seen: &mut f64) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            // A number that is not representable as f64 cannot be compared: it does not match (the failure text shows it).
            let (Some(x), Some(y)) = (x.as_f64(), y.as_f64()) else {
                return false;
            };
            let d = (x - y).abs();

            *seen = seen.max(d);
            d <= tol
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| within_abs(p, q, tol, seen))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, p)| y.get(k).is_some_and(|q| within_abs(p, q, tol, seen)))
        }
        _ => a == b,
    }
}
