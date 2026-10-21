use crate::base::error::Error;
use crate::rimworld::about::is_workshop_id;
use crate::said;

use crate::base::vdf::{Token, tokens};
use rayon::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

const DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";

const PAGE_URL: &str = "https://steamcommunity.com/sharedfiles/filedetails/?id=";

/// `timeupdated` in `appworkshop_294100.acf`: seconds since 1970, by Workshop id.
pub fn installed_times(acf: &str) -> HashMap<String, u64> {
    installed(acf, "timeupdated")
}

pub fn installed_sizes(acf: &str) -> HashMap<String, u64> {
    installed(acf, "size")
}

fn installed(acf: &str, fact: &str) -> HashMap<String, u64> {
    let all = tokens(acf);
    let mut times = HashMap::new();
    let start = all
        .iter()
        .position(|t| matches!(t, Token::Text(s) if s == "WorkshopItemsInstalled"));
    let Some(start) = start else {
        return times;
    };
    // "<id>" { "key" "value" ... } repeated, inside one pair of braces.
    let mut depth = 0;
    let mut item: Option<&str> = None;
    let mut rest = all[start + 1..].iter();
    while let Some(token) = rest.next() {
        match token {
            Token::Open => depth += 1,
            Token::Close => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Token::Text(text) if depth == 1 => item = Some(text),
            Token::Text(key) => {
                let value = rest.next();
                if let (2, true, Some(id), Some(Token::Text(v))) = (depth, key == fact, item, value)
                    && let Ok(time) = v.parse()
                {
                    times.insert(id.to_string(), time);
                }
            }
        }
    }
    times
}

pub fn ids_in(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for piece in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let digits = |s: &str| -> String { s.chars().take_while(char::is_ascii_digit).collect() };
        let id = match piece.split_once("id=") {
            Some((_, after)) => digits(after),
            None if piece.chars().all(|c| c.is_ascii_digit()) => piece.to_string(),
            None => String::new(),
        };
        if (6..=12).contains(&id.len()) && !found.contains(&id) {
            found.push(id);
        }
    }
    found
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Details {
    pub id: String,
    pub title: String,
    /// Seconds since 1970.
    pub updated: u64,
    /// 0 when Steam does not say.
    pub size: u64,
}

pub fn parse_details(json: &str) -> Result<Vec<Details>, Error> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("Steam's answer is not JSON: {}", e)))?;
    let files = root
        .pointer("/response/publishedfiledetails")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::format(said!("Steam's answer has no file list")))?;
    let details = |file: &Value| -> Option<Details> {
        Some(Details {
            id: file.get("publishedfileid")?.as_str()?.to_string(),
            title: file.get("title")?.as_str()?.to_string(),
            updated: file.get("time_updated")?.as_u64()?,
            // Steam writes the size as text.
            size: file
                .get("file_size")
                .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()))
                .unwrap_or(0),
        })
    };
    Ok(files.iter().filter_map(details).collect())
}

/// The ids the API hides (`result` 9): removed, private or friends-only items, and unlisted
/// ones, which anyone with the link still has. An id missing from the answer is not one of
/// them: Steam was not asked of it.
pub fn hidden_in(json: &str) -> Result<Vec<String>, Error> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("Steam's answer is not JSON: {}", e)))?;
    let files = root
        .pointer("/response/publishedfiledetails")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::format(said!("Steam's answer has no file list")))?;
    Ok(files
        .iter()
        .filter(|file| file.get("result").and_then(Value::as_u64) == Some(9))
        .filter_map(|file| {
            file.get("publishedfileid")
                .and_then(Value::as_str)
                .map(String::from)
        })
        .collect())
}

/// Of the hidden ids, those whose public page is Steam's error page: an unlisted item's page
/// shows it. `page` gives a page's text, `None` when it was not had, which ends the asking:
/// an id not seen gone is not gone.
pub fn gone_among(hidden: &[String], mut page: impl FnMut(&str) -> Option<String>) -> Vec<String> {
    let mut gone = Vec::new();
    for id in hidden {
        let Some(text) = page(id) else {
            break;
        };
        if text.contains(r#"<div class="error_ctn">"#) {
            gone.push(id.clone());
        }
    }
    gone
}

fn page(agent: &ureq::Agent, id: &str) -> Option<String> {
    // steamcommunity.com answers HTTP 429 to a page asked with gzip, and serves it asked plain.
    let mut answer = agent
        .get(&format!("{PAGE_URL}{id}"))
        .header("accept-encoding", "identity")
        .call()
        .ok()?;
    answer.body_mut().read_to_string().ok()
}

