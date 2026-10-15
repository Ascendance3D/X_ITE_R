//! FileLoader.js (loadDocument / loadDocumentAsync / loadDocumentError / printError) against a
//! local, read-only file root, plus the X3DUrlObject nodes that only fetch (textures, sounds),
//! Inline and EXTERNPROTO loading.
//!
//! The oracle serves `oracle/` over HTTP and aborts every other request, so:
//! * `http://oracle/<path>` → the file under the root, `Error: Not Found` if missing (404),
//!   `Error: Forbidden` outside the root (403);
//! * every other URL (remote hosts, `file:`, `urn:` …) → `TypeError: Failed to fetch`.

use crate::*;
use std::path::PathBuf;

pub trait Fetcher {
    /// `fetch (url)` + `checkResponse`: the body bytes, or `String (error)` of the rejection.
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String>;
}

/// Serves `<prefix><path>` from `root/<path>` like oracle/x_ite-dump.mjs's server.
pub struct LocalFetcher {
    pub root: PathBuf,
    /// e.g. `http://oracle/`
    pub prefix: String,
}

impl Fetcher for LocalFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let href = url.as_str();

        if !href.starts_with(&self.prefix) {
            return Err("TypeError: Failed to fetch".into());
        }

        // decodeURIComponent (pathname), then path.normalize (join (ROOT, …)).
        let decoded = percent_encoding::percent_decode_str(url.path()).decode_utf8_lossy().into_owned();
        let mut path = self.root.clone();

        for part in decoded.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if !path.pop() || !path.starts_with(&self.root) {
                        return Err("Error: Forbidden".into());
                    }
                }
                p => path.push(p),
            }
        }

        if !path.starts_with(&self.root) {
            return Err("Error: Forbidden".into());
        }

        match std::fs::read(&path) {
            Ok(data) => Ok(data),
            Err(_) => Err("Error: Not Found".into()),
        }
    }
}

/// `decodeURI`: decode escapes except reserved characters; `None` on a malformed sequence.
pub fn decode_uri(s: &str) -> Option<String> {
    const RESERVED: &[u8] = b";/?:@&=+$,#";
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = vec![];
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }

        let hex = |j: usize| -> Option<u8> {
            let h = bytes.get(j..j + 2)?;
            u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
        };

        let b0 = hex(i + 1)?;

        if b0 < 0x80 {
            if RESERVED.contains(&b0) {
                out.extend_from_slice(&bytes[i..i + 3]);
            } else {
                out.push(b0);
            }

            i += 3;
            continue;
        }

        let n = match b0 {
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            _ => return None,
        };

        let mut seq = vec![b0];

        for k in 1..n {
            if bytes.get(i + 3 * k) != Some(&b'%') {
                return None;
            }

            seq.push(hex(i + 3 * k + 1)?);
        }

        std::str::from_utf8(&seq).ok()?;
        out.extend_from_slice(&seq);
        i += 3 * n;
    }

    String::from_utf8(out).ok()
}

/// FileLoader.truncate (string, 120).
fn truncate(s: &str) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();

    if units.len() > 120 { format!("{}…", String::from_utf16_lossy(&units[..120])) } else { s.to_string() }
}

/// What a FileLoader candidate produced.
pub enum Loaded {
    /// `javascript:` / `ecmascript:` / `vrmlscript:` source (no resolved URL).
    Script(String),
    /// Fetched (and gunzipped) bytes with the resolved URL.
    Data(Vec<u8>, url::Url),
}

/// A FileLoader instance: node type name for messages (`""` for the world) and the attempts.
pub struct FileLoader {
    pub type_name: String,
    pub base: String,
    pub data_as_string: bool,
}

impl Runtime {
    /// X3DExecutionContext.getBaseURL of a node's context (the scene's world URL).
    pub fn base_url(&self, node: NodeId) -> String {
        self.scene.world_url_of(self.scene.node(node).ctx).to_string()
    }

