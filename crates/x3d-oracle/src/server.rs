//! The static file server of the Node tools: serves ROOT (oracle/) on 127.0.0.1 with `Cache-Control: no-store`,
//! a thread per connection (ES module fan-out opens many parallel connections), 403 when the request path is
//! outside ROOT, 404 for anything unreadable, and the same Content-Type table (`model/vrml` for everything else).
//!
//! The ROOT check is lexical: it compares path components after normalization, before any file access.
//! Link targets are not confined. A link below ROOT is followed, also to a target outside ROOT, as Node
//! follows it (the X_ITE pin reaches node_modules through one). Only the owner of ROOT can place a link.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::thread;

pub struct Server {
    pub port: u16,
}

pub fn content_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "html" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "model/vrml",
    }
}

/// Lexical `path.normalize (path.join (a, b))` for absolute `a`.
pub fn join_normalize(a: &Path, b: &str) -> PathBuf {
    let mut out = PathBuf::from("/");
    for c in a.components().chain(Path::new(b).components()) {
        match c {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s),
        }
    }
    #[cfg(windows)]
    {
        let mut w = PathBuf::new();
        for c in a.components() {
            if let Component::Prefix(_) = c {
                w.push(c.as_os_str());
            }
        }
        if !w.as_os_str().is_empty() {
            return w.join(out.strip_prefix("/").unwrap_or(&out));
        }
    }
    out
}

/// The URL path as `new URL (req.url, "http://x").pathname` sees it (dot segments removed).
fn url_pathname(target: &str) -> String {
    let p = target.split(['?', '#']).next().unwrap_or("/");
    let mut segs: Vec<&str> = Vec::new();
    let parts: Vec<&str> = p.split('/').collect();
    for (i, s) in parts.iter().enumerate().skip(1) {
        let last = i == parts.len() - 1;
        match *s {
            "." | "%2e" | "%2E" => {
                if last {
                    segs.push("");
                }
            }
            ".." | ".%2e" | ".%2E" | "%2e." | "%2E." | "%2e%2e" | "%2E%2E" | "%2e%2E"
            | "%2E%2e" => {
                segs.pop();
                if last {
                    segs.push("");
                }
            }
            s => segs.push(s),
        }
    }
    format!("/{}", segs.join("/"))
}

fn respond(stream: &mut TcpStream, root: &Path, method: &str, target: &str) -> std::io::Result<()> {
    let decoded = crate::json::decode_uri_component(&url_pathname(target));
    let (status, ctype, body): (&str, Option<&str>, Vec<u8>) = match decoded {
        None => ("400 Bad Request", None, Vec::new()),
        Some(p) => {
            let path = join_normalize(root, &p);
            if !path.starts_with(root) {
                ("403 Forbidden", None, Vec::new())
            } else {
                match std::fs::read(&path) {
                    Ok(body) => ("200 OK", Some(content_type(&path)), body),
                    Err(_) => ("404 Not Found", None, Vec::new()),
                }
            }
        }
    };
    let mut head = format!("HTTP/1.1 {status}\r\n");
    if let Some(t) = ctype {
        head.push_str(&format!("Content-Type: {t}\r\n"));
    }
    // The 403 of the Node server has no Cache-Control; 200 and 404 have no-store.
    if !status.starts_with("403") {
        head.push_str("Cache-Control: no-store\r\n");
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: keep-alive\r\n\r\n",
        body.len()
    ));
    stream.write_all(head.as_bytes())?;
    if method != "HEAD" {
        stream.write_all(&body)?;
    }
    stream.flush()
}

fn serve(stream: TcpStream, root: PathBuf) {
    let _ = stream.set_nodelay(true);
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut stream = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = line.split_whitespace();
        let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
            return;
        };
        let (method, target) = (method.to_string(), target.to_string());
        let mut length = 0usize;
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).unwrap_or(0) == 0 {
                return;
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':')
                && k.eq_ignore_ascii_case("content-length")
            {
                length = v.trim().parse().unwrap_or(0);
            }
        }
        if length > 0 {
            let mut skip = vec![0u8; length];
            if reader.read_exact(&mut skip).is_err() {
                return;
            }
        }
        if respond(&mut stream, &root, &method, &target).is_err() {
            return;
        }
    }
}

impl Server {
    /// Listens on 127.0.0.1:0; the accept loop runs until the process exits.
    pub fn start(root: PathBuf) -> std::io::Result<Server> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        thread::Builder::new().name("http".into()).spawn(move || {
            for stream in listener.incoming().flatten() {
                let root = root.clone();
                let _ = thread::Builder::new()
                    .name("http-conn".into())
                    .spawn(move || serve(stream, root));
            }
        })?;
        Ok(Server { port })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathname_and_join() {
        assert_eq!(url_pathname("/a/./b/../c.wrl?x=1"), "/a/c.wrl");
        assert_eq!(url_pathname("/../../etc/passwd"), "/etc/passwd");
        assert_eq!(
            join_normalize(Path::new("/r/oracle"), "/x/../y.wrl"),
            PathBuf::from("/r/oracle/y.wrl")
        );
        assert_eq!(
            join_normalize(Path::new("/r/oracle"), "/../../z"),
            PathBuf::from("/z")
        );
        assert_eq!(content_type(Path::new("a.WRL")), "model/vrml");
        assert_eq!(content_type(Path::new("a.mjs")), "text/javascript");
    }