/// Blocks on the network: one page after another, as steamcommunity.com refuses many at once
/// (HTTP 429).
pub fn gone_of(hidden: &[String]) -> Vec<String> {
    let agent = crate::base::web::agent(30);
    gone_among(hidden, |id| page(&agent, id))
}

/// What Steam said of the asked items, and which it hides.
#[derive(Debug, Default)]
pub struct Answered {
    pub items: Vec<Details>,
    /// Not proof of removal: see `hidden_in`.
    pub hidden: Vec<String>,
}

/// Blocks on the network. Steam is slow per item, so chunks of 100 go out in parallel.
pub fn fetch_details(ids: &[String]) -> Result<Answered, Error> {
    // Steam answers HTTP 400 to a whole request holding one id that is not a number above 0,
    // as a folder in the Workshop folder that is no item: such an id is not sent.
    let ids: Vec<&String> = ids.iter().filter(|id| is_workshop_id(id)).collect();
    let agent = crate::base::web::agent(120);
    let ask = |batch: &[&String]| -> Result<Answered, Error> {
        let mut form = vec![("itemcount".to_string(), batch.len().to_string())];
        for (i, id) in batch.iter().enumerate() {
            form.push((format!("publishedfileids[{i}]"), id.to_string()));
        }
        let pairs = form.iter().map(|(k, v)| (k.as_str(), v.as_str()));
        let mut response = agent
            .post(DETAILS_URL)
            .send_form(pairs)
            .map_err(crate::base::web::steam_failed)?;
        // Descriptions come along, so this is above the default limit.
        let mut body = response
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .reader();
        let mut text = String::new();
        std::io::Read::read_to_string(&mut body, &mut text)
            .map_err(crate::base::web::steam_failed)?;
        Ok(Answered {
            items: parse_details(&text)?,
            hidden: hidden_in(&text)?,
        })
    };
    let answers: Vec<Result<Answered, Error>> = ids.par_chunks(100).map(ask).collect();
    let mut all = Answered::default();
    for answer in answers {
        let answer = answer?;
        all.items.extend(answer.items);
        all.hidden.extend(answer.hidden);
    }
    Ok(all)
}

const COLLECTIONS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetCollectionDetails/v1/";

const QUERY_URL: &str = "https://api.steampowered.com/IPublishedFileService/QueryFiles/v1/";

/// The game's app id on Steam.
pub const RIMWORLD: &str = "294100";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub id: String,
    pub title: String,
    /// Seconds since 1970; 0 when Steam does not say.
    pub updated: u64,
    /// `None` when Steam does not say.
    pub subscriptions: Option<u64>,
}

/// A page of `IPublishedFileService/QueryFiles` search results.
pub fn parse_search(json: &str) -> Result<Vec<Found>, Error> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("Steam's answer is not JSON: {}", e)))?;
    let files = root
        .pointer("/response/publishedfiledetails")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::format(said!("Steam's answer has no file list")))?;
    let found = |file: &Value| -> Option<Found> {
        if file.get("result").and_then(Value::as_u64) != Some(1) {
            return None;
        }
        Some(Found {
            id: file.get("publishedfileid")?.as_str()?.to_string(),
            title: file.get("title")?.as_str()?.to_string(),
            updated: file
                .get("time_updated")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            subscriptions: file.get("lifetime_subscriptions").and_then(Value::as_u64),
        })
    };
    Ok(files.iter().filter_map(found).collect())
}

/// Blocks on the network. The key goes only to api.steampowered.com and is never logged.
/// `page` starts at 1.
pub fn query_files(key: &str, text: &str, page: usize) -> Result<Vec<Found>, Error> {
    let agent = crate::base::web::agent(60);
    let mut response = agent
        .get(QUERY_URL)
        .query_pairs([
            ("query_type", "12"),
            ("page", &page.to_string()),
            ("numperpage", "30"),
            ("creator_appid", RIMWORLD),
            ("appid", RIMWORLD),
            ("search_text", text),
            ("filetype", "0"),
            ("return_metadata", "1"),
        ])
        .header("x-webapi-key", key)
        .call()
        .map_err(crate::base::web::steam_failed)?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(crate::base::web::steam_failed)?;
    parse_search(&text)
}

