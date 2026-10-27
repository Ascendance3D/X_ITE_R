//! GLB2Parser.js and the entry points of the glTF parsers.

use super::*;
use std::cell::RefCell;

type FetchFn = dyn Fn(&str, &str) -> Result<Vec<u8>, String>;

thread_local! {
    /// The loader the embedding application installed for the duration of a parse.
    static FETCHER: RefCell<Option<*const FetchFn>> = const { RefCell::new(None) };
}

/// Run `f` with `fetch` as the resource loader of glTF buffers (called with the buffer `uri`
/// and the base URL of the scene).
pub fn with_fetcher<T>(
    fetch: &dyn Fn(&str, &str) -> Result<Vec<u8>, String>,
    f: impl FnOnce() -> T,
) -> T {
    // SAFETY: the pointer is only dereferenced inside `f`, while `fetch` is borrowed.
    let ptr: *const FetchFn = unsafe {
        std::mem::transmute::<&dyn Fn(&str, &str) -> Result<Vec<u8>, String>, &'static FetchFn>(
            fetch,
        )
    };
    let previous = FETCHER.with(|c| c.borrow_mut().replace(ptr));
    let result = f();

    FETCHER.with(|c| *c.borrow_mut() = previous);
    result
}

fn call_fetcher(uri: &str, base: &str) -> Result<Vec<u8>, String> {
    let installed = FETCHER.with(|c| *c.borrow());

    match installed {
        // SAFETY: see `with_fetcher`.
        Some(p) => unsafe { (*p)(uri, base) },
        None => default_fetch(uri, base),
    }
}

/// `data:` URLs and local files.
pub fn default_fetch(uri: &str, base: &str) -> Result<Vec<u8>, String> {
    let trimmed = uri.trim_start();

    // Chromium also reads `DATA:` and `da<TAB>ta:` as data URLs.
    if crate::is_data_url(uri) {
        return crate::decode_data_url(uri);
    }

    // A tab or newline in the scheme is removed, as in the data URL check.
    let has_scheme = trimmed.find(':').is_some_and(|i| {
        let scheme = trimmed[..i]
            .bytes()
            .filter(|b| !matches!(b, b'\t' | b'\n' | b'\r'));

        scheme.clone().next().is_some()
            && scheme
                .into_iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    });

    // `new URL (uri, base)` throws for a malformed http(s) authority.
    for scheme in ["http://", "https://"] {
        if let Some(rest) = trimmed.strip_prefix(scheme) {
            let host = rest.split(['/', '?', '#']).next().unwrap_or("");
            let bracketed = host.starts_with('[');

            if host.is_empty()
                || host.contains(char::is_whitespace)
                || (bracketed && !host.contains(']'))
                || (!bracketed && host.contains(['[', ']']))
            {
                return Err("TypeError: Failed to construct 'URL': Invalid URL".into());
            }
        }
    }

    let path = if has_scheme {
        match trimmed.strip_prefix("file://") {
            Some(p) => std::path::PathBuf::from(percent_decode_string(p)),
            None => return Err("TypeError: Failed to fetch".into()),
        }
    } else {
        let Some(base) = base.strip_prefix("file://") else {
            return Err("TypeError: Failed to fetch".into());
        };
        let base = percent_decode_string(base);
        let dir = std::path::Path::new(&base)
            .parent()
            .unwrap_or(std::path::Path::new("/"))
            .to_path_buf();
        let rel = percent_decode_string(trimmed.split(['?', '#']).next().unwrap_or(""));

        if rel.starts_with('/') {
            std::path::PathBuf::from(rel)
        } else {
            dir.join(rel)
        }
    };

    std::fs::read(path).map_err(|_| "Error: Not Found".to_string())
}

/// GoldenGate: a parsed JSON value that GLTF2Parser accepts, parsed into a new scene.
pub(crate) fn parse_gltf(input: &J, world_url: &str) -> R<Scene> {
    let mut scene = Scene::new(world_url);

    {
        let fetch = |uri: &str, base: &str| call_fetcher(uri, base);
        let mut p = P::new(&mut scene, V::new(input), world_url, &fetch);

        p.root_object()?;
    }

    Ok(scene)
}

/// GLB2Parser.isValid
pub(crate) fn is_valid_glb(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }

    let u32_at = |i: usize| u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);

    u32_at(0) == 0x46546C67 && u32_at(4) == 2 && u32_at(8) as usize == data.len()
}

/// GLB2Parser.parseIntoScene
pub(crate) fn parse_glb(data: &[u8], world_url: &str) -> R<Scene> {
    let mut scene = Scene::new(world_url);

    // chunks ()
    let mut json: Vec<String> = vec![];
    let mut buffers: Vec<Option<Rc<Vec<u8>>>> = vec![];
    let mut i = 12usize;

    while i < data.len() {
        let u32_at = |k: usize| -> R<u32> {
            data.get(k..k + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| {
                    "RangeError: Offset is outside the bounds of the DataView".to_string()
                })
        };

        let length = u32_at(i)? as usize;
        let ty = u32_at(i + 4)?;

        i += 8;

        let end = i.saturating_add(length).min(data.len());
        let slice = &data[i.min(data.len())..end];

        match ty {
            0x4e4f534a => json.push(crate::decode_text(slice)),
            0x004e4942 => buffers.push(Some(Rc::new(slice.to_vec()))),
            _ => {}
        }

        i = i.saturating_add(length);
    }

    for text in &json {
        let Ok(value) = crate::json::json_parse(text) else {
            continue;
        };

        if !is_valid_gltf(&value) {
            continue;
        }

        let fetch = |uri: &str, base: &str| call_fetcher(uri, base);
        let mut p = P::new(&mut scene, V::new(&value), world_url, &fetch);

        p.buffers = buffers.clone();
        p.root_object()?;
    }

    Ok(scene)
}