    #[test]
    fn serves_files_with_no_store() {
        let dir = std::env::temp_dir().join(format!("x3d-oracle-srv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a b.wrl"), b"#VRML V2.0 utf8\n").unwrap();
        let s = Server::start(dir.clone()).unwrap();
        let get = |path: &str| {
            let mut c = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
            write!(
                c,
                "GET {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            let mut r = BufReader::new(c);
            let mut head = String::new();
            loop {
                let mut l = String::new();
                r.read_line(&mut l).unwrap();
                if l == "\r\n" {
                    break;
                }
                head.push_str(&l);
            }
            let len: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("Content-Length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap();
            (head, body)
        };
        let (h, b) = get("/a%20b.wrl");
        assert!(h.starts_with("HTTP/1.1 200"), "{h}");
        assert!(h.contains("Content-Type: model/vrml") && h.contains("Cache-Control: no-store"));
        assert_eq!(b, b"#VRML V2.0 utf8\n");
        assert!(get("/missing.wrl").0.starts_with("HTTP/1.1 404"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    /// A status line for each target, from a server on its own canonical root (as `main` passes it).
    fn statuses(root: &Path, targets: &[&str]) -> Vec<String> {
        let s = Server::start(root.to_path_buf()).unwrap();
        targets
            .iter()
            .map(|t| {
                let mut c = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
                write!(
                    c,
                    "GET {t} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
                let mut l = String::new();
                BufReader::new(c).read_line(&mut l).unwrap();
                l.trim_end().to_string()
            })
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn stays_inside_root() {
        // Own directories: serves_files_with_no_store runs in parallel and removes x3d-oracle-srv-<pid>.
        let base = std::env::temp_dir().join(format!("x3d-oracle-srv-trav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        let sibling = base.join("root_secret");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(root.join("in.wrl"), b"in").unwrap();
        std::fs::write(root.join("sub/a.wrl"), b"a").unwrap();
        std::fs::write(sibling.join("secret.txt"), b"secret").unwrap();
        std::os::unix::fs::symlink(&sibling, root.join("link")).unwrap();
        let root = std::fs::canonicalize(&root).unwrap();
        let ok = "HTTP/1.1 200 OK";
        let forbidden = "HTTP/1.1 403 Forbidden";
        let missing = "HTTP/1.1 404 Not Found";
        let cases: &[(&str, &str)] = &[
            ("/in.wrl", ok),
            ("/sub/a.wrl", ok),
            ("/sub/../in.wrl", ok),
            ("/missing.wrl", missing),
            // The sibling shares the root's string prefix; the old string check served it.
            ("/%2e%2e%2froot_secret/secret.txt", forbidden),
            ("/sub/%2e%2e%2f%2e%2e%2froot_secret/secret.txt", forbidden),
            ("/%2E%2E%2F%2E%2E%2F%2E%2E%2Fetc/passwd", forbidden),
            // Dot segments in the URL are removed before decoding, as `new URL` does.
            ("/../root_secret/secret.txt", missing),
            ("/%2e%2e/root_secret/secret.txt", missing),
            // Double encoding decodes once, to a literal file name.
            ("/%252e%252e%252froot_secret/secret.txt", missing),
            // Absolute paths join below the root.
            ("//etc/passwd", missing),
            ("/%2fetc%2fpasswd", missing),
            // On Linux a backslash is a file-name byte, not a separator.
            ("/..%5croot_secret%5csecret.txt", missing),
            ("/%2e%2e%5croot_secret%5csecret.txt", missing),
            // Normalization is lexical and comes before any file access, so `..` after a link
            // stays below the root.
            ("/link/%2e%2e%2f%2e%2e%2froot_secret/secret.txt", forbidden),
            ("/link/%2e%2e%2fin.wrl", ok),
            ("/%zz", "HTTP/1.1 400 Bad Request"),
        ];
        let targets: Vec<&str> = cases.iter().map(|c| c.0).collect();
        let got = statuses(&root, &targets);
        for ((target, want), got) in cases.iter().zip(&got) {
            assert_eq!(got, want, "{target}");
        }
        std::fs::remove_dir_all(&base).ok();
    }

    #[cfg(unix)]
    #[test]
    fn follows_symlinks_below_root_like_node() {
        // A link placed below the root is served, as Node's fs.readFile serves it: the X_ITE pin
        // reaches its node_modules through such a link. Only the owner of the root can create one;
        // a request cannot.
        let base = std::env::temp_dir().join(format!("x3d-oracle-srv-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        let outside = base.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("m.js"), b"m").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("node_modules")).unwrap();
        let root = std::fs::canonicalize(&root).unwrap();
        assert_eq!(
            statuses(&root, &["/node_modules/m.js"]),
            ["HTTP/1.1 200 OK"]
        );
        std::fs::remove_dir_all(&base).ok();
    }
}
