//! REA batch 08 lane net stepped cases against pinned X_ITE (oracle/rea-b08-net-baseline, made by
//! `oracle/tools/rea_b08_net.py case`): DISEntityManager, DISEntityTypeMapping, ReceiverPdu, SignalPdu, TransmitterPdu,
//! InlineGeometry and FontLibrary (worlds: oracle/synthetic-rea-b08-net).
//!
//! Each case of oracle/rea-b08-net-baseline/cases.txt (`name world script msaa [text_tol] [--udp] [--known-diff=KEY]`) runs
//! `x3d-drive` on the world and its event script and compares the result with the oracle's
//! `js.json` like oracle/tools/x10_compare.py: scene text exact (or, with `text_tol`, the same
//! text with every number within it: absolute below magnitude 1, relative above), poses within
//! 1e-4, the sorted log exact, captured frames with at most 0.5% of the pixels more than 8 off.
//! Without a GPU adapter the image part is skipped. A `--known-diff=KEY` case leaves the picture regions of a decided
//! difference (`known_diff`) out of that comparison and checks the difference itself there: X_ITE's region stays the same in
//! every captured frame, Rust's changes (ig-second, ADR 0012: Rust keeps the routes of a second InlineGeometry).
//!
//! `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const SENDER_PORTS: [u16; 2] = [41234, 41235];
const LISTEN_PORT: u16 = 41236;
/// The period of one round of the UDP sender (a datagram to each of two ports).
const ROUND: std::time::Duration = std::time::Duration::from_millis(12);

struct UdpVerdict {
    control: bool,
    received: usize,
    /// (port, datagrams sent, sends refused)
    ports: Vec<(u16, usize, usize)>,
}

struct Udp {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: std::thread::JoinHandle<UdpVerdict>,
}

impl Udp {
    fn start() -> Udp {
        Udp::start_on(SENDER_PORTS, LISTEN_PORT)
    }

    fn start_on(sender_ports: [u16; 2], listen_port: u16) -> Udp {
        use std::net::UdpSocket;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        // Positive control first: a bound socket receives what the sender sends.
        let control = {
            let b = UdpSocket::bind(("127.0.0.1", sender_ports[0])).unwrap();
            b.set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let c = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
            c.send_to(&[7u8; 144], ("127.0.0.1", sender_ports[0]))
                .unwrap();
            let mut buf = [0u8; 2048];
            matches!(b.recv_from(&mut buf), Ok((144, _)))
        };
        let listener = UdpSocket::bind(("127.0.0.1", listen_port)).unwrap();
        // Never block: a round that waits for the listener is a round the sender does not make. A short drive (a fast
        // machine) then gets a few rounds, and the floor of the check below cannot be met (physical Mac: 9 rounds).
        // The rounds are paced by a fixed period instead (ROUND), not run as fast as the sleep allows: macOS limits
        // the ICMP port unreachable replies to 250 a second, and a sender above that sees sends that no refusal
        // follows (the check wants half of the sends refused). Two ports at 12 ms make 167 a second.
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            // DIS-PDU shaped datagrams: version 6, exercise 1, type, family, timestamp, length.
            let pdu = |kind: u8, size: usize| {
                let mut d = vec![
                    6u8,
                    1,
                    kind,
                    1,
                    0,
                    0,
                    0,
                    0,
                    (size >> 8) as u8,
                    size as u8,
                    0,
                    0,
                ];
                d.extend((0..size - 12).map(|i| i as u8));
                d
            };
            let senders: Vec<UdpSocket> = sender_ports
                .iter()
                .map(|p| {
                    let s = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
                    s.connect(("127.0.0.1", *p)).unwrap();
                    s
                })
                .collect();
            let mut sent = [0usize; 2];
            let mut refused = [0usize; 2];
            let mut received = 0;
            let mut buf = [0u8; 4096];
            let mut round = 0u8;

            let mut next = std::time::Instant::now();

            while !flag.load(Ordering::Relaxed) {
                for (i, s) in senders.iter().enumerate() {
                    let kind = [1u8, 25, 26, 27][(round as usize + i) % 4];

                    // An ICMP port unreachable of an earlier datagram shows as ConnectionRefused on a later send.
                    match s.send(&pdu(kind, 144 + (round as usize % 4) * 16)) {
                        Ok(_) => sent[i] += 1,
                        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                            refused[i] += 1
                        }
                        Err(_) => {}
                    }
                }

                while let Ok(n) = listener.recv(&mut buf) {
                    received += n;
                }

                round = round.wrapping_add(1);
                // A tick every ROUND, not a sleep of ROUND after the work: a sleep that overshoots (the hosted
                // macOS machine rounds 1 ms up to 9 ms) is made up for by the next tick.
                next += ROUND;
                std::thread::sleep(next.saturating_duration_since(std::time::Instant::now()));
            }

