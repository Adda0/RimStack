use crate::base::error::Error;

const REPO: &str = "https://api.github.com/repos/TheForgotten69/RimStack/releases";
const NIGHTLY_TAG: &str = "beta";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Stable: without the tag's leading `v`. Nightly: `nightly` and the short commit.
    pub version: String,
    pub page: String,
}

/// The program that is running: its commit, and when that commit was made (Unix seconds).
/// Without a commit, a nightly is never offered.
#[derive(Debug, Clone, Copy)]
pub struct Running<'a> {
    pub commit: &'a str,
    pub made: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Nightly,
}

/// The highest numbered release that is neither a draft nor a prerelease. A tag that is
/// not a number, such as the rolling `beta`, is never one.
pub fn stable_from(json: &str) -> Option<Release> {
    let all: serde_json::Value = serde_json::from_str(json).ok()?;
    let mut best: Option<(Vec<u64>, Release)> = None;
    for item in all.as_array()? {
        let flagged = |key| item[key].as_bool().unwrap_or(false);
        let tag = item["tag_name"].as_str().unwrap_or_default();
        let version = tag.trim_start_matches('v');
        if flagged("draft")
            || flagged("prerelease")
            || !version.starts_with(|c: char| c.is_ascii_digit())
        {
            continue;
        }
        let Some(page) = item["html_url"].as_str() else {
            continue;
        };
        let numbers = numbers(version);
        if best.as_ref().is_none_or(|(held, _)| numbers > *held) {
            best = Some((
                numbers,
                Release {
                    version: version.to_string(),
                    page: page.to_string(),
                },
            ));
        }
    }
    best.map(|(_, release)| release)
}

/// The rolling release's notes say `Built at <commit> on <YYYY-MM-DD HH:MM> UTC.` and, for
/// each system, `- <system>: built from <commit>` (written by `packaging/release.sh`).
/// The system's own line wins: a system whose build failed keeps its older files.
pub fn nightly_from(json: &str, system: &str, running: Running) -> Option<Release> {
    let answer: serde_json::Value = serde_json::from_str(json).ok()?;
    let notes = answer["body"].as_str()?;
    let after = |marker: &str| notes.split_once(marker).map(|(_, rest)| rest);
    let header = after("Built at ")?;
    let (commit, made) = header.split_once(" on ")?;
    let made = unix_seconds(made.get(..16)?)?;
    let own = after(&format!("- {system}: built from "))
        .map(|rest| {
            rest.split(|c: char| !c.is_ascii_hexdigit())
                .next()
                .unwrap_or_default()
        })
        .filter(|commit| !commit.is_empty());
    let commit = own.unwrap_or(commit.trim());
    let same = |a: &str, b: &str| a.starts_with(b) || b.starts_with(a);
    if running.commit.is_empty() || commit.is_empty() || same(commit, running.commit) {
        return None;
    }
    if made <= running.made {
        return None;
    }
    Some(Release {
        version: format!("nightly {}", commit.get(..7).unwrap_or(commit)),
        page: answer["html_url"].as_str()?.to_string(),
    })
}

fn numbers(version: &str) -> Vec<u64> {
    let version = version.trim_start_matches('v');
    let core = version.split(['-', '+']).next().unwrap_or_default();
    core.split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// Compared number by number, not as text: 0.10.0 is later than 0.9.3.
/// What is not a number counts as 0.
pub fn newer(current: &str, latest: &str) -> bool {
    let (mut now, mut then) = (numbers(current), numbers(latest));
    let parts = now.len().max(then.len());
    now.resize(parts, 0);
    then.resize(parts, 0);
    then > now
}

/// `YYYY-MM-DD HH:MM`, read as UTC.
pub fn unix_seconds(text: &str) -> Option<i64> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute) = (number(11..13)?, number(14..16)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let of_year = year.rem_euclid(400);
    let of_month = (month + 9) % 12;
    let of_day = (153 * of_month + 2) / 5 + day - 1;
    let of_era = of_year * 365 + of_year / 4 - of_year / 100 + of_day;
    let days = era * 146_097 + of_era - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60)
}