    /// FileLoader.loadDocument with its fallback loop. `accept` is the data callback: an `Err`
    /// is a rejection of loadDocumentAsync and moves on to the next URL, like X_ITE.
    /// Returns whether a candidate was accepted.
    pub fn load_document(&mut self, loader: &FileLoader, urls: &[String], mut accept: impl FnMut(&mut Runtime, Loaded) -> Result<(), String>) -> bool {
        let mut attempts = 0usize;

        if urls.is_empty() {
            // loadDocumentError () without an error prints nothing.
            return false;
        }

        for (i, candidate) in urls.iter().enumerate() {
            let remaining = urls.len() - i - 1;
            let mut resolved: Option<url::Url> = None;

            let result: Result<(), String> = (|| {
                if candidate.is_empty() {
                    return Err("Error: Empty URL.".to_string());
                }

                // Script URLs: /^\s*(?:ecmascript|javascript|vrmlscript):/s
                let trimmed = candidate.trim_start();
                for scheme in ["ecmascript:", "javascript:", "vrmlscript:"] {
                    if trimmed.starts_with(scheme) {
                        let source = &trimmed[scheme.len()..];
                        return accept(self, Loaded::Script(source.to_string()));
                    }
                }

                let base = url::Url::parse(&loader.base).map_err(|e| format!("TypeError: {e}"))?;
                let r = base.join(candidate).map_err(|_| format!("TypeError: Failed to construct 'URL': Invalid URL"))?;
                resolved = Some(r.clone());

                if loader.data_as_string && r.scheme() == "data" {
                    return Err("TypeError: Failed to fetch".to_string());
                }

                let data = self.fetcher.fetch(&r)?;
                accept(self, Loaded::Data(x3d_parse::gunzip(&data), r))
            })();

            match result {
                Ok(()) => return true,
                Err(error) => {
                    attempts += 1;
                    self.print_load_error(loader, candidate, resolved.as_ref(), &error, attempts, remaining);
                }
            }
        }

        false
    }

    /// FileLoader.printError for the candidate that just failed: `attempts` counts it, `remaining`
    /// is the number of candidates after it.
    pub(crate) fn print_load_error(&mut self, loader: &FileLoader, candidate: &str, resolved: Option<&url::Url>, error: &str, attempts: usize, remaining: usize) {
        let data_url = resolved.is_some_and(|r| r.scheme() == "data");
        let resolved_text = match resolved {
            Some(r) if !data_url => decode_uri(r.as_str()).unwrap_or_else(|| r.as_str().to_string()),
            _ => String::new(),
        };
        let subject = if candidate.is_empty() {
            "empty URL".to_string()
        } else if data_url {
            "data URL".to_string()
        } else if !resolved_text.is_empty() {
            format!("URL '{resolved_text}'")
        } else {
            format!("URL '{}'", truncate(candidate))
        };
        let prefix = if loader.type_name.is_empty() { String::new() } else { format!("{}: ", loader.type_name) };

        if remaining > 0 {
            self.log_warn(format!("{prefix}Couldn't load {subject}, trying URL {} of {}. {error}", attempts + 1, attempts + remaining));
        } else if attempts == 1 {
            self.log_error(format!("{prefix}Couldn't load {subject}. {error}"));
        } else {
            self.log_error(format!("{prefix}Couldn't load any of the {attempts} URLs, tried in this order:"));
        }
    }

    pub(crate) fn url_strings(&self, node: NodeId) -> Vec<String> {
        match self.pre_value(node, "url") {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        }
    }

    /// Inline.loadData / ImageTexture.loadData … : run the FileLoader for the node.
    pub(crate) fn url_load(&mut self, node: NodeId) {
        let type_name = self.type_name(node);

        if type_name == "MovieTexture" {
            return self.movie_load(node);
        }

        if type_name == "Inline" {
            return self.inline_load(node);
        }

        let loader = FileLoader { type_name: type_name.clone(), base: self.base_url(node), data_as_string: false };
        let urls = self.url_strings(node);

        let ok = self.load_document(&loader, &urls, |_rt, loaded| match loaded {
            Loaded::Script(_) => Err("Error: Couldn't parse X3D. No suitable file handler found.".into()),
            Loaded::Data(..) => Ok(()),
        });

        self.set_load_state(node, if ok { nodes::LoadState::Complete } else { nodes::LoadState::Failed });
    }

