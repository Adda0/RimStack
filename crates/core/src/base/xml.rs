//! Every XML file RimStack reads is parsed here. roxmltree descends one call per level of
//! nesting, so a file nested a hundred thousand deep would overflow the stack and abort the
//! app: a file nested deeper than any the game reads is refused before it is parsed.

use roxmltree::{Document, ParsingOptions};
use std::fmt;

/// Twice as deep as the deepest file of the game (24) or a mod (34) found. roxmltree overflows a
/// thread's 2 MiB stack at 136 levels in a debug build (3436 in a release build), so this leaves
/// the caller half the stack even there.
pub const DEEPEST: usize = 64;

#[derive(Debug)]
pub enum Refused {
    /// The line where elements nest deeper than `DEEPEST`.
    TooDeep(u32),
    Malformed(roxmltree::Error),
}

impl Refused {
    /// The line the reading stopped at.
    pub fn row(&self) -> u32 {
        match self {
            Self::TooDeep(row) => *row,
            Self::Malformed(problem) => problem.pos().row,
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooDeep(row) => write!(f, "elements nested over {DEEPEST} deep at {row}:1"),
            Self::Malformed(problem) => problem.fmt(f),
        }
    }
}

pub fn parse(text: &str) -> Result<Document<'_>, Refused> {
    parse_with(text, ParsingOptions::default())
}

pub fn parse_with<'input>(
    text: &'input str,
    options: ParsingOptions<'input>,
) -> Result<Document<'input>, Refused> {
    if let Some(at) = past_deepest(text.as_bytes()) {
        let row = text.as_bytes()[..at]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1;
        return Err(Refused::TooDeep(u32::try_from(row).unwrap_or(u32::MAX)));
    }
    Document::parse_with_options(text, options).map_err(Refused::Malformed)
}

/// Where an element opens past `DEEPEST`, in one pass. Comments, CDATA, processing
/// instructions and declarations nest nothing; a quoted value may hold `>`. Malformed text may
/// be counted a little off, and is refused by the parser anyway.
fn past_deepest(bytes: &[u8]) -> Option<usize> {
    let after = |from: usize, end: &[u8]| {
        let rest = &bytes[from..];
        let found = rest.windows(end.len()).position(|w| w == end);
        found.map_or(bytes.len(), |at| from + at + end.len())
    };
    let mut depth = 0;
    let mut at = 0;
    while let Some(found) = bytes[at..].iter().position(|b| *b == b'<') {
        let open = at + found;
        let rest = &bytes[open..];
        at = if rest.starts_with(b"<!--") {
            after(open + 4, b"-->")
        } else if rest.starts_with(b"<![CDATA[") {
            after(open + 9, b"]]>")
        } else if rest.starts_with(b"<?") {
            after(open + 2, b"?>")
        } else if rest.starts_with(b"<!") {
            after(open + 2, b">")
        } else if rest.starts_with(b"</") {
            depth = usize::saturating_sub(depth, 1);
            after(open + 2, b">")
        } else {
            // A tag ends at the first `>` outside quotes; a `<` is never inside a tag.
            let mut quote = None;
            let mut end = None;
            for (offset, byte) in rest.iter().enumerate().skip(1) {
                match (quote, *byte) {
                    (_, b'<') => break,
                    (Some(q), b) if b == q => quote = None,
                    (Some(_), _) => {}
                    (None, b'"' | b'\'') => quote = Some(*byte),
                    (None, b'>') => {
                        end = Some(offset);
                        break;
                    }
                    (None, _) => {}
                }
            }
            let closed = end.is_some_and(|end| rest[end - 1] == b'/');
            if !closed {
                depth += 1;
                if depth > DEEPEST {
                    return Some(open);
                }
            }
            end.map_or(open + 1, |end| open + end + 1)
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested(levels: usize) -> String {
        format!("{}x{}", "<a k='>'>".repeat(levels), "</a>".repeat(levels))
    }

    /// Half of what a thread gets: the other half is the caller's.
    #[test]
    fn a_file_as_deep_as_allowed_is_read_in_half_a_thread_s_stack() {
        let thread = std::thread::Builder::new().stack_size(1024 * 1024);
        let read = thread.spawn(|| parse(&nested(DEEPEST)).map(|d| d.descendants().count()));
        assert_eq!(read.unwrap().join().unwrap().unwrap(), DEEPEST + 2);
    }

    /// roxmltree would overflow this thread from 136 levels on: the refusal comes first.
    #[test]
    fn a_hundred_thousand_levels_are_refused_on_a_thread_s_own_stack() {
        let thread = std::thread::Builder::new().stack_size(2 * 1024 * 1024);
        let read = thread.spawn(|| matches!(parse(&nested(100_000)), Err(Refused::TooDeep(1))));
        assert!(read.unwrap().join().unwrap());
    }

    #[test]
    fn a_file_nested_deeper_is_refused_at_the_line_it_goes_too_deep() {
        let text = format!(
            "<?xml version='1.0'?>\n<!-- <b> -->\n{}",
            nested(DEEPEST + 1)
        );
        let refused = parse(&text).unwrap_err();
        assert_eq!(refused.row(), 3);
        assert_eq!(refused.to_string(), "elements nested over 64 deep at 3:1");
    }

    #[test]
    fn what_nests_nothing_is_not_counted() {
        let flat = "<a/><b c=\"/>\"/>".repeat(1000);
        let skipped = "<!-- <a> --><![CDATA[<a>]]><?pi <a>?>".repeat(1000);
        let text = format!("<r>{flat}{skipped}{}</r>", "<a></a>".repeat(1000));
        assert!(parse(&text).is_ok());
    }

    #[test]
    fn a_malformed_file_gives_the_parser_s_own_problem() {
        let refused = parse("<a>\n<b></a>").unwrap_err();
        assert!(matches!(refused, Refused::Malformed(_)));
        assert_eq!(refused.row(), 2);
    }
}
