//! Workshop descriptions are BBCode; the game's own tags (`<b>`, `<color=…>`) are read too.

const PAIRS: [(&str, &str); 12] = [
    ("b", "b"),
    ("i", "i"),
    ("u", "u"),
    ("strike", "s"),
    ("h1", "h3"),
    ("h2", "h4"),
    ("h3", "h4"),
    ("list", "ul"),
    ("olist", "ol"),
    ("quote", "blockquote"),
    ("code", "pre"),
    ("spoiler", "i"),
];

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn without_game_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out += &rest[..open];
        let inside = &rest[open + 1..];
        if !inside.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/') {
            out.push('<');
            rest = inside;
            continue;
        }
        // No `>` is left for any later `<` either.
        let Some(close) = inside.find('>') else {
            out.push('<');
            rest = inside;
            break;
        };
        let tag = inside[..close].to_lowercase();
        if matches!(tag.as_str(), "b" | "/b" | "i" | "/i") {
            out += &format!("[{tag}]");
        }
        rest = &inside[close + 1..];
    }
    out + rest
}

/// The next `needle` in what is left of `text`, looked for again only once the reading passes
/// it: looking from each of many openings would take quadratic time.
struct Ahead<'t> {
    text: &'t str,
    needle: &'static str,
    /// From the start of `text`; `Some(None)` when there is none left.
    found: Option<Option<usize>>,
}

impl<'t> Ahead<'t> {
    fn new(text: &'t str, needle: &'static str) -> Self {
        Self {
            text,
            needle,
            found: None,
        }
    }

    /// Where in `rest`, a tail of `text`.
    fn within(&mut self, rest: &str) -> Option<usize> {
        let from = self.text.len() - rest.len();
        let at = match self.found {
            Some(Some(at)) if at >= from => Some(at),
            Some(None) => None,
            _ => *self
                .found
                .insert(rest.find(self.needle).map(|at| from + at)),
        };
        at.map(|at| at - from)
    }
}

fn safe_link(url: &str) -> bool {
    ["https://", "http://", "steam://"]
        .iter()
        .any(|s| url.starts_with(s))
}

/// A link is closed only if it was opened: one to an address that is not a
/// page never is.
fn tag_html(tag: &str, in_link: &mut bool) -> Option<String> {
    let (closing, name) = match tag.strip_prefix('/') {
        Some(name) => (true, name),
        None => (false, tag),
    };
    // No name is longer than `spoiler`: what runs on without an `=` is no tag, however long.
    let (name, value) = match name.bytes().take(9).position(|b| b == b'=') {
        Some(at) => (&name[..at], &name[at + 1..]),
        None if name.len() > 8 => return None,
        None => (name, ""),
    };
    let name = name.to_lowercase();
    if let Some((_, html)) = PAIRS.iter().find(|(bb, _)| *bb == name) {
        return Some(format!("<{}{html}>", if closing { "/" } else { "" }));
    }
    Some(match (name.as_str(), closing) {
        ("*", false) => "<li>".to_string(),
        ("hr", false) => "<hr>".to_string(),
        ("url", false) if safe_link(value) => {
            *in_link = true;
            format!("<a href=\"{value}\">")
        }
        ("url", true) if std::mem::take(in_link) => "</a>".to_string(),
        ("hr" | "table" | "tr" | "td" | "th" | "noparse" | "url", _) => String::new(),
        _ => return None,
    })
}

/// The output is the HTML subset a Qt text view shows.
pub fn rich(text: &str) -> String {
    let text = escape(&without_game_tags(text));
    let mut out = String::with_capacity(text.len() + 64);
    let mut rest = text.as_str();
    let mut in_link = false;
    let mut tag_end = Ahead::new(&text, "]");
    let mut picture_end = Ahead::new(&text, "[/img]");
    while let Some(open) = rest.find('[') {
        out += &rest[..open];
        rest = &rest[open..];
        let Some(close) = tag_end.within(rest) else {
            break;
        };
        let tag = &rest[1..close];
        let after = &rest[close + 1..];
        if tag.eq_ignore_ascii_case("img") {
            rest = picture_end
                .within(after)
                .map_or(after, |end| &after[end + 6..]);
        } else if tag.eq_ignore_ascii_case("url") {
            let end = after.find("[/url]").unwrap_or(after.len());
            let url = &after[..end];
            out += &match safe_link(url) {
                true => format!("<a href=\"{url}\">{url}</a>"),
                false => url.to_string(),
            };
            rest = after.get(end + 6..).unwrap_or("");
        } else if let Some(html) = tag_html(tag, &mut in_link) {
            out += &html;
            rest = after;
        } else {
            out.push('[');
            rest = &rest[1..];
        }
    }
    out += rest;
    // A line break beside a block is the block's own: Qt would show it as an extra line.
    let mut html = out.replace("\r\n", "\n");
    for block in [
        "<ul>",
        "</ul>",
        "<ol>",
        "</ol>",
        "<li>",
        "<hr>",
        "</h3>",
        "</h4>",
        "<blockquote>",
        "</blockquote>",
    ] {
        html = html
            .replace(&format!("\n{block}"), block)
            .replace(&format!("{block}\n"), block);
    }
    html.trim().replace('\n', "<br>")
}

