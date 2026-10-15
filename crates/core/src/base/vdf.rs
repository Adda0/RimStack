use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Library {
    pub path: PathBuf,
    pub apps: Vec<String>,
}

pub(crate) enum Token {
    Open,
    Close,
    Text(String),
}

pub(crate) fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '"' => {
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => s.extend(chars.next()),
                        c => s.push(c),
                    }
                }
                out.push(Token::Text(s));
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// In file order; text that is not VDF gives none.
pub fn libraries(text: &str) -> Vec<Library> {
    let mut out: Vec<Library> = Vec::new();
    let mut blocks: Vec<String> = Vec::new();
    let mut key: Option<String> = None;
    for token in tokens(text) {
        match token {
            Token::Open => blocks.push(key.take().unwrap_or_default()),
            Token::Close => {
                blocks.pop();
                key = None;
            }
            Token::Text(s) => match key.take() {
                None => key = Some(s),
                Some(k) if blocks.len() == 2 && k.eq_ignore_ascii_case("path") => {
                    out.push(Library {
                        path: s.into(),
                        apps: Vec::new(),
                    });
                }
                Some(k) if blocks.len() == 3 && blocks[2].eq_ignore_ascii_case("apps") => {
                    if let Some(library) = out.last_mut() {
                        library.apps.push(k);
                    }
                }
                Some(_) => {}
            },
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_every_library_and_its_apps() {
        let text = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/u/.local/share/Steam"
		"label"		""
		"apps"
		{
			"228980"		"5"
		}
	}
	"1"
	{
		"path"		"/mnt/games"
		"apps"
		{
			"294100"		"577359346"
			"730"		"1"
		}
	}
}"#;
        let libs = libraries(text);
        assert_eq!(libs.len(), 2);
        assert_eq!(libs[0].path, PathBuf::from("/home/u/.local/share/Steam"));
        assert_eq!(libs[0].apps, ["228980"]);
        assert_eq!(libs[1].path, PathBuf::from("/mnt/games"));
        assert_eq!(libs[1].apps, ["294100", "730"]);
    }

    #[test]
    fn escaped_backslashes_become_one() {
        let libs = libraries(r#""libraryfolders" { "0" { "path" "D:\\Steam Library" } }"#);
        assert_eq!(libs[0].path, PathBuf::from(r"D:\Steam Library"));
    }

    #[test]
    fn garbage_gives_nothing() {
        for text in [
            "",
            "not vdf at all",
            "\"libraryfolders\" {",
            "{{{{",
            "\"path\"",
            "}}}",
        ] {
            assert_eq!(libraries(text), vec![], "input: {text:?}");
        }
    }
}
