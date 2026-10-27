//! `fetch (dataURL)`: the body a browser gives for a `data:` URL, whatever the resource is (an X3D file, a glTF
//! buffer, a font, an image, a sound, a movie). It follows what headless Chromium's `new URL` and `fetch` give (the
//! probes of PR #123), which is close to the fetch standard's data: URL processor
//! (https://fetch.spec.whatwg.org/#data-url-processor) but not the same: the media type is not parsed into
//! parameters, and a tab or newline after the scheme can be kept (`keeps_tab_and_newline`).

/// The body of the `data:` URL `href` (raw, as authored, or serialized by a URL parser), or the fetch rejection
/// `TypeError: Failed to fetch` when the URL is not a data URL, has no comma or its base64 does not decode.
///
/// - Leading and trailing C0 controls and spaces are stripped. The scheme is `data` in any case; a tab or newline in
///   it is removed. After the scheme, a tab or newline is kept (as `%09`, `%0A`, `%0D`) when `href` starts with
///   `data:` (`keeps_tab_and_newline`), and removed otherwise. The fragment is not read.
/// - The media type is the text before the first comma. It names base64 when it ends with `;`, any spaces, `base64`
///   in any case (`;BASE64` too) and any spaces. Its parameters are not parsed (`;foo=bar;base64` is base64,
///   `;foo=base64` is not), and it is not percent-decoded (`;%62ase64` is not base64).
/// - The body is percent-decoded first; a `%` that two hex digits do not follow stays as it is. Its spaces are kept.
/// - Base64 is forgiving-base64: ASCII whitespace is skipped, the padding is optional but must be right, and any
///   other character fails.
pub fn decode_data_url(href: &str) -> Result<Vec<u8>, String> {
    let failed = || "TypeError: Failed to fetch".to_string();
    let href: std::borrow::Cow<'_, str> = if keeps_tab_and_newline(href) {
        href.into()
    } else {
        href.replace(['\t', '\n', '\r'], "").into()
    };
    let path = data_url_path(&href).ok_or_else(failed)?;
    let path = escape_tab_and_newline(path.split('#').next().unwrap_or(""));
    let (mime, body) = path.split_once(',').ok_or_else(failed)?;
    let body = percent_decode(body.as_bytes());

    if is_base64(mime.trim_end_matches(' ')) {
        return forgiving_base64_decode(&body).ok_or_else(failed);
    }

    Ok(body)
}

/// Whether Chromium's `new URL (href)` gives a `data:` URL: the scheme is `data` in any case, a tab or newline in it
/// removed, after leading and trailing C0 controls and spaces are stripped.
pub fn is_data_url(href: &str) -> bool {
    data_url_path(href).is_some()
}

/// `href` as a URL parser that follows the URL standard (the `url` crate) must read it to give the URL Chrome's
/// `new URL (href, base)` gives. The standard removes every tab and newline of the input; Chrome keeps those after the
/// scheme of a URL that starts with `data:` (`keeps_tab_and_newline`), as `%09`, `%0A` and `%0D`. Every other URL is
/// returned as it is.
pub fn chromium_url_input(href: &str) -> std::borrow::Cow<'_, str> {
    match data_url_path(href) {
        Some(path) if keeps_tab_and_newline(href) && path.contains(['\t', '\n', '\r']) => {
            format!("data:{}", escape_tab_and_newline(path)).into()
        }
        _ => href.into(),
    }
}

/// Whether Chrome keeps the tabs and newlines of the URL `href`: after any leading tabs and newlines, it starts with
/// `data:` in any case. Chrome 156.0.8078.47 (the browser of the pinned X_ITE recordings) does this; otherwise it
/// removes them like the URL standard (` data:`, `da<TAB>ta:`). Chromium 151.0.7922.34 kept them only after an exact
/// `data:` at the start (probes 1-3 of XITE_R_WEB_DATA_URL_COMPAT_01).
fn keeps_tab_and_newline(href: &str) -> bool {
    href.trim_start_matches(['\t', '\n', '\r'])
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
}

