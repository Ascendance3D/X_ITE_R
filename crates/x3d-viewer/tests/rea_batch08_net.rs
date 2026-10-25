//! REA batch 08 lane net stepped cases against pinned X_ITE (oracle/rea-batch08-net-baseline, made by
//! `oracle/tools/rea_b08_net.py case`): DISEntityManager, DISEntityTypeMapping, ReceiverPdu, SignalPdu, TransmitterPdu,
//! InlineGeometry and FontLibrary (worlds: oracle/synthetic-rea-b08-net).
//!
//! Each case of oracle/rea-batch08-net-baseline/cases.txt (`name world script msaa [text_tol]`) runs
//! `x3d-drive` on the world and its event script and compares the result with the oracle's
//! `js.json` like oracle/tools/x10_compare.py: scene text exact (or, with `text_tol`, the same
//! text with every number within it: absolute below magnitude 1, relative above), poses within
//! 1e-4, the sorted log exact, captured frames with at most 0.5% of the pixels more than 8 off.
//! Without a GPU adapter the image part is skipped.
//!
//! `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const SENDER_PORTS: [u16; 2] = [41234, 41235];
const LISTEN_PORT: u16 = 41236;

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
        use std::net::UdpSocket;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        // Positive control first: a bound socket receives what the sender sends.
        let control = {
            let b = UdpSocket::bind(("127.0.0.1", SENDER_PORTS[0])).unwrap();
            b.set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            let c = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
            c.send_to(&[7u8; 144], ("127.0.0.1", SENDER_PORTS[0]))
                .unwrap();
            let mut buf = [0u8; 2048];
            matches!(b.recv_from(&mut buf), Ok((144, _)))
        };
        let listener = UdpSocket::bind(("127.0.0.1", LISTEN_PORT)).unwrap();
        listener
            .set_read_timeout(Some(std::time::Duration::from_millis(20)))
            .unwrap();
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
            let senders: Vec<UdpSocket> = SENDER_PORTS
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
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            UdpVerdict {
                control,
                received,
                ports: SENDER_PORTS
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
    failures: &mut Vec<String>,
) {
    let dir = root.join(format!("oracle/rea-batch08-net-baseline/{name}-msaa{msaa}"));
    let js: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("js.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "rea-batch08-net-{}-{name}-msaa{msaa}",
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

    if images {
        for entry in std::fs::read_dir(dir.join("js")).unwrap() {
            let p = entry.unwrap().path();
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            let (w, h, want) = x3d_render::compare::load_png(&p).unwrap();

            match x3d_render::compare::load_png(&png_dir.join(&file)) {
                Ok((rw, rh, got)) if (rw, rh) == (w, h) => {
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

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn rea_batch08_net_cases_match_x_ite() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases =
        std::fs::read_to_string(root.join("oracle/rea-batch08-net-baseline/cases.txt")).unwrap();
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