            UdpVerdict {
                control,
                received,
                ports: sender_ports
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (*p, sent[i], refused[i]))
                    .collect(),
            }
        });

        Udp { stop, handle }
    }

    fn finish(self) -> UdpVerdict {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        self.handle.join().unwrap()
    }
}

fn quat(r: &[f64]) -> [f64; 4] {
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let n = if n == 0.0 { 1.0 } else { n };
    let s = (r[3] / 2.0).sin() / n;
    [r[0] * s, r[1] * s, r[2] * s, (r[3] / 2.0).cos()]
}

/// x10_compare.py text_equal: identical, or identical after replacing every number and the
/// numbers within `tol`.
fn text_equal(a: &str, b: &str, tol: f64) -> bool {
    if a == b {
        return true;
    }

    if tol == 0.0 {
        return false;
    }

    fn split(s: &str) -> (String, Vec<f64>) {
        let (mut text, mut numbers) = (String::new(), vec![]);
        let bytes = s.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            let start = i;
            let digit = |j: usize| j < bytes.len() && bytes[j].is_ascii_digit();

            if digit(i) || (bytes[i] == b'-' && digit(i + 1)) {
                i += 1;
                while digit(i) {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'.' && digit(i + 1) {
                    i += 1;
                    while digit(i) {
                        i += 1;
                    }
                }
                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    let mut j = i + 1;
                    if j < bytes.len() && (bytes[j] == b'-' || bytes[j] == b'+') {
                        j += 1;
                    }
                    if digit(j) {
                        i = j;
                        while digit(i) {
                            i += 1;
                        }
                    }
                }
                numbers.push(s[start..i].parse().unwrap_or(f64::NAN));
                text.push('#');
            } else {
                let c = s[i..].chars().next().unwrap();
                text.push(c);
                i += c.len_utf8();
            }
        }

        (text, numbers)
    }

    let ((ta, na), (tb, nb)) = (split(a), split(b));

    ta == tb
        && na
            .iter()
            .zip(&nb)
            .all(|(x, y)| (x - y).abs() / x.abs().max(1.0) <= tol)
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

/// A decided difference of a case (`--known-diff=KEY` in cases.txt): the picture region where X_ITE and Rust differ by
/// decision, as `(x0, y0, x1, y1)` in pixels from the top left, and why.
fn known_diff(key: &str) -> (&'static [(u32, u32, u32, u32)], &'static str) {
    match key {
        // ADR 0012 (issue #68): with Cache on, X_ITE 16.5.0 gives a second InlineGeometry of the same url a copy() of the
        // geometry without its routes ("TODO: add routes", InlineGeometry.js setInternalScene). Rust keeps the routes. The
        // Extrusion row (routes to EX.set_spine) shows it: X_ITE's second and third copies stay still, Rust's move. The
        // Coordinate row moves in both, because the copy shares the routed Coordinate node.
        "ig-second" => (
            &[(170, 256, 512, 512)],
            "ADR 0012: the second and third InlineGeometry of routes-extrusion.x3dv keep their routes in Rust",
        ),
        _ => panic!("unknown --known-diff={key}"),
    }
}