pub fn parse_collections(json: &str) -> Result<Vec<(String, Vec<String>)>, Error> {
    let root: Value = serde_json::from_str(json)
        .map_err(|e| Error::format(said!("Steam's answer is not JSON: {}", e)))?;
    let all = root["response"]["collectiondetails"].as_array();
    let text = |v: &Value| v["publishedfileid"].as_str().map(String::from);
    let one = |c: &Value| {
        let children = c["children"].as_array();
        Some((
            text(c)?,
            children.map_or(Vec::new(), |list| list.iter().filter_map(text).collect()),
        ))
    };
    Ok(all
        .ok_or_else(|| Error::format(said!("Steam's answer has no collections")))?
        .iter()
        .filter_map(one)
        .collect())
}

/// Blocks on the network.
pub fn expand_collections(ids: &[String]) -> Result<Vec<String>, Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let agent = crate::base::web::agent(60);
    let mut form = vec![("collectioncount".to_string(), ids.len().to_string())];
    for (i, id) in ids.iter().enumerate() {
        form.push((format!("publishedfileids[{i}]"), id.clone()));
    }
    let failed = crate::base::web::steam_failed;
    let pairs = form.iter().map(|(k, v)| (k.as_str(), v.as_str()));
    let mut response = agent
        .post(COLLECTIONS_URL)
        .send_form(pairs)
        .map_err(failed)?;
    let text = response.body_mut().read_to_string().map_err(failed)?;
    let collections = parse_collections(&text)?;
    let mut all: Vec<String> = Vec::new();
    for id in ids {
        let items = collections
            .iter()
            .find(|(c, items)| c == id && !items.is_empty());
        let these = items.map_or(std::slice::from_ref(id), |(_, items)| items.as_slice());
        for item in these {
            if !all.contains(item) {
                all.push(item.clone());
            }
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACF: &str = r#""AppWorkshop"
{
	"appid"		"294100"
	"TimeLastUpdated"		"1791750395"
	"WorkshopItemsInstalled"
	{
		"705924057"
		{
			"size"		"1372735"
			"timeupdated"		"1711286985"
			"manifest"		"7900175110102980905"
		}
		"712141500"
		{
			"size"		"5310730"
			"timeupdated"		"1710519878"
		}
	}
	"WorkshopItemDetails"
	{
		"705924057"
		{
			"timeupdated"		"5"
		}
	}
}"#;

    #[test]
    fn installed_times_come_from_the_installed_block_only() {
        let times = installed_times(ACF);
        assert_eq!(times.len(), 2);
        assert_eq!(times["705924057"], 1711286985);
        assert_eq!(times["712141500"], 1710519878);
        assert!(installed_times("garbage {").is_empty());
        assert_eq!(installed_sizes(ACF)["705924057"], 1372735);
    }

    #[test]
    fn ids_are_taken_from_links_and_bare_numbers_once_each() {
        let pasted = "https://steamcommunity.com/sharedfiles/filedetails/?id=2009463077&searchtext=x\n\
                      818773962, 2009463077 hello 12 steam://url/CommunityFilePage/1 ?id=abc";
        assert_eq!(ids_in(pasted), ["2009463077", "818773962"]);
    }

    #[test]
    fn details_skip_items_steam_does_not_know() {
        let json = r#"{"response":{"result":1,"resultcount":2,"publishedfiledetails":[
            {"publishedfileid":"2009463077","result":1,"title":"Harmony","time_updated":1756914561,"file_size":"5729526"},
            {"publishedfileid":"1","result":9}]}}"#;
        let found = parse_details(json).unwrap();
        assert_eq!(
            found,
            [Details {
                id: "2009463077".into(),
                title: "Harmony".into(),
                updated: 1756914561,
                size: 5729526
            }]
        );
        assert!(parse_details("<html>").is_err());
        assert!(parse_details("{}").is_err());
    }

    #[test]
    fn a_hidden_item_is_gone_only_when_its_page_is_steams_error() {
        // The API answers both alike: 3288243760 is unlisted and live, 9999999999 no item.
        let json = r#"{"response":{"result":1,"resultcount":4,"publishedfiledetails":[
            {"publishedfileid":"2009463077","result":1,"title":"Harmony","time_updated":1756914561,"file_size":"5729526"},
            {"publishedfileid":"3288243760","result":9},
            {"publishedfileid":"9999999999","result":9},
            {"publishedfileid":"3811986740","result":2}]}}"#;
        let hidden = hidden_in(json).unwrap();
        assert_eq!(hidden, ["3288243760", "9999999999"]);
        assert!(hidden_in("{}").is_err());
        // Trimmed from what steamcommunity.com serves a visitor not logged in.
        let shown = "<title>Steam Workshop::Salvaging</title>\n\
            <div class=\"workshopItemTitle\">Salvaging</div>";
        let error = "<title>Steam Community :: Error</title>\n<div class=\"error_ctn\">\n\
            <h3>There was a problem accessing the item.  Please try again.</h3>";
        let pages = |id: &str| Some(if id == "3288243760" { shown } else { error }.to_string());
        assert_eq!(gone_among(&hidden, pages), ["9999999999"]);
        // A refused page (HTTP 429) ends the asking: the rest is not gone.
        let mut asked = 0;
        let refused = |_: &str| {
            asked += 1;
            None
        };
        assert!(gone_among(&hidden, refused).is_empty());
        assert_eq!(asked, 1);
    }

    #[test]
    fn search_results_skip_items_steam_does_not_know() {
        let json = r#"{"response":{"total":4123,"publishedfiledetails":[
            {"publishedfileid":"2009463077","result":1,"title":"Harmony","time_updated":1756914561,"lifetime_subscriptions":3181967},
            {"publishedfileid":"3298098166","result":1,"title":"No counts"},
            {"publishedfileid":"1","result":9}]}}"#;
        assert_eq!(
            parse_search(json).unwrap(),
            [
                Found {
                    id: "2009463077".into(),
                    title: "Harmony".into(),
                    updated: 1756914561,
                    subscriptions: Some(3181967),
                },
                Found {
                    id: "3298098166".into(),
                    title: "No counts".into(),
                    updated: 0,
                    subscriptions: None,
                },
            ]
        );
        assert!(parse_search("{}").is_err());
    }

    #[test]
    #[ignore = "needs the network and RIMSTACK_WORKSHOP: .../steamapps/workshop/content/294100"]
    fn real_workshop_folder_is_checked_for_updates() {
        let content = std::path::PathBuf::from(std::env::var_os("RIMSTACK_WORKSHOP").unwrap());
        let record = content
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("appworkshop_294100.acf");
        let on_disk = installed_times(&std::fs::read_to_string(record).unwrap());
        let ids: Vec<String> = std::fs::read_dir(&content)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        let started = std::time::Instant::now();
        let now = fetch_details(&ids).unwrap();
        println!(
            "asked {} answered {} in {:?}",
            ids.len(),
            now.items.len(),
            started.elapsed()
        );
        println!("{} hidden", now.hidden.len());
        let newer = |d: &&Details| on_disk.get(&d.id).is_some_and(|&have| d.updated > have);
        let behind: Vec<&Details> = now.items.iter().filter(newer).collect();
        println!("{} on record, {} behind", on_disk.len(), behind.len());
        for d in behind.iter().take(8) {
            println!("  {} have {} now {}", d.title, on_disk[&d.id], d.updated);
        }
        assert!(now.items.len() > ids.len() / 2);
    }

    #[test]
    #[ignore = "needs the network"]
    fn steam_answers_about_a_real_item() {
        let ids = ["2009463077", "1", "0", "", "2009463077 - Copy"].map(String::from);
        let found = fetch_details(&ids).unwrap();
        assert_eq!(found.items.len(), 1);
        assert_eq!(found.items[0].title, "Harmony");
    }

    #[test]
    #[ignore = "needs the network and RIMSTACK_IDS: Workshop ids or links"]
    fn real_hidden_items_are_gone_only_without_a_page() {
        let ids = ids_in(&std::env::var("RIMSTACK_IDS").unwrap());
        let hidden = fetch_details(&ids).unwrap().hidden;
        let agent = crate::base::web::agent(30);
        let mut read = Vec::new();
        let gone = gone_among(&hidden, |id| {
            let text = page(&agent, id);
            if text.is_some() {
                read.push(id.to_string());
            }
            text
        });
        println!(
            "asked {} hidden {} pages read {}: {read:?}",
            ids.len(),
            hidden.len(),
            read.len()
        );
        println!("gone {}: {gone:?}", gone.len());
    }

    #[test]
    fn a_collection_gives_its_items_and_a_mod_none() {
        let answer = r#"{"response":{"result":1,"resultcount":2,"collectiondetails":[
            {"publishedfileid":"111","result":1,"children":[{"publishedfileid":"5","sortorder":1,"filetype":0},{"publishedfileid":"6","sortorder":2,"filetype":0}]},
            {"publishedfileid":"222","result":9}]}}"#;
        let found = parse_collections(answer).unwrap();
        assert_eq!(
            found[0],
            ("111".to_string(), vec!["5".to_string(), "6".to_string()])
        );
        assert_eq!(found[1], ("222".to_string(), vec![]));
        assert!(parse_collections("{}").is_err());
    }
}
