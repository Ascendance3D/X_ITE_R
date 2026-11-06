//! The depth order X_ITE_R displays equals the order the pinned X_ITE 16.5.0's sort worker answers (lane gs/worker-timing).
//!
//! `oracle/x_ite-splat-sort-timing.mjs` recorded, for a synthetic cloud of N splats (the generator is `world(n)` of that
//! tool, written again below) and a fixed camera path, the model-view matrix of every "sort" request that X_ITE posted and
//! the FNV-1a checksum of the index array of the answer. The checksums do not depend on timing. This test sorts the same
//! positions with the same matrices by `sort_indices` and compares the checksums, so the order is compared apart from the
//! time the worker needs. File: oracle/gs-sort-order-baseline/order.json.

use serde_json::Value;
use x3d_render::splats::{SortScratch, sort_indices};

/// The generator of `world(n)`: a linear congruential sequence in JavaScript double arithmetic.
struct Lcg(i32);

impl Lcg {
    fn next(&mut self) -> f64 {
        // (s * 1103515245 + 12345) & 0x7fffffff: the product is rounded to a double, then taken as a 32 bit integer.
        let v = f64::from(self.0) * 1103515245.0 + 12345.0;
        self.0 = ((v as u64) as u32 as i32) & 0x7fffffff;
        f64::from(self.0) / 2147483647.0
    }
}

fn positions(n: usize) -> Vec<f32> {
    let mut r = Lcg(12345);
    let mut out = Vec::with_capacity(n * 3);

    for _ in 0..n {
        for _ in 0..3 {
            let text = format!("{:.3}", (r.next() - 0.5) * 3.0);
            out.push(text.parse::<f32>().unwrap());
        }

        // scale, opacity, three spherical harmonic coefficients: eight draws of the generator per splat in all.
        for _ in 0..5 {
            r.next();
        }
    }

    out
}

fn fnv(a: &[u32]) -> u32 {
    let mut h: u32 = 0x811c9dc5;

    for &v in a {
        h ^= v;
        h = h.wrapping_mul(0x01000193);
    }

    h
}

#[test]
fn the_displayed_depth_order_is_the_order_of_the_pinned_sort_worker() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/gs-sort-order-baseline/order.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no order recording {}: {e}", path.display()));
    let json: Value = serde_json::from_str(&text).unwrap();

    assert_eq!(json["pinned"], "34cdf9f97");

    let orders = json["orders"].as_object().unwrap();

    for want in ["2", "441", "3969", "30000", "100000", "300000"] {
        assert!(
            orders.contains_key(want),
            "no order recording for {want} splats"
        );
    }

    let mut scratch = SortScratch::default();

    for (n, order) in orders {
        let count: usize = n.parse().unwrap();
        let pos = positions(count);
        let matrices = order["matrices"].as_array().unwrap();
        let hashes = order["sortedHashes"].as_array().unwrap();

        assert_eq!(matrices.len(), hashes.len(), "{n}: one answer per request");
        assert!(
            matrices.len() >= 4,
            "{n}: the initial sort and three camera steps"
        );

        for (k, (m, want)) in matrices.iter().zip(hashes).enumerate() {
            let mut mv = [0f32; 16];

            for (d, s) in mv.iter_mut().zip(m.as_array().unwrap()) {
                *d = s.as_f64().unwrap() as f32;
            }

            let started = std::time::Instant::now();
            let got = sort_indices(&pos, count, &mv, &mut scratch);
            // Information only (a debug build is slower than a release build): the time of the in-frame sort.
            eprintln!(
                "{n} splats, request {k}: sort_indices {:.2} ms",
                started.elapsed().as_secs_f64() * 1000.0
            );

            assert_eq!(
                got.len() as u64,
                want["len"].as_u64().unwrap(),
                "{n} splats, request {k}: length"
            );
            assert_eq!(
                u64::from(fnv(&got)),
                want["h"].as_u64().unwrap(),
                "{n} splats, request {k}: the displayed order differs from the pinned worker's answer"
            );
        }
    }
}
