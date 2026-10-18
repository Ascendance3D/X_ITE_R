//! btBoxBoxDetector against Ammo.js (Bullet 2.82, asm.js) on inputs that exposed a defect in
//! intersectRectQuad2: a corner of the incident face inside the reference face by less than a float
//! ulp is stored, and the crossing test against the next corner then sees it at its float value,
//! outside, and adds a second point. The data is in tests/data/bullet_box_box/.

use x3d_runtime::physics::bt::boxbox::box_box;

fn nums(line: &str) -> Vec<f64> {
    line.split_whitespace().skip(1).map(|t| t.parse().unwrap()).collect()
}

fn m3(v: &[f64]) -> [[f64; 3]; 3] {
    [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]]
}

fn v3(v: &[f64]) -> [f64; 3] {
    [v[0], v[1], v[2]]
}

#[test]
fn box_box_aligned_stack_n4_f120_frame16_substep5() {
    let path = format!("{}/tests/data/bullet_box_box/aligned_n4_f120_f16_s5.txt", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut input = std::collections::BTreeMap::new();
    let mut want: Vec<Vec<f64>> = vec![];

    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let key = line.split_whitespace().next().unwrap();

        if key == "out" {
            want.push(nums(line));
        } else {
            input.insert(key, nums(line));
        }
    }

    let got = box_box(v3(&input["p1"]), &m3(&input["r1"]), v3(&input["s1"]), v3(&input["p2"]), &m3(&input["r2"]), v3(&input["s2"]));

    assert_eq!(got.len(), want.len(), "contact count: got {got:?}");

    for (k, ((n, p, d), w)) in got.iter().zip(&want).enumerate() {
        let g: Vec<f64> = n.iter().chain(p.iter()).chain(std::iter::once(d)).copied().collect();
        assert_eq!(g, *w, "contact {k}");
    }
}