/// The pixels of `px` (RGBA, width `w`) inside `r`.
fn region(px: &[u8], w: u32, r: (u32, u32, u32, u32)) -> Vec<u8> {
    let (x0, y0, x1, y1) = r;
    (y0..y1)
        .flat_map(|y| px[((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize].to_vec())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn compare_case(
    root: &Path,
    name: &str,
    world: &str,
    script: &str,
    msaa: u32,
    text_tol: f64,
    udp_on: bool,
    gpu: bool,
    known: Option<&str>,
    failures: &mut Vec<String>,
) {
    let dir = root.join(format!("oracle/rea-b08-net-baseline/{name}-msaa{msaa}"));
    let js: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("js.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "rea-b08-net-{}-{name}-msaa{msaa}",
        std::process::id()
    ));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let png_dir = out.join("rs");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-drive"));
    cmd.current_dir(root)
        .args([world, script, "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(dir.join("js.json"));

    let images = gpu && dir.join("js").is_dir();

    if images {
        cmd.arg("--png-dir").arg(&png_dir);
    }

    if msaa > 0 {
        cmd.args(["--msaa", &msaa.to_string(), "--gl-samples"]);
    }

    // `--udp` cases: a loopback sender feeds the ports the reader nodes name, a listener sits on the port of the
    // writer nodes (127.0.0.1 only). Connected UDP sockets get ECONNREFUSED when nothing listens, and a node that
    // listened or wrote would show as missing refusals or received datagrams.
    let udp = if udp_on { Some(Udp::start()) } else { None };
    let status = cmd.status().unwrap();
    let mut fail = |what: String| failures.push(format!("{name} MSAA {msaa}: {what}"));

    if let Some(u) = udp {
        let v = u.finish();

        if !v.control {
            fail("udp: the positive control did not receive".into());
        }

        if v.received != 0 {
            fail(format!(
                "udp: a node sent {} datagrams to the listener",
                v.received
            ));
        }

        for (port, sent, refused) in &v.ports {
            // Half of the datagrams sent while the drive ran must have met a closed port: a bound socket refuses none.
            if *sent < 5 || *refused < 5 || *refused * 2 < *sent {
                fail(format!(
                    "udp: port {port} sent {sent} refused {refused}: something may be listening"
                ));
            }
        }
    }

    if !status.success() {
        return fail(format!("x3d-drive exited with {status}"));
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        return fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    for (k, a) in js["vrml"].as_object().unwrap() {
        let equal = match (a.as_str(), rs["vrml"].get(k).and_then(Value::as_str)) {
            (Some(a), Some(b)) => text_equal(a, b, text_tol),
            _ => rs["vrml"].get(k) == Some(a),
        };

        if !equal {
            fail(format!("scene text differs at frame {k}"));
        }
    }

    if js.get("log") != rs.get("log") {
        fail(format!("log differs: {} / {}", js["log"], rs["log"]));
    }

    for (k, a) in js["poses"].as_object().unwrap() {
        let Some(b) = rs["poses"].get(k) else {
            fail(format!("pose at frame {k} missing"));
            continue;
        };
        let d = |key: &str| {
            nums(&a[key])
                .iter()
                .zip(nums(&b[key]))
                .map(|(p, q)| (p - q).abs())
                .fold(0.0, f64::max)
        };
        let (qa, qb) = (
            quat(&nums(&a["orientation"])),
            quat(&nums(&b["orientation"])),
        );
        let dot = qa.iter().zip(qb).map(|(p, q)| p * q).sum::<f64>().abs();
        let angle = 2.0 * dot.min(1.0).acos();
        let fov = (a["fieldOfViewScale"].as_f64().unwrap_or(1.0)
            - b["fieldOfViewScale"].as_f64().unwrap_or(1.0))
        .abs();
        let viewpoint = a["viewpoint"] == b["viewpoint"] || b["viewpoint"] == "<default>";

        if d("position") > 1e-4
            || d("centerOfRotation") > 1e-4
            || angle > 1e-4
            || fov > 1e-4
            || a["viewer"] != b["viewer"]
            || !viewpoint
        {
            fail(format!(
                "pose frame {k}: dpos {:.2e} dcenter {:.2e} dangle {angle:.2e} dfov {fov:.2e} viewer {}/{}",
                d("position"),
                d("centerOfRotation"),
                a["viewer"],
                b["viewer"]
            ));
        }
    }

    let (masked, why) = known.map_or((&[][..], ""), known_diff);
    // The masked regions of each picture, X_ITE and Rust, in frame order: the difference itself is checked below.
    let mut regions: Vec<(String, Vec<Vec<u8>>, Vec<Vec<u8>>)> = vec![];

    if images {
        let mut files: Vec<_> = std::fs::read_dir(dir.join("js"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort_by_key(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.parse::<u32>().ok())
        });

        for p in files {
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            let (w, h, want) = x3d_render::compare::load_png(&p).unwrap();

            match x3d_render::compare::load_png(&png_dir.join(&file)) {
                Ok((rw, rh, mut got)) if (rw, rh) == (w, h) => {
                    if let Some(r) = masked
                        .iter()
                        .find(|r| r.2 > w || r.3 > h || r.0 >= r.2 || r.1 >= r.3)
                    {
                        fail(format!(
                            "{file}: known-difference region {r:?} is not inside the {w}x{h} picture"
                        ));
                        continue;
                    }
                    for (i, &r) in masked.iter().enumerate() {
                        if regions.len() <= i {
                            regions.push((format!("{r:?}"), vec![], vec![]));
                        }
                        regions[i].1.push(region(&want, w, r));
                        regions[i].2.push(region(&got, w, r));
                        let (x0, y0, x1, y1) = r;
                        for y in y0..y1 {
                            let row = ((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize;
                            got[row.clone()].copy_from_slice(&want[row]);
                        }
                    }
                    let s = x3d_render::compare::compare(w, h, &want, &got, 8);

                    if s.pixels_over * 1000 > (w * h) as usize * 5 {
                        fail(format!(
                            "{file}: {} pixels over 8 ({:.3}%), max {}",
                            s.pixels_over, s.pct_over, s.max_delta
                        ));
                    }
                }
                Ok((rw, rh, _)) => fail(format!("{file}: size {rw}x{rh}, X_ITE {w}x{h}")),
                Err(e) => fail(format!("{file}: {e}")),
            }
        }
    }

    // The decided difference must be there, else the mask hides nothing and must go: X_ITE's region is the same in
    // every picture, Rust's is not, and the two are not equal.
    for (r, js_px, rs_px) in &regions {
        if js_px.windows(2).any(|p| p[0] != p[1]) {
            fail(format!(
                "known difference {}: the X_ITE region {r} changes between pictures ({why})",
                known.unwrap()
            ));
        }
        if rs_px.windows(2).all(|p| p[0] == p[1]) {
            fail(format!(
                "known difference {}: the Rust region {r} does not change between pictures ({why})",
                known.unwrap()
            ));
        }
        // And the two differ there: else the region holds no difference to leave out.
        if js_px.iter().zip(rs_px).all(|(a, b)| a == b) {
            fail(format!(
                "known difference {}: the X_ITE and Rust regions {r} are equal in every picture ({why})",
                known.unwrap()
            ));
        }
    }

    if !masked.is_empty() && images && regions.is_empty() {
        fail(format!(
            "known difference {}: no picture was compared",
            known.unwrap()
        ));
    }

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn rea_batch08_net_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-b08-net-baseline/cases.txt")).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    // One GPU at a time: the cases run in order in this test.
    let gpu = x3d_render::gpu::Gpu::new().is_ok();
    if !gpu {
        x3d_evidence::no_gpu("image comparisons skipped");
    }
    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3].parse::<u32>().unwrap());
        let udp_on = f.contains(&"--udp");
        let known = f.iter().find_map(|a| a.strip_prefix("--known-diff="));
        let text_tol = f
            .get(4)
            .filter(|t| !t.starts_with("--"))
            .map_or(0.0, |t| t.parse::<f64>().unwrap());

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        compare_case(
            &root,
            name,
            world,
            script,
            msaa,
            text_tol,
            udp_on,
            gpu,
            known,
            &mut failures,
        );
        checked += 1;
    }

    eprintln!("{checked} cases checked (images: {gpu})");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The sender of the `--udp` cases must make many rounds in a short drive: the check of `compare_case` wants at least
/// 5 refusals per port, and a refusal needs an earlier datagram, so a drive that gets under 10 rounds can never pass.
/// A listener read that waited for a timeout made a round take 20 ms or more; on a physical Mac (Apple M1) a drive
/// lasted for 9 rounds ("sent 5, refused 4"). This runs on every platform and needs no GPU, no answer key and no
/// port of the cases (free ports are asked of the system).
#[test]
fn the_udp_sender_makes_many_rounds_in_a_short_drive() {
    use std::net::UdpSocket;

    // Three ports that are distinct: all three sockets stay bound until every port is read, so no bind can be handed a port
    // another one just released (they are released together, right before the sender binds them).
    let held: Vec<UdpSocket> = (0..3)
        .map(|_| UdpSocket::bind(("127.0.0.1", 0)).unwrap())
        .collect();
    let ports: Vec<u16> = held
        .iter()
        .map(|s| s.local_addr().unwrap().port())
        .collect();
    drop(held);
    let udp = Udp::start_on([ports[0], ports[1]], ports[2]);

    // A drive of 400 ms: the old sender (a 20 ms wait in every round) made 19 rounds or fewer in it (9 in the 190 ms of
    // the Mac drive); a round every 12 ms makes 33.
    std::thread::sleep(std::time::Duration::from_millis(400));

    let v = udp.finish();

    assert!(v.control, "the positive control did not receive");
    assert_eq!(v.received, 0, "nothing sends to the listener");

    for (port, sent, refused) in &v.ports {
        assert!(
            sent + refused >= 25,
            "port {port}: {sent} sent and {refused} refused in 400 ms; a round must not wait"
        );
        // The share of refusals is not checked here. A connected UDP socket keeps one pending error, so ICMP replies that
        // arrive between two sends make one refusal; on a loaded hosted macOS machine the job read "sent 32 refused 11" in
        // 43 rounds. The share is a property of the OS, not of the sender; `compare_case` checks it for the real cases.
        assert!(
            *refused >= 5,
            "port {port}: sent {sent} refused {refused}; a closed port must refuse"
        );
    }
}