/// None where nothing newer is published. Blocks on the network.
pub fn check(channel: Channel, system: &str, running: Running) -> Result<Option<Release>, Error> {
    let url = match channel {
        Channel::Stable => format!("{REPO}?per_page=30"),
        Channel::Nightly => format!("{REPO}/tags/{NIGHTLY_TAG}"),
    };
    let asked = crate::base::web::agent(15)
        .get(&url)
        .header("User-Agent", "RimStack")
        .header("Accept", "application/vnd.github+json")
        .call();
    let body = match asked {
        Ok(mut answer) => answer
            .body_mut()
            .read_to_string()
            .map_err(crate::base::web::failed)?,
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(problem) => return Err(crate::base::web::failed(problem)),
    };
    Ok(match channel {
        Channel::Stable => stable_from(&body),
        Channel::Nightly => nightly_from(&body, system, running),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTES: &str = "Built at e963c8a on 2026-10-15 01:09 UTC.\n\n- linux: built from e963c8a\n- windows: built from 1111111, not rebuilt from e963c8a (its build failed)\n- macos: built from e963c8a\n";

    fn nightly(notes: &str) -> String {
        serde_json::json!({ "body": notes, "html_url": "https://github.com/o/r/releases/tag/beta" })
            .to_string()
    }

    fn running(commit: &str, made: &str) -> Running<'static> {
        Running {
            commit: Box::leak(commit.to_string().into_boxed_str()),
            made: unix_seconds(made).unwrap(),
        }
    }

    #[test]
    fn a_later_version_is_told_from_an_earlier_one_by_its_numbers() {
        assert!(newer("0.9.3", "0.10.0") && newer("1.2", "v1.2.1") && newer("0.0.0", "0.1.0-beta"));
        assert!(!newer("1.2.0", "1.2") && !newer("2.0.0", "1.9.9") && !newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn stable_is_the_highest_number_that_is_not_a_draft_a_prerelease_or_the_rolling_beta() {
        let list = r#"[
            {"tag_name":"beta","prerelease":true,"html_url":"https://x/beta"},
            {"tag_name":"v0.10.0-beta.1","prerelease":true,"html_url":"https://x/b1"},
            {"tag_name":"v0.9.3","html_url":"https://x/093"},
            {"tag_name":"v0.9.10","html_url":"https://x/0910"},
            {"tag_name":"v1.0.0","draft":true,"html_url":"https://x/draft"}
        ]"#;
        let found = stable_from(list).unwrap();
        assert_eq!(
            (found.version.as_str(), found.page.as_str()),
            ("0.9.10", "https://x/0910")
        );
        let only_beta = r#"[{"tag_name":"beta","prerelease":true,"html_url":"https://x/beta"}]"#;
        assert_eq!(stable_from(only_beta), None);
        assert_eq!(stable_from(r#"{"message":"Not Found"}"#), None);
    }

    #[test]
    fn a_nightly_is_newer_when_its_commit_differs_and_was_built_after_this_one() {
        let json = nightly(NOTES);
        let older = running("a1b2c3d", "2026-10-14 12:00");
        let found = nightly_from(&json, "linux", older).unwrap();
        assert_eq!(found.version, "nightly e963c8a");
        assert_eq!(
            nightly_from(&json, "linux", running("e963c8a1234", "2026-10-14 12:00")),
            None
        );
        assert_eq!(
            nightly_from(&json, "linux", running("a1b2c3d", "2026-10-16 09:00")),
            None
        );
        assert_eq!(
            nightly_from(&json, "linux", running("", "2026-10-14 12:00")),
            None
        );
    }

    #[test]
    fn a_nightly_is_read_for_the_system_that_asks() {
        let json = nightly(NOTES);
        let older = running("a1b2c3d", "2026-10-14 12:00");
        assert_eq!(
            nightly_from(&json, "windows", older).unwrap().version,
            "nightly 1111111"
        );
        assert_eq!(
            nightly_from(&json, "freebsd", older).unwrap().version,
            "nightly e963c8a"
        );
        assert_eq!(nightly_from(&nightly("nothing here"), "linux", older), None);
        assert_eq!(nightly_from("not json", "linux", older), None);
    }

    #[test]
    fn a_date_reads_as_unix_seconds() {
        assert_eq!(unix_seconds("1970-01-01 00:00"), Some(0));
        assert_eq!(unix_seconds("2026-10-15 01:09"), Some(1_792_026_540));
        assert_eq!(unix_seconds("2026-13-15 01:09"), None);
    }
}
