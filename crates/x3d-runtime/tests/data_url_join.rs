//! `join_url` gives the URL Chromium's `new URL (candidate, base)` gives, and `fetch` of a `data:` URL the body
//! Chromium gives, for a raw tab, LF or CR in and after the data scheme, with http(s) and relative URLs as controls
//! (XITE_R_WEB_DATA_URL_COMPAT_01).

use x3d_runtime::loader::join_url;

const BASE: &str = "http://example.test/page/index.html";

/// Probe 1 of the lane (`evidence/probes/chromium-probe.jsonl`, headless Chromium 151.0.7922.34; Chrome 156.0.8078.47
/// gives the same values, `chrome156-probe.jsonl`); its server origin is `example.test` here. Each row: the candidate as authored, Chromium's `new URL (candidate, base) .href`, and the
/// body of `fetch` of it: `None` when it is no data URL or the fetch rejects.
#[rustfmt::skip]
const ROWS: &[(&str, &str, Option<&[u8]>)] = &[
        ("data:,ab", "data:,ab", Some(&[97, 98])),
        ("DATA:,ab", "data:,ab", Some(&[97, 98])),
        ("Data:;base64,QUJD", "data:;base64,QUJD", Some(&[65, 66, 67])),
        ("data:text/plain;base64,QUJD", "data:text/plain;base64,QUJD", Some(&[65, 66, 67])),
        ("da\tta:,x", "data:,x", Some(&[120])),
        ("da\nta:,x", "data:,x", Some(&[120])),
        ("da\rta:,x", "data:,x", Some(&[120])),
        ("data\t:,x", "data:,x", Some(&[120])),
        ("D\tA\nT\rA:,x", "data:,x", Some(&[120])),
        ("\tdata:,x", "data:,x", Some(&[120])),
        ("data:,x\t", "data:,x", Some(&[120])),
        ("data:,x\n", "data:,x", Some(&[120])),
        ("data:\t,ab", "data:%09,ab", Some(&[97, 98])),
        ("data:\n,ab", "data:%0A,ab", Some(&[97, 98])),
        ("data:\r,ab", "data:%0D,ab", Some(&[97, 98])),
        ("data:\t\n\r;base64,QUJD", "data:%09%0A%0D;base64,QUJD", Some(&[65, 66, 67])),
        ("data:,a\tb", "data:,a%09b", Some(&[97, 9, 98])),
        ("data:,a\nb", "data:,a%0Ab", Some(&[97, 10, 98])),
        ("data:,a\rb", "data:,a%0Db", Some(&[97, 13, 98])),
        ("data:,a\r\nb\t", "data:,a%0D%0Ab", Some(&[97, 13, 10, 98])),
        ("data:;base64,QU\tJD", "data:;base64,QU%09JD", Some(&[65, 66, 67])),
        ("data:;base64,QU\nJ\rD", "data:;base64,QU%0AJ%0DD", Some(&[65, 66, 67])),
        ("data:text/plain;\tbase64,QUJD", "data:text/plain;%09base64,QUJD", Some(&[81, 85, 74, 68])),
        ("data:text/plain;base64\t,QUJD", "data:text/plain;base64%09,QUJD", Some(&[81, 85, 74, 68])),
        ("data:text/plain\n;base64,QUJD", "data:text/plain%0A;base64,QUJD", Some(&[65, 66, 67])),
        ("data:text/plain;charset=utf-8\t;base64,QUJD", "data:text/plain;charset=utf-8%09;base64,QUJD", Some(&[65, 66, 67])),
        ("data:,a%09b", "data:,a%09b", Some(&[97, 9, 98])),
        ("data:,a%0Ab", "data:,a%0Ab", Some(&[97, 10, 98])),
        ("data:,a%0Db", "data:,a%0Db", Some(&[97, 13, 98])),
        ("data:;base64,QU%09JD", "data:;base64,QU%09JD", Some(&[65, 66, 67])),
        ("data:text/plain;%09base64,QUJD", "data:text/plain;%09base64,QUJD", Some(&[81, 85, 74, 68])),
        ("da%09ta:,x", "http://example.test/page/da%09ta:,x", None),
        ("data\t", "http://example.test/page/data", None),
        ("data:\tnocomma", "data:%09nocomma", None),
        ("data:;base64,QQ=\t@", "data:;base64,QQ=%09@", None),
        ("dat\t:,x", "dat:,x", None),
        ("d\tata-x:,y", "data-x:,y", None),
        ("da ta:,x", "http://example.test/page/da%20ta:,x", None),
        ("data:application/octet-stream;base64,AAEC\tAwQF", "data:application/octet-stream;base64,AAEC%09AwQF", Some(&[0, 1, 2, 3, 4, 5])),
        ("data:application/octet-stream,%00%01\n%FF", "data:application/octet-stream,%00%01%0A%FF", Some(&[0, 1, 10, 255])),
        ("DA\tTA:application/octet-stream;BASE64,AAECAwQF", "data:application/octet-stream;BASE64,AAECAwQF", Some(&[0, 1, 2, 3, 4, 5])),
        ("http://example.test/page/a\tb.bin", "http://example.test/page/ab.bin", None),
        ("http://example.test/page/c\nd.bin", "http://example.test/page/cd.bin", None),
        ("ht\ttp://example.test/page/e.bin", "http://example.test/page/e.bin", None),
        ("https://example.invalid/x\ty.bin", "https://example.invalid/xy.bin", None),
        ("rel\tative\n.bin", "http://example.test/page/relative.bin", None),
    // Probe 2 in Chrome 156.0.8078.47 (`chrome156-probe-2.jsonl`), the browser of the pinned X_ITE recordings: a raw
    // tab or newline in the body is kept only when the URL starts with `data:` in any case, after any leading tabs
    // and newlines. Chromium 151 kept it only after an exact lower-case `data:` (`chromium-probe-2.jsonl`).
        ("data:,a\tb", "data:,a%09b", Some(&[97, 9, 98])),
        ("DATA:,a\tb", "data:,a%09b", Some(&[97, 9, 98])),
        ("Data:,a\nb", "data:,a%0Ab", Some(&[97, 10, 98])),
        ("da\tta:,a\tb", "data:,ab", Some(&[97, 98])),
        ("data\t:,a\tb", "data:,ab", Some(&[97, 98])),
        ("D\nATA:,a\rb", "data:,ab", Some(&[97, 98])),
        ("\tdata:,a\tb", "data:,a%09b", Some(&[97, 9, 98])),
        (" data:,a\tb", "data:,ab", Some(&[97, 98])),
        ("\n DATA:,a\tb", "data:,ab", Some(&[97, 98])),
        ("\u{1}data:,a\tb", "data:,ab", Some(&[97, 98])),
        ("da\tta:;base64,QU\tJD", "data:;base64,QUJD", Some(&[65, 66, 67])),
        ("da\tta:text/plain;\tbase64,QUJD", "data:text/plain;base64,QUJD", Some(&[65, 66, 67])),
        ("data\n:text/plain;\tbase64,QUJD", "data:text/plain;base64,QUJD", Some(&[65, 66, 67])),
        ("DATA:text/plain;\tbase64,QUJD", "data:text/plain;%09base64,QUJD", Some(&[81, 85, 74, 68])),
        ("da\tta:\t,ab", "data:,ab", Some(&[97, 98])),
        ("data:\t\t,ab", "data:%09%09,ab", Some(&[97, 98])),
];

