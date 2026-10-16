//! Text as tmux prints it. tmux 3.4 format output escapes a backslash as `\\`,
//! seven control bytes in C style (`\t`, `\n`, `\r`, `\a`, `\b`, `\v`, `\f`)
//! and every other control byte as `\NNN` octal. The CLI and the daemon both
//! undo that with [`unescape_tmux_format`].

/// Undoes tmux's format escaping: `\\` becomes `\`, the seven C-style escapes
/// become their control bytes, and `\NNN` (three octal digits, at most `\377`)
/// becomes that byte. Anything else, including a lone trailing backslash, an
/// unknown letter escape or an invalid octal escape, stays as it is. Bytes that
/// do not form UTF-8 become U+FFFD.
///
/// The escaping above is what tmux prints for a name set by `rename-window`.
/// A name set by `new-window -n` prints a tab raw and does not double a
/// backslash, so such a name with `\t` or `\\` in it does not round-trip
/// exactly. That limit is accepted.
#[must_use]
pub fn unescape_tmux_format(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(rest) = bytes.get(index..).filter(|rest| !rest.is_empty()) {
        let (byte, width) = escape_at(rest).unwrap_or((rest[0], 1));
        decoded.push(byte);
        index += width;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// The byte an escape at the start of `rest` stands for, and the escape's length.
fn escape_at(rest: &[u8]) -> Option<(u8, usize)> {
    let digit = |byte: u8| byte - b'0';
    match rest {
        [b'\\', b'\\', ..] => Some((b'\\', 2)),
        [b'\\', high @ b'0'..=b'3', middle @ b'0'..=b'7', low @ b'0'..=b'7', ..] => Some((digit(*high) * 64 + digit(*middle) * 8 + digit(*low), 4)),
        [b'\\', letter, ..] => c_style_control(*letter).map(|control| (control, 2)),
        _ => None,
    }
}

fn c_style_control(letter: u8) -> Option<u8> {
    match letter {
        b't' => Some(b'\t'),
        b'n' => Some(b'\n'),
        b'r' => Some(b'\r'),
        b'a' => Some(0x07),
        b'b' => Some(0x08),
        b'v' => Some(0x0b),
        b'f' => Some(0x0c),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn unescapes_the_cases_tmux_prints() {
        let cases = [
            (r"a\\b", "a\\b"),
            (r"x\037y", "x\u{1f}y"),
            (r"tab\011end", "tab\tend"),
            (r"del\177", "del\u{7f}"),
            ("trailing\\", "trailing\\"),
            (r"bad\08x", r"bad\08x"),
            (r"short\03", r"short\03"),
            (r"too big\400", r"too big\400"),
            (r"other\n", "other\n"),
            (r"a\tb\nc\rd\ae\bf\vg\fh", "a\tb\nc\rd\u{7}e\u{8}f\u{b}g\u{c}h"),
            (r"\\t", r"\t"),
            (r"unknown\q", r"unknown\q"),
            ("café·x", "café·x"),
            (r"\\037", r"\037"),
            ("", ""),
        ];
        for (escaped, expected) in cases {
            assert_eq!(unescape_tmux_format(escaped), expected, "{escaped:?}");
        }
    }

    #[test]
    fn octal_bytes_that_form_utf8_join_into_one_character() {
        assert_eq!(unescape_tmux_format(r"caf\303\251"), "café");
        assert_eq!(unescape_tmux_format(r"caf\303"), "caf\u{FFFD}");
    }

    /// tmux's rule for text set by `rename-window`: a backslash doubles, the
    /// seven C-style controls print as `\t` and the like, any other control
    /// byte prints as `\NNN`, and valid UTF-8 prints raw.
    fn tmux_escape(text: &str) -> String {
        text.chars()
            .map(|character| match character {
                '\\' => "\\\\".to_string(),
                '\t' => r"\t".to_string(),
                '\n' => r"\n".to_string(),
                '\r' => r"\r".to_string(),
                '\u{7}' => r"\a".to_string(),
                '\u{8}' => r"\b".to_string(),
                '\u{b}' => r"\v".to_string(),
                '\u{c}' => r"\f".to_string(),
                control if control.is_ascii_control() => format!("\\{:03o}", u32::from(control)),
                printable => printable.to_string(),
            })
            .collect()
    }

    fn tmux_alphabet() -> impl Strategy<Value = String> {
        let character = prop_oneof![
            4 => any::<char>().prop_filter("printable", |character| !character.is_control()),
            2 => Just('\\'),
            2 => (0_u8..0x20).prop_map(char::from),
            1 => Just('\u{7f}'),
            1 => prop_oneof![Just('0'), Just('3'), Just('7'), Just('t'), Just('n'), Just('a'), Just('v')],
        ];
        proptest::collection::vec(character, 0..64).prop_map(|characters| characters.into_iter().collect())
    }

    proptest! {
        #[test]
        fn unescape_inverts_tmux_escaping(text in tmux_alphabet()) {
            prop_assert_eq!(unescape_tmux_format(&tmux_escape(&text)), text);
        }
    }
}
