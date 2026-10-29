/// JavaScript's `\s`: Unicode `White_Space` without U+0085, and U+FEFF.
pub fn is_js_space(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}