#[test]
fn candidates_resolve_like_chromium() {
    let base = url::Url::parse(BASE).unwrap();
    let mut differ = Vec::new();

    for &(candidate, href, _) in ROWS {
        let joined = join_url(&base, candidate).map(String::from);

        if joined.as_deref() != Ok(href) {
            differ.push(format!("{candidate:?}: {joined:?}, Chromium {href:?}"));
        }

        // `new URL (String (url), base)` gives the same URL again.
        if let Ok(again) = join_url(&base, href)
            && again.as_str() != href
        {
            differ.push(format!("{href:?} again: {again}"));
        }
    }

    assert!(differ.is_empty(), "{}", differ.join("\n"));
}

#[test]
fn data_urls_fetch_like_chromium() {
    let base = url::Url::parse(BASE).unwrap();
    let mut differ = Vec::new();

    for &(candidate, href, body) in ROWS {
        let data = href.starts_with("data:");

        if x3d_parse::is_data_url(candidate) != data {
            differ.push(format!("{candidate:?}: is_data_url {}", !data));
        }

        if !data {
            continue;
        }

        let expected = body
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "TypeError: Failed to fetch".to_string());
        let joined = join_url(&base, candidate).unwrap();

        // The authored URL (the glTF buffer path), and the URL the fetcher receives.
        for input in [candidate, joined.as_str()] {
            let got = x3d_parse::decode_data_url(input);

            if got != expected {
                differ.push(format!("{input:?}: {got:?}, Chromium {expected:?}"));
            }
        }
    }

    assert!(differ.is_empty(), "{}", differ.join("\n"));
}
