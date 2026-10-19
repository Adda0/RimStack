//! Sentences are put in words here, on the window's thread, as they are handed to QML.

use cxx_qt_lib::QString;
use rimstack_core::base::error::Error;
use rimstack_core::base::sentence::{Language, Sentence};
use rimstack_core::base::translate::{Counting, LOCALE};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Words {
    table: HashMap<String, String>,
    locale: String,
    counting: Counting,
}

impl Default for Words {
    fn default() -> Self {
        Self::of(&Map::new())
    }
}

impl Words {
    pub fn of(table: &Map<String, Value>) -> Self {
        let said = |(english, said): (&String, &Value)| {
            let said = said.as_str().filter(|said| !said.is_empty())?;
            Some((english.clone(), said.to_string()))
        };
        let table: HashMap<String, String> = table.iter().filter_map(said).collect();
        let locale = table.get(LOCALE).cloned().unwrap_or_default();
        Self {
            counting: Counting::of(&locale),
            locale,
            table,
        }
    }
}

impl Language for Words {
    fn word<'a>(&'a self, english: &'a str) -> &'a str {
        self.table.get(english).map_or(english, String::as_str)
    }

    fn counting(&self) -> &Counting {
        &self.counting
    }
}

thread_local! {
    /// Set on the window's thread only.
    static ON_SCREEN: RefCell<Rc<Words>> = RefCell::new(Rc::new(Words::default()));
}

pub fn show_in(words: Words) {
    ON_SCREEN.with(|on_screen| *on_screen.borrow_mut() = Rc::new(words));
}

pub fn put(sentence: &Sentence) -> String {
    let words = ON_SCREEN.with(|on_screen| on_screen.borrow().clone());
    sentence.put(words.as_ref())
}

pub fn counted(n: usize, one: &str, many: &str, locale: &str) -> String {
    let words = ON_SCREEN.with(|on_screen| on_screen.borrow().clone());
    if words.locale == locale {
        return words.counting.pick(n, one, many).to_string();
    }
    Counting::of(locale).pick(n, one, many).to_string()
}

pub fn said(sentence: &Sentence) -> QString {
    QString::from(&put(sentence))
}

pub fn said_of(problem: &Error) -> QString {
    said(problem.sentence())
}

/// Empty when it was done.
pub fn why_not<T>(done: &Result<T, Error>) -> QString {
    done.as_ref().err().map_or_else(QString::default, said_of)
}

/// What was done, or why it was refused.
pub fn said_or_why(done: &Result<Sentence, Error>) -> QString {
    match done {
        Ok(sentence) => said(sentence),
        Err(problem) => said_of(problem),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstack_core::say_n;

    #[test]
    fn a_sentence_is_shown_in_the_words_set_and_english_before_any() {
        let counted = say_n!(0, "{} mod", "{} mods");
        assert_eq!(put(&counted), "0 mods");
        let table = serde_json::json!({ "@locale": "fr", "{} mod": "{} mod", "{} mods": "{} mods", "Play": "" });
        let words = Words::of(table.as_object().unwrap());
        assert_eq!(
            words.word("Play"),
            "Play",
            "an empty wording leaves the English"
        );
        show_in(words);
        assert_eq!(put(&counted), "0 mod");
        show_in(Words::default());
    }
}