/// The text after the scheme of a `data:` URL, without the leading and trailing C0 controls and spaces of `href`.
fn data_url_path(href: &str) -> Option<&str> {
    let (scheme, path) = href.trim_matches(|c: char| c <= ' ').split_once(':')?;
    let scheme: String = scheme
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();

    scheme.eq_ignore_ascii_case("data").then_some(path)
}

/// A tab or newline after the scheme, as Chromium keeps it.
fn escape_tab_and_newline(path: &str) -> String {
    path.replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// The URL standard's percent-decode: `%` and two hex digits is one byte; any other `%` stays.
pub(crate) fn percent_decode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;

    while i < input.len() {
        if input[i] == b'%'
            && i + 2 < input.len()
            && let (Some(h), Some(l)) = (hex(input[i + 1]), hex(input[i + 2]))
        {
            out.push(h << 4 | l);
            i += 3;
            continue;
        }

        out.push(input[i]);
        i += 1;
    }

    out
}

fn hex(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

fn is_ascii_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\x0c' | '\r' | ' ')
}

/// The media type ends with `;`, zero or more spaces, and an ASCII case-insensitive `base64`.
fn is_base64(mime: &str) -> bool {
    let Some(head) = mime
        .len()
        .checked_sub(6)
        .filter(|&i| mime.is_char_boundary(i) && mime[i..].eq_ignore_ascii_case("base64"))
        .map(|i| &mime[..i])
    else {
        return false;
    };

    head.trim_end_matches(' ').ends_with(';')
}