/// Whether `[tag]` is markup; `Some(true)` for one that stands between words, as a list item.
fn apart(tag: &str) -> Option<bool> {
    let name = tag.strip_prefix('/').unwrap_or(tag);
    let name = name.split_once('=').map_or(name, |(name, _)| name);
    match name.to_lowercase().as_str() {
        "b" | "i" | "u" | "strike" | "spoiler" | "url" | "noparse" => Some(false),
        "*" | "hr" | "table" | "tr" | "td" | "th" => Some(true),
        other => PAIRS.iter().any(|(bb, _)| *bb == other).then_some(true),
    }
}

/// One line of plain text: markup and pictures left out, every run of spaces and line breaks one space.
pub fn plain(text: &str) -> String {
    let text = without_game_tags(text);
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(open) = rest.find('[') {
        out += &rest[..open];
        rest = &rest[open..];
        let Some(close) = rest.find(']') else {
            break;
        };
        let tag = &rest[1..close];
        let after = &rest[close + 1..];
        if tag.eq_ignore_ascii_case("img") {
            rest = after.find("[/img]").map_or(after, |end| &after[end + 6..]);
            out.push(' ');
        } else if let Some(apart) = apart(tag) {
            if apart {
                out.push(' ');
            }
            rest = after;
        } else {
            out.push('[');
            rest = &rest[1..];
        }
    }
    out += rest;
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workshop_markup_becomes_html() {
        assert_eq!(
            rich("[h1]Title[/h1]\n[b]Bold[/b] and [i]slanted[/i]\nNext"),
            "<h3>Title</h3><b>Bold</b> and <i>slanted</i><br>Next"
        );
        assert_eq!(
            rich("[list]\n[*]One\n[*][b]Two[/b]\n[/list]\n[hr][/hr]"),
            "<ul><li>One<li><b>Two</b></ul><hr>"
        );
        assert_eq!(rich("[quote]Said[/quote]"), "<blockquote>Said</blockquote>");
    }

    #[test]
    fn links_open_only_pages() {
        assert_eq!(
            rich("[url=https://example.org/?a=1&b=2]Page[/url]"),
            "<a href=\"https://example.org/?a=1&amp;b=2\">Page</a>"
        );
        assert_eq!(
            rich("[url]https://example.org[/url]"),
            "<a href=\"https://example.org\">https://example.org</a>"
        );
        assert_eq!(rich("[url=javascript:x]Bad[/url]"), "Bad");
        assert_eq!(rich("[url]file:///etc[/url]"), "file:///etc");
    }

    #[test]
    fn pictures_are_left_out_and_other_brackets_stay() {
        assert_eq!(rich("[img]REPLACE_HERO[/img]\nText"), "Text");
        assert_eq!(rich("[CF] Mod [1.6] a[0]"), "[CF] Mod [1.6] a[0]");
        assert_eq!(rich("open [ never closed"), "open [ never closed");
        assert_eq!(rich("[img]\nRequires Harmony."), "Requires Harmony.");
    }

    #[test]
    fn the_game_s_own_tags_are_read_and_html_is_not() {
        assert_eq!(
            rich("<b>Bold</b> <color=#ff0000>red</color>"),
            "<b>Bold</b> red"
        );
        assert_eq!(rich("a < b & c"), "a &lt; b &amp; c");
        assert_eq!(rich("x [script]y"), "x [script]y");
    }

    #[test]
    fn plain_text_keeps_the_words_and_drops_the_markup() {
        assert_eq!(
            plain(
                "[h1]Title[/h1]\r\n[b]Bold[/b], [url=https://x.org]a page[/url].\n[img]https://x.org/a.png[/img]\n[list][*]One[*]Two[/list]"
            ),
            "Title Bold, a page. One Two"
        );
        assert_eq!(
            plain("<color=#ff0000>Red</color> [CF] Mod [1.6]"),
            "Red [CF] Mod [1.6]"
        );
        assert_eq!(plain("open [ never closed"), "open [ never closed");
    }
}
