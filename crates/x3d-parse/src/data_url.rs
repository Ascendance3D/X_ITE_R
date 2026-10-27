//! `fetch (dataURL)`: the body a browser gives for a `data:` URL, whatever the resource is (an X3D file, a glTF
//! buffer, a font, an image, a sound, a movie). The fetch standard's data: URL processor
//! (https://fetch.spec.whatwg.org/#data-url-processor) with the URL parser's trimming, as Chromium does it.

/// The body of the `data:` URL `href` (raw, as authored, or serialized by a URL parser), or the fetch rejection
/// `TypeError: Failed to fetch` when the URL has no comma or its base64 does not decode.
///
/// - The URL parser strips leading and trailing C0 controls and spaces and removes every tab and newline; the
///   fragment is not part of the URL that is read.
/// - The media type is the text before the first comma. It names base64 when it ends with `;`, any spaces, and
///   `base64` in any case (`;BASE64` too).
/// - The body is percent-decoded first; a `%` that two hex digits do not follow stays as it is.
/// - Base64 is forgiving-base64: ASCII whitespace is skipped, the padding is optional but must be right, and any
///   other character fails.
pub fn decode_data_url(href: &str) -> Result<Vec<u8>, String> {
    let failed = || "TypeError: Failed to fetch".to_string();
    let url: String = href
        .trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();

    if url.len() < 5 || !url[..5].eq_ignore_ascii_case("data:") {
        return Err(failed());
    }

    let rest = url[5..].split('#').next().unwrap_or("");
    let rest = rest.trim_matches(is_ascii_whitespace);
    let (mime, body) = rest.split_once(',').ok_or_else(failed)?;
    let body = percent_decode(body.as_bytes());

    if is_base64(mime.trim_matches(is_ascii_whitespace)) {
        return forgiving_base64_decode(&body).ok_or_else(failed);
    }

    Ok(body)
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
    use super::decode_data_url;

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
}