/// The infra standard's forgiving-base64 decode.
fn forgiving_base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut data: Vec<u8> = input
        .iter()
        .copied()
        .filter(|&b| !is_ascii_whitespace(b as char))
        .collect();

    if data.len().is_multiple_of(4) {
        if data.ends_with(b"==") {
            data.truncate(data.len() - 2);
        } else if data.ends_with(b"=") {
            data.truncate(data.len() - 1);
        }
    }

    if data.len() % 4 == 1 {
        return None;
    }

    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);

    for c in data {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };

        acc = (acc << 6) | v as u32;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{chromium_url_input, decode_data_url, is_data_url};

    fn base64(data: &[u8]) -> String {
        const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();

        for chunk in data.chunks(3) {
            let n = chunk.iter().fold(0u32, |n, &b| n << 8 | b as u32) << (8 * (3 - chunk.len()));

            for k in 0..=chunk.len() {
                out.push(B64[(n >> (18 - 6 * k)) as usize & 63] as char);
            }
        }

        out
    }

    /// The values headless Chromium 151 gives for `fetch (url)` (the probe in the lane report of PR #123).
    #[test]
    fn chromium_fetch_values() {
        let ok: &[(&str, &[u8])] = &[
            ("data:,AB", b"AB"),
            ("data:;base64,QUJD", b"ABC"),
            ("data:;BASE64,QUJD", b"ABC"),
            ("data:;Base64,QUJD", b"ABC"),
            ("data:; base64,QUJD", b"ABC"),
            ("data:;base64 ,QUJD", b"ABC"),
            ("data:x/y;base64;foo,QUJD", b"QUJD"),
            ("data:;base64,QUJ", b"AB"),
            ("data:;base64,QU", b"A"),
            ("data:;base64,QQ==", b"A"),
            ("data:;base64,QQ", b"A"),
            ("data:;base64,QU JD", b"ABC"),
            ("data:;base64,QU%4AD", b"ABC"),
            ("data:;base64,", b""),
            ("data:,", b""),
            ("data:,%zz%41", b"%zzA"),
            ("data:,%4", b"%4"),
            ("data:,%", b"%"),
            ("data:,%e9", &[0xe9]),
            ("data:,a#frag", b"a"),
            ("data:;base64,QUJD#frag", b"ABC"),
            ("data:,a?b", b"a?b"),
            ("data:,%C3%A9", &[0xc3, 0xa9]),
            ("data:,é", &[0xc3, 0xa9]),
            ("data:,a b", b"a b"),
            ("data:,a%20b", b"a b"),
            ("data:;charset=utf-8;base64,QUJD", b"ABC"),
            ("data:,%+1", b"%+1"),
            ("data:;base64,QUJD\x0c", b"ABC"),
            (" data:,lead", b"lead"),
            ("data:;base64,QU\tJD", b"ABC"),
            ("DATA:,upper", b"upper"),
            ("data: ;base64,QUJD", b"ABC"),
            ("data:text/plain;base64  ,QUJD", b"ABC"),
        ];
        let failed = [
            "data:;base64,Q",
            "data:;base64,QUJD=",
            "data:;base64,QUJD==",
            "data:;base64,QQ=",
            "data:;base64,QQ==QQ",
            "data:;base64,Q-_A",
            "data:;base64,%",
            "data:noComma",
            "http://example/,a",
        ];

        for (url, body) in ok {
            assert_eq!(decode_data_url(url).as_deref(), Ok(*body), "{url}");
        }

        for url in failed {
            assert_eq!(
                decode_data_url(url),
                Err("TypeError: Failed to fetch".to_string()),
                "{url}"
            );
        }
    }

    /// Bytes of every value survive both encodings (fonts, images, sounds are binary).
    #[test]
    fn binary_bytes() {
        let all: Vec<u8> = (0..=255).collect();
        let percent: String = all.iter().map(|b| format!("%{b:02X}")).collect();

        assert_eq!(
            decode_data_url(&format!("data:application/octet-stream,{percent}")),
            Ok(all.clone())
        );
        assert_eq!(
            decode_data_url(&format!("data:font/ttf;BASE64,{}", base64(&all))),
            Ok(all)
        );
    }

    /// A `%` before a character of more than one byte (the URL as authored, not serialized) does not panic.
    #[test]
    fn percent_before_multibyte() {
        assert_eq!(decode_data_url("data:,%aé"), Ok("%aé".as_bytes().to_vec()));
        assert_eq!(decode_data_url("data:,%é"), Ok("%é".as_bytes().to_vec()));
    }

    /// The PR #123 final review probe (headless Chromium 151.0.7922.34, `fetch (url)` and `new URL (url) .href`): the
    /// scheme in any case, MIME parameters and the base64 flag, empty and whitespace media types, whitespace, percent
    /// escapes and non-ASCII text in the media type and the body, base64 lengths, bad escapes and binary bytes. Each
    /// row is (the URL as authored, Chromium's serialization of it, the body or `None` for `Failed to fetch`).
    #[test]
    fn chromium_fetch_values_final_review() {
        #[rustfmt::skip]
        let rows: &[(&str, &str, Option<&[u8]>)] = &[
            ("DATA:,x", "data:,x", Some(&[120])),
            ("Data:,x", "data:,x", Some(&[120])),
            ("dAtA:;BaSe64,QUJD", "data:;BaSe64,QUJD", Some(&[65, 66, 67])),
            (" DATA:,lead", "data:,lead", Some(&[108, 101, 97, 100])),
            ("DATA:text/plain;BASE64,QUJD", "data:text/plain;BASE64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;foo=bar;base64,QUJD", "data:text/plain;foo=bar;base64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;foo=base64,QUJD", "data:text/plain;foo=base64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;BASE64,QUJD", "data:text/plain;BASE64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;bAsE64,QUJD", "data:text/plain;bAsE64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain; foo=bar ; base64 ,QUJD", "data:text/plain; foo=bar ; base64 ,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;base64;,QUJD", "data:text/plain;base64;,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;base64=,QUJD", "data:text/plain;base64=,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;base64=x,QUJD", "data:text/plain;base64=x,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;base64;charset=utf-8,QUJD", "data:text/plain;base64;charset=utf-8,QUJD", Some(&[81, 85, 74, 68])),
            ("data:base64,QUJD", "data:base64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:;;base64,QUJD", "data:;;base64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;xbase64,QUJD", "data:text/plain;xbase64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;x base64,QUJD", "data:text/plain;x base64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:;base64,QUJD", "data:;base64,QUJD", Some(&[65, 66, 67])),
            ("data:,AB", "data:,AB", Some(&[65, 66])),
            ("data: ,AB", "data: ,AB", Some(&[65, 66])),
            ("data:  ;base64,QUJD", "data:  ;base64,QUJD", Some(&[65, 66, 67])),
            ("data:\u{c};base64,QUJD", "data:%0C;base64,QUJD", Some(&[65, 66, 67])),
            ("data: ; base64 ,QUJD", "data: ; base64 ,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;\u{9}base64,QUJD", "data:text/plain;%09base64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;base64\u{9},QUJD", "data:text/plain;base64%09,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;\u{c}base64,QUJD", "data:text/plain;%0Cbase64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;base64\u{c},QUJD", "data:text/plain;base64%0C,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;\u{b}base64,QUJD", "data:text/plain;%0Bbase64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;\u{a0}base64,QUJD", "data:text/plain;%C2%A0base64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;base64\u{a0},QUJD", "data:text/plain;base64%C2%A0,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;\u{a}base64,QUJD", "data:text/plain;%0Abase64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;\u{d}base64\u{d},QUJD", "data:text/plain;%0Dbase64%0D,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;%62ase64,QUJD", "data:text/plain;%62ase64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;base%36%34,QUJD", "data:text/plain;base%36%34,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain;charset=%C3%A9;base64,QUJD", "data:text/plain;charset=%C3%A9;base64,QUJD", Some(&[65, 66, 67])),
            ("data:t\u{eb}xt/plain;base64,QUJD", "data:t%C3%ABxt/plain;base64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;charset=\u{e9};base64,QUJD", "data:text/plain;charset=%C3%A9;base64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;n\u{e4}me=v;base64,QUJD", "data:text/plain;n%C3%A4me=v;base64,QUJD", Some(&[65, 66, 67])),
            ("data:text/plain;%3Bbase64,QUJD", "data:text/plain;%3Bbase64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:text/plain%3Bbase64,QUJD", "data:text/plain%3Bbase64,QUJD", Some(&[81, 85, 74, 68])),
            ("data:;base64,QU\u{a}JD", "data:;base64,QU%0AJD", Some(&[65, 66, 67])),
            ("data:;base64,QU\u{c}JD", "data:;base64,QU%0CJD", Some(&[65, 66, 67])),
            ("data:;base64,Q U J D", "data:;base64,Q U J D", Some(&[65, 66, 67])),
            ("data:;base64,QU%20JD", "data:;base64,QU%20JD", Some(&[65, 66, 67])),
            ("data:;base64,QU%09JD", "data:;base64,QU%09JD", Some(&[65, 66, 67])),
            ("data:;base64,QU%0CJD", "data:;base64,QU%0CJD", Some(&[65, 66, 67])),
            ("data:;base64,QU%0AJD", "data:;base64,QU%0AJD", Some(&[65, 66, 67])),
            ("data:;base64,QU%0DJD", "data:;base64,QU%0DJD", Some(&[65, 66, 67])),
            ("data:;base64,QUJD%20", "data:;base64,QUJD%20", Some(&[65, 66, 67])),
            ("data:;base64,%20QUJD", "data:;base64,%20QUJD", Some(&[65, 66, 67])),
            ("data:;base64,QU\u{a0}JD", "data:;base64,QU%C2%A0JD", None),
            ("data:;base64,QU%0BJD", "data:;base64,QU%0BJD", None),
            ("data:;base64,QQ%3D%3D", "data:;base64,QQ%3D%3D", Some(&[65])),
            ("data:;base64,QQ= =", "data:;base64,QQ= =", Some(&[65])),
            ("data:;base64,QQ%20==", "data:;base64,QQ%20==", Some(&[65])),
            ("data:,%G1", "data:,%G1", Some(&[37, 71, 49])),
            ("data:,%1", "data:,%1", Some(&[37, 49])),
            ("data:,%%41", "data:,%%41", Some(&[37, 65])),
            ("data:;base64,QU%JD", "data:;base64,QU%JD", None),
            ("data:,%41%", "data:,%41%", Some(&[65, 37])),
            ("data:,%4G", "data:,%4G", Some(&[37, 52, 71])),
            ("data:;base64,QU%4", "data:;base64,QU%4", None),
            ("data:application/octet-stream,%00%FF%80", "data:application/octet-stream,%00%FF%80", Some(&[0, 255, 128])),
            ("data:,%00", "data:,%00", Some(&[0])),
            ("data:application/octet-stream;base64,AP+A", "data:application/octet-stream;base64,AP+A", Some(&[0, 255, 128])),
            ("data:application/octet-stream;base64,AP%2BA", "data:application/octet-stream;base64,AP%2BA", Some(&[0, 255, 128])),
            ("data:,\u{ff}", "data:,%C3%BF", Some(&[195, 191])),
            ("data:,\u{20ac}", "data:,%E2%82%AC", Some(&[226, 130, 172])),
            ("data:;base64,", "data:;base64,", Some(&[])),
            ("data:;base64,Q", "data:;base64,Q", None),
            ("data:;base64,QU", "data:;base64,QU", Some(&[65])),
            ("data:;base64,QUJ", "data:;base64,QUJ", Some(&[65, 66])),
            ("data:;base64,QUJD", "data:;base64,QUJD", Some(&[65, 66, 67])),
            ("data:;base64,QUJDR", "data:;base64,QUJDR", None),
            ("data:;base64,QUJDRA", "data:;base64,QUJDRA", Some(&[65, 66, 67, 68])),
            ("data:;base64,QUJDRA=", "data:;base64,QUJDRA=", None),
            ("data:;base64,QUJDRA==", "data:;base64,QUJDRA==", Some(&[65, 66, 67, 68])),
            ("data:;base64,QUJDREU", "data:;base64,QUJDREU", Some(&[65, 66, 67, 68, 69])),
            ("data:;base64,QUJDREU=", "data:;base64,QUJDREU=", Some(&[65, 66, 67, 68, 69])),
            ("data:;base64,QUJDREVG", "data:;base64,QUJDREVG", Some(&[65, 66, 67, 68, 69, 70])),
            ("data:;base64,QUJDREVGR", "data:;base64,QUJDREVGR", None),
            ("data:;base64,QUJD=", "data:;base64,QUJD=", None),
            ("data:;base64,QUJD====", "data:;base64,QUJD====", None),
            ("data:;base64,QQ===", "data:;base64,QQ===", None),
            ("data:;base64,=", "data:;base64,=", None),
            ("data:;base64,==", "data:;base64,==", None),
            ("data:;base64,QQ=A", "data:;base64,QQ=A", None),
            ("data:;base64,QUJ=", "data:;base64,QUJ=", Some(&[65, 66])),
            ("data:;base64,QU==", "data:;base64,QU==", Some(&[65])),
            ("data:;base64,QUI=", "data:;base64,QUI=", Some(&[65, 66])),
            ("data:;base64,QUJE", "data:;base64,QUJE", Some(&[65, 66, 68])),
            ("data:,a\u{9}b", "data:,a%09b", Some(&[97, 9, 98])),
            ("data:,a\u{a}b", "data:,a%0Ab", Some(&[97, 10, 98])),
            ("data:,a\u{d}b", "data:,a%0Db", Some(&[97, 13, 98])),
            ("data:,a\u{d}\u{a}b", "data:,a%0D%0Ab", Some(&[97, 13, 10, 98])),
            ("data:;base64,QU\u{9}JD", "data:;base64,QU%09JD", Some(&[65, 66, 67])),
            ("data:te\u{9}xt/plain;base64,QUJD", "data:te%09xt/plain;base64,QUJD", Some(&[65, 66, 67])),
            ("data:\u{c};base64,QUJD", "data:%0C;base64,QUJD", Some(&[65, 66, 67])),
            ("data:;base64\u{c} ,QUJD", "data:;base64%0C ,QUJD", Some(&[81, 85, 74, 68])),
            ("data:;base64 \u{c},QUJD", "data:;base64 %0C,QUJD", Some(&[81, 85, 74, 68])),
            ("data:;base64\u{9} ,QUJD", "data:;base64%09 ,QUJD", Some(&[81, 85, 74, 68])),
            ("data:\u{9}DATA", "data:%09DATA", None),
            ("da\u{9}ta:,x", "data:,x", Some(&[120])),
            ("data\u{9}:,x", "data:,x", Some(&[120])),
            ("\u{9}data:,x", "data:,x", Some(&[120])),
            ("data:,a #x", "data:,a #x", Some(&[97, 32])),
            ("data:,a\u{c}#x", "data:,a%0C#x", Some(&[97, 12])),
            ("data:, a", "data:, a", Some(&[32, 97])),
            ("data:,a\u{c}\u{c}b", "data:,a%0C%0Cb", Some(&[97, 12, 12, 98])),
            ("data:;base64,QUJD #x", "data:;base64,QUJD #x", Some(&[65, 66, 67])),
            ("data:x/y ,a", "data:x/y ,a", Some(&[97])),
            ("data:,\u{c}a", "data:,%0Ca", Some(&[12, 97])),
        ];
        let mut differ = Vec::new();

        for &(url, href, body) in rows {
            let expected = body
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "TypeError: Failed to fetch".to_string());

            for input in [url, href] {
                if decode_data_url(input) != expected {
                    differ.push(format!(
                        "{input:?}: {:?}, Chromium {expected:?}",
                        decode_data_url(input)
                    ));
                }
            }
        }

        assert!(differ.is_empty(), "{}", differ.join("\n"));
    }

    /// A URL whose fifth byte is inside a character of more than one byte is no data URL; it does not panic.
    #[test]
    fn non_ascii_scheme() {
        let failed = Err("TypeError: Failed to fetch".to_string());

        assert_eq!(decode_data_url("da\u{e9}\u{e9}"), failed);
        assert_eq!(decode_data_url("d\u{e9}ta:,x"), failed);
        assert_eq!(decode_data_url("\u{e9}"), failed);
        assert_eq!(decode_data_url(""), failed);
    }

    /// The scheme as Chromium reads it (XITE_R_WEB_DATA_URL_COMPAT_01 probe): any case, a tab or newline in it removed,
    /// leading and trailing C0 controls and spaces stripped. A space in it, an escape or another scheme is no data URL.
    #[test]
    fn data_scheme_like_chromium() {
        for url in [
            "data:,x",
            "DATA:,x",
            "Data:,x",
            "da\tta:,x",
            "da\nta:,x",
            "da\rta:,x",
            "data\t:,x",
            "D\tA\nT\rA:,x",
            "\tdata:,x",
            " data:,x",
            "data:,x\t",
            "data:",
        ] {
            assert!(is_data_url(url), "{url:?}");
        }

        for url in [
            "da ta:,x",
            "da%09ta:,x",
            "dat\t:,x",
            "d\tata-x:,y",
            "data\t",
            "datafile.wrl",
            "",
            "\u{e9}data:,x",
        ] {
            assert!(!is_data_url(url), "{url:?}");
        }
    }

    /// Only a data URL with a tab or newline after its scheme changes; that one keeps them as escapes.
    #[test]
    fn chromium_url_input_escapes_only_after_a_data_scheme() {
        use std::borrow::Cow;

        for url in [
            "data:,ab",
            "DA\tTA:,ab",
            "DA\tTA:,a\tb",
            "http://example.test/a\tb.bin",
            "ht\ttp://example.test/e.bin",
            "rel\tative\n.bin",
            "da ta:,a\tb",
            "",
        ] {
            assert!(
                matches!(chromium_url_input(url), Cow::Borrowed(u) if u == url),
                "{url:?}"
            );
        }

        assert_eq!(chromium_url_input("data:,a\tb"), "data:,a%09b");
        assert_eq!(
            chromium_url_input("\t\nDATA:\t\n\r;base64,QU\tJD\t "),
            "data:%09%0A%0D;base64,QU%09JD"
        );
        // A leading space, or a tab in the scheme: the URL standard's removal (the `url` crate does it).
        for url in [" DATA:\t,a\tb", "DA\tTA:\t,a\tb", "\u{1}data:,a\tb"] {
            assert!(
                matches!(chromium_url_input(url), Cow::Borrowed(u) if u == url),
                "{url:?}"
            );
        }
    }

    /// The glTF buffer fetch without a fetcher of the runtime: a data URL in any scheme form decodes, an invalid one
    /// fails like `fetch`, and another scheme is not read as one.
    #[test]
    fn gltf_default_fetch_reads_the_data_scheme_like_chromium() {
        use crate::gltf2::default_fetch;

        let base = "file:///nonexistent/dir/model.gltf";
        let bytes: &[u8] = &[0, 1, 2, 3, 4, 5];
        let failed = Err("TypeError: Failed to fetch".to_string());

        for uri in [
            "data:application/octet-stream;base64,AAECAwQF",
            "DATA:application/octet-stream;base64,AAECAwQF",
            "Data:application/octet-stream;BASE64,AAECAwQF",
            "da\tta:application/octet-stream;base64,AAECAwQF",
            "DA\tTA:application/octet-stream;BASE64,AAECAwQF",
            "data\n:application/octet-stream;base64,AAEC\tAwQF",
            "data:application/octet-stream,%00%01%02%03%04%05",
        ] {
            assert_eq!(default_fetch(uri, base).as_deref(), Ok(bytes), "{uri:?}");
        }

        assert_eq!(
            default_fetch("DATA:application/octet-stream,%00%01\n%FF", base),
            Ok(vec![0, 1, 10, 255])
        );
        // `;<TAB>base64` is no base64 flag after `DATA:`; after `DA<TAB>TA:` the tab is removed and it is one.
        assert_eq!(
            default_fetch("DATA:application/octet-stream;\tbase64,AAEC", base),
            Ok(b"AAEC".to_vec())
        );
        assert_eq!(
            default_fetch("DA\tTA:application/octet-stream;\tbase64,AAEC", base),
            Ok(vec![0, 1, 2])
        );
        assert_eq!(
            default_fetch(" DATA:application/octet-stream,%00\t%01", base),
            Ok(vec![0, 1])
        );
        assert_eq!(
            default_fetch("\tDATA:application/octet-stream,%00\t%01", base),
            Ok(vec![0, 9, 1])
        );
        assert_eq!(
            default_fetch("DA\tTA:application/octet-stream;base64,AA@@", base),
            failed
        );
        assert_eq!(default_fetch("da\tta:nocomma", base), failed);
        // `dat<TAB>:` is the scheme `dat`, not a data URL: no local file is read for it.
        assert_eq!(default_fetch("dat\t:,x", base), failed);
    }

    /// Probe 3 of XITE_R_WEB_DATA_URL_COMPAT_01: `new URL (prefix + ",a<TAB>b<LF>c") .href` in Chrome 156.0.8078.47.
    /// The decoder gives the body of that URL for the authored one: `a\tb\nc` or `abc`.
    #[test]
    fn chrome_keeps_tabs_only_after_a_leading_data_scheme() {
        let kept: &[u8] = b"a\tb\nc";
        let removed: &[u8] = b"abc";
        #[rustfmt::skip]
        let rows: &[(&str, &[u8])] = &[
            ("data:", kept), ("DATA:", kept), ("Data:", kept), ("dAtA:", kept), ("\tdata:", kept), ("\ndata:", kept),
            ("\rdata:", kept), ("\t\tdata:", kept), ("\t\nDATA:", kept), ("\tDATA:", kept), ("data:\t", kept),
            ("DATA:\n", kept), (" data:", removed), ("  data:", removed), ("\t data:", removed), (" \tdata:", removed),
            ("\u{b}data:", removed), ("\u{c}data:", removed), ("\u{0}data:", removed), ("\u{1f}data:", removed),
            ("da\tta:", removed), ("data\t:", removed), ("\tda\nta:", removed),
        ];

        for &(prefix, body) in rows {
            let url = format!("{prefix},a\tb\nc");

            assert_eq!(decode_data_url(&url).as_deref(), Ok(body), "{url:?}");
        }
    }
}