    /// MovieTexture.loadData: one attempt per URL, each with its own FileLoader (GIF/PNG) or a
    /// `<video>` element (anything else).
    ///
    /// Decoding is not ported. A fetched GIF/PNG is taken as loaded. For video, headless
    /// Chromium fires `error` for a missing file and for formats it cannot play; an existing
    /// .webm/.ogv/.ogg/.mp4/.m4v is taken as playable (an assumption, not observed in the corpora).
    fn movie_load(&mut self, node: NodeId) {
        let base = self.base_url(node);
        let Ok(base) = url::Url::parse(&base) else {
            return;
        };

        for candidate in self.url_strings(node) {
            // `new URL (url, base)` is outside the try: an invalid URL rejects loadData.
            let Ok(file_url) = base.join(&candidate) else {
                return;
            };
            let decoded = decode_uri(file_url.as_str()).unwrap_or_else(|| file_url.as_str().to_string());
            let path = file_url.path().to_string();
            let href = file_url.as_str();
            let image = path.ends_with(".gif") || path.ends_with(".png") || href.starts_with("data:image/gif") || href.starts_with("data:image/png");

            let error = if image {
                let loader = FileLoader { type_name: "MovieTexture".into(), base: href.to_string(), data_as_string: false };
                let ok = self.load_document(&loader, &[href.to_string()], |_, _| Ok(()));

                if ok {
                    return;
                }

                "MovieTexture: No suitable file handler found.".to_string()
            } else {
                let playable = [".webm", ".ogv", ".ogg", ".mp4", ".m4v"].iter().any(|e| path.to_ascii_lowercase().ends_with(e));

                if playable && self.fetcher.fetch(&file_url).is_ok() {
                    return;
                }

                format!("Couldn't load video '{href}': error.")
            };

            self.log_warn(format!("Error loading movie '{decoded}': {error}"));
            self.log_error(format!("Error: {error}"));
        }

        self.set_load_state(node, nodes::LoadState::Failed);
    }

    /// X3DFontStyleNode.initialize → requestImmediateLoad → loadData (font selection only).
    ///
    /// Each family name is tried in order, with "SERIF" appended: a default family (SERIF,
    /// SANS, TYPEWRITER) is X_ITE's own font and always loads; a FontLibrary lookup finds
    /// nothing (FontLibrary is not ported); a family that looks like a font file URL is fetched
    /// through browser.loadFont (cached per URL); anything else warns.
    pub(crate) fn font_style_initialize(&mut self, node: NodeId) {
        let style = match self.pre_value(node, "style") {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => String::new(),
        };
        let mut family = match self.pre_value(node, "family") {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        };

        family.push("SERIF".into());

        let Ok(base) = url::Url::parse(&self.base_url(node)) else {
            return;
        };

        for f in family {
            if matches!(f.as_str(), "SERIF" | "SANS" | "TYPEWRITER") {
                return;
            }

            // `new URL (fontFamily, base)` throws for an invalid URL: loadData rejects silently.
            let Ok(file_url) = base.join(&f) else {
                return;
            };

            let path = file_url.path().to_ascii_lowercase();
            let font_file = [".woff2", ".woff", ".otf", ".ttf"].iter().any(|e| path.ends_with(e));

            if file_url.scheme() == "data" || font_file {
                if self.scene.specification_version.as_str() >= "4.1" {
                    self.log_warn("Loading a font file via family field is deprecated, please use new FontLibrary node instead.");
                }

                let href = file_url.as_str().to_string();

                let ok = match self.font_cache.get(&href) {
                    Some(ok) => *ok,
                    None => {
                        let loader = FileLoader { type_name: self.type_name(node), base: href.clone(), data_as_string: false };
                        // browser.loadFont: `OpenType .parse (await decompressFont (data))`. A file that
                        // does not parse throws inside the loader's async callback, so the promise never
                        // resolves: the FontStyle stays without a font and no further family is tried.
                        let mut parsed: Option<Result<std::rc::Rc<x3d_text::Font>, x3d_text::FontError>> = None;
                        let ok = self.load_document(&loader, std::slice::from_ref(&href), |_, loaded| {
                            if let Loaded::Data(data, _) = loaded {
                                parsed = Some(x3d_text::Font::parse(&data).map(std::rc::Rc::new));
                            }

                            Ok(())
                        });

                        if let Some(result) = parsed {
                            x3d_text::fonts::register_url_font(&href, result.ok());
                        }

                        if !ok && file_url.scheme() != "data" {
                            let decoded = decode_uri(&href).unwrap_or_else(|| href.clone());
                            self.log_warn(format!("Error loading font '{decoded}':"));
                        }

                        self.font_cache.insert(href, ok);
                        ok
                    }
                };

                if ok {
                    return;
                }
            } else {
                self.log_warn(format!("Couldn't find font family '{f}' with style '{style}'."));
            }
        }
    }
}
