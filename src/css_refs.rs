//! Lenient `url(...)` extractor for CSS files. Handles the three
//! idiomatic forms (`url("...")`, `url('...')`, `url(...)`); skips
//! `data:` URLs. Avoids a full CSS-parser dep — covers `~95%` of
//! real-world references at this scope.

/// Extract every `url(...)` reference from a CSS body. Returns the
/// inner URL strings, trimmed, in document order. Duplicates are
/// preserved (caller dedupes if needed); `data:` URLs are skipped.
pub fn extract_url_refs(css: &str) -> Vec<String> {
    let needle = "url(";
    let mut refs: Vec<String> = Vec::new();
    let bytes = css.as_bytes();
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        let Some(rel) = css[i..].find(needle) else {
            break;
        };
        let body_start = i + rel + needle.len();
        // Skip whitespace after `url(`.
        let mut s = body_start;
        while s < bytes.len() && (bytes[s] == b' ' || bytes[s] == b'\t' || bytes[s] == b'\n') {
            s += 1;
        }
        if s >= bytes.len() {
            break;
        }
        // Determine quoting; choose terminator.
        let (terminator, value_start) = match bytes[s] {
            b'"' => (b'"', s + 1),
            b'\'' => (b'\'', s + 1),
            _ => (b')', s),
        };
        // Find terminator.
        let Some(end_off) = bytes[value_start..].iter().position(|b| *b == terminator) else {
            break;
        };
        let end = value_start + end_off;
        let raw = &css[value_start..end];
        let url = raw.trim().to_string();
        if !url.is_empty() && !url.starts_with("data:") {
            refs.push(url);
        }
        // Advance past the terminator (and the closing `)` if quoted).
        i = if terminator == b')' {
            end + 1
        } else {
            // Skip past closing quote, then look for the closing `)`.
            let mut j = end + 1;
            while j < bytes.len() && bytes[j] != b')' {
                j += 1;
            }
            j + 1
        };
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_quoted_urls() {
        let css = r#"a { background: url("foo.png"); }
b { background: url('bar.png'); }"#;
        let refs = extract_url_refs(css);
        assert_eq!(refs, vec!["foo.png", "bar.png"]);
    }

    #[test]
    fn extracts_unquoted_url() {
        let css = "a { background: url(/_assets/x.svg); }";
        let refs = extract_url_refs(css);
        assert_eq!(refs, vec!["/_assets/x.svg"]);
    }

    #[test]
    fn skips_data_urls() {
        let css = r#"a { background: url("data:image/png;base64,xx"); }
b { background: url(/real/path.png); }"#;
        let refs = extract_url_refs(css);
        assert_eq!(refs, vec!["/real/path.png"]);
    }

    #[test]
    fn handles_multiple_per_line() {
        let css = "a { background: url(a.png), url(b.png); }";
        let refs = extract_url_refs(css);
        assert_eq!(refs, vec!["a.png", "b.png"]);
    }

    #[test]
    fn handles_font_face_block() {
        let css = r#"@font-face {
  font-family: "Foo";
  src: url("/fonts/foo.woff2") format("woff2"),
       url("/fonts/foo.woff") format("woff");
}"#;
        let refs = extract_url_refs(css);
        assert_eq!(refs, vec!["/fonts/foo.woff2", "/fonts/foo.woff"]);
    }

    #[test]
    fn handles_whitespace_inside_paren() {
        let css = "a { background: url(  spaces.png  ); }";
        let refs = extract_url_refs(css);
        assert_eq!(refs, vec!["spaces.png"]);
    }

    #[test]
    fn empty_url_skipped() {
        let css = "a { background: url(); }";
        let refs = extract_url_refs(css);
        assert!(refs.is_empty());
    }
}
