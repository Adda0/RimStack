//! XML read with a depth limit. `roxmltree` parses by recursion, so a small file nested a
//! hundred thousand levels deep overflows the stack and aborts the process. Few real mod XML
//! files nest past twenty; a save's nested tags stay under forty. 64 halves deep recursion's
//! cost while leaving room for a deeply-nested mod, and an over-deep file is reported.

use roxmltree::Document;

/// Nesting beyond this is not parsed: that would recurse without limit.
const MAX_DEPTH: usize = 64;

/// Reason an XML text was refused: what it is (a mod file, a save) is known by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooDeep {
    Nesting,
}

impl std::fmt::Display for TooDeep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TooDeep::Nesting => write!(f, "XML nests too deeply; it is not read"),
        }
    }
}

/// First pass of a scan: count element nesting without recursing. `roxmltree` cannot parse
/// anything this deep, so the text is refused before it can.
///
/// Bytes are compared directly: XML names and markup are ASCII, and a byte of a UTF-8 unit
/// never equals an ASCII delimiter.
pub fn nests_too_deeply(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut depth: usize = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => {
                if bytes.get(i + 1) == Some(&b'/') {
                    // Closing tag: unwind; a stray one cannot underflow the depth.
                    depth = depth.saturating_sub(1);
                    i += 2;
                } else if bytes.get(i + 1) == Some(&b'!') || bytes.get(i + 1) == Some(&b'?') {
                    // Comment, DOCTYPE, PI: find the closing `>` (or `]]>` for CDATA).
                    i += 2;
                    skip_special(bytes, &mut i);
                } else {
                    depth += 1;
                    if depth > MAX_DEPTH {
                        return true;
                    }
                    i += 1;
                    let self_closing = tag_is_self_closing(bytes, &mut i);
                    if self_closing {
                        depth = depth.saturating_sub(1);
                    }
                }
            }
            _ => i += 1,
        }
    }
    false
}

/// After `<!`, `<?`: advance past the closing `>`, honouring `-->` for comments and `]]>` for
/// CDATA so a `>` inside does not end the scan early.
fn skip_special(bytes: &[u8], i: &mut usize) {
    // The `!`/`?` has already been consumed (i points past it).
    let rest = &bytes[*i..];
    // Comment: `<!-- ... -->`
    if rest.starts_with(b"--") {
        *i += 2;
        while *i + 2 < bytes.len() {
            if &bytes[*i..*i + 3] == b"-->" {
                *i += 3;
                return;
            }
            *i += 1;
        }
        *i = bytes.len();
        return;
    }
    // CDATA: `<![CDATA[ ... ]]>`
    if rest.starts_with(b"[CDATA[") {
        *i += 7;
        while *i + 2 < bytes.len() {
            if &bytes[*i..*i + 3] == b"]]>" {
                *i += 3;
                return;
            }
            *i += 1;
        }
        *i = bytes.len();
        return;
    }
    // DOCTYPE, PI, entity: skip to `>`.
    while *i < bytes.len() {
        if bytes[*i] == b'>' {
            *i += 1;
            return;
        }
        *i += 1;
    }
}

/// From just after `<name` (i at the space or `>`): move past the closing `>`. Returns whether
/// the tag was self-closing (`/>`).
fn tag_is_self_closing(bytes: &[u8], i: &mut usize) -> bool {
    while *i < bytes.len() {
        match bytes[*i] {
            b'>' => {
                *i += 1;
                return *i >= 2 && bytes.get(*i - 2) == Some(&b'/');
            }
            b'"' | b'\'' => {
                // Skip quoted attribute value so a `>` inside it does not end the tag.
                let quote = bytes[*i];
                *i += 1;
                while *i < bytes.len() && bytes[*i] != quote {
                    *i += 1;
                }
            }
            _ => {}
        }
        *i += 1;
    }
    false
}

/// Parse XML; return the roxmltree document, or the parse error string. Over-deep documents
/// are refused before `roxmltree::Document::parse` is called.
pub fn parse(text: &str) -> Result<(Document<'_>, roxmltree::TextPos), String> {
    if nests_too_deeply(text) {
        return Err(TooDeep::Nesting.to_string());
    }
    Document::parse(text)
        .map(|doc| (doc, roxmltree::TextPos { row: 0, col: 0 }))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_comment_or_self_closing_tag_does_not_count_as_nesting_nor_a_stray_close() {
        // `<x/>` contributes no depth; a stray `</x>` cannot underflow it.
        let text = "<a><!-- <x> --><b/><c attr=\">\"/></a></stray>";
        assert!(!nests_too_deeply(text));

        // A CDATA section contributes no nesting.
        let cdata = "<root><![CDATA[some <not-xml> content]]></root>";
        assert!(!nests_too_deeply(cdata));

        // 64 deep: fine.
        let at_limit = "<a>".repeat(64);
        assert!(!nests_too_deeply(&at_limit));

        // 65 deep: refused.
        let too_deep = "<a>".repeat(65);
        assert!(nests_too_deeply(&too_deep));
    }

    #[test]
    fn parse_refuses_deep_nesting_before_roxmltree() {
        let deep = "<a>".repeat(100) + &"</a>".repeat(100);
        let err = parse(&deep);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("nests too deeply"));

        let ok = "<root><child>text</child></root>";
        let (doc, _) = parse(ok).unwrap();
        assert_eq!(doc.root_element().tag_name().name(), "root");
    }

    #[test]
    fn self_closing_depth_is_undone() {
        let text = "<a><b/><c/><d/></a>";
        assert!(!nests_too_deeply(text));
    }

    #[test]
    fn stray_closing_tag_no_underflow() {
        assert!(!nests_too_deeply("</a></b></c>"));
        assert!(!nests_too_deeply("</a><a/>"));
    }

    #[test]
    fn comment_does_not_count() {
        let text = "<a><!-- <b><c><d><e> --></a>";
        assert!(!nests_too_deeply(text));
    }
}
