//! The user's own rules to the community rules repository, as a pull request on GitHub,
//! from the user's account with the user's token: a fork, a branch, the merged file, the
//! request. Everything but the network itself is pure and tested.

use crate::base::error::Error;
use crate::mods::community::{self, OwnRule};
use crate::said;
use serde_json::Value;
use std::time::Duration;

pub const API: &str = "https://api.github.com";

/// The community's, when the settings keep no `rulesRepo`.
pub const COMMUNITY: &str = "https://github.com/RimSort/Community-Rules-Database";

const FILE: &str = "communityRules.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

impl Repo {
    fn api(&self, path: &str) -> String {
        format!("{API}/repos/{}/{}{}", self.owner, self.name, path)
    }
}

/// A `github.com/owner/name` address, as the community's repositories are given;
/// `http://`, a trailing `/` or `.git` and a branch come with the URL in the settings.
pub fn repo_of(url: &str) -> Result<Repo, Error> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .ok_or_else(|| {
            Error::refused(said!(
                "the community rules repository is not a github.com address: {}",
                url
            ))
        })?;
    let mut parts = rest
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    match owner.is_empty() || name.is_empty() || parts.next().is_some() {
        true => Err(Error::refused(said!(
            "the community rules repository is no owner/name address: {}",
            url
        ))),
        false => Ok(Repo {
            owner: owner.to_string(),
            name: name.to_string(),
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
}

/// One REST call: the URL in full and the JSON body to send, if any.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub method: Method,
    pub url: String,
    pub body: String,
}

impl Call {
    fn new(method: Method, url: String, body: Option<Value>) -> Self {
        Self {
            method,
            url,
            body: body.map(|body| body.to_string()).unwrap_or_default(),
        }
    }

    pub fn fork(repo: &Repo) -> Self {
        Self::new(
            Method::Post,
            repo.api("/forks"),
            Some(serde_json::json!({})),
        )
    }

    pub fn repo(repo: &Repo) -> Self {
        Self::new(Method::Get, repo.api(""), None)
    }

    pub fn ref_sha(repo: &Repo, branch: &str) -> Self {
        Self::new(
            Method::Get,
            repo.api(&format!("/git/ref/heads/{branch}")),
            None,
        )
    }

    pub fn create_ref(repo: &Repo, branch: &str, sha: &str) -> Self {
        let body = serde_json::json!({ "ref": format!("refs/heads/{branch}"), "sha": sha });
        Self::new(Method::Post, repo.api("/git/refs"), Some(body))
    }

    pub fn file(repo: &Repo, branch: &str) -> Self {
        Self::new(
            Method::Get,
            repo.api(&format!("/contents/{FILE}?ref={branch}")),
            None,
        )
    }

    /// The raw download URL the file's answer names.
    pub fn raw(url: &str) -> Self {
        Self::new(Method::Get, url.to_string(), None)
    }

    pub fn write_file(repo: &Repo, branch: &str, text: &str, message: &str, sha: &str) -> Self {
        let body = serde_json::json!({
            "message": message,
            "content": base64(text),
            "branch": branch,
            "sha": sha,
        });
        Self::new(
            Method::Put,
            repo.api(&format!("/contents/{FILE}")),
            Some(body),
        )
    }

    pub fn open_request(
        repo: &Repo,
        branch: &str,
        head: &str,
        title: &str,
        body: &str,
        base: &str,
    ) -> Self {
        let body = serde_json::json!({
            "title": title,
            "body": body,
            "head": format!("{head}:{branch}"),
            "base": base,
        });
        Self::new(Method::Post, repo.api("/pulls"), Some(body))
    }

    /// The token's own login, to name the fork.
    pub fn who() -> Self {
        Self::new(Method::Get, format!("{API}/user"), None)
    }
}

/// base64 of the file's text, as the contents API takes it; standard, padded.
pub fn base64(text: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(bytes.len() / 3 * 4 + 4);
    for chunk in bytes.chunks(3) {
        let (a, b, c) = (chunk[0], chunk.get(1).copied(), chunk.get(2).copied());
        out.push(ALPHABET[(a >> 2) as usize] as char);
        out.push(ALPHABET[((a << 4 | b.unwrap_or(0) >> 4) & 0x3f) as usize] as char);
        out.push(match b {
            Some(b) => ALPHABET[((b << 2 | c.unwrap_or(0) >> 6) & 0x3f) as usize] as char,
            None => '=',
        });
        out.push(match c {
            Some(c) => ALPHABET[(c & 0x3f) as usize] as char,
            None => '=',
        });
    }
    out
}

fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let start = text.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = &text[start..];
    let body = rest.trim_start().strip_prefix(':')?.trim_start();
    if let Some(inner) = body.strip_prefix('"') {
        return inner.split('"').next();
    }
    Some(body.split([',', '}', ']']).next()?.trim())
}

/// What the API answered for the seeded values: the field or, failing, the message.
fn value<'a>(text: &'a str, key: &str) -> Result<&'a str, String> {
    field(text, key).ok_or_else(|| text.to_string())
}

pub fn answered_request(body: &str) -> Result<String, String> {
    value(body, "html_url").map(str::to_string)
}

pub fn answered_login(body: &str) -> Result<String, String> {
    value(body, "login").map(str::to_string)
}

pub fn answered_sha(body: &str) -> Result<String, String> {
    value(body, "sha").map(str::to_string)
}

/// The branch against which the pull request opens and from which the own branch starts.
pub fn answered_default_branch(body: &str) -> Result<String, String> {
    value(body, "default_branch").map(str::to_string)
}

/// The raw address to read the file's current text from.
pub fn answered_download_url(body: &str) -> Result<String, String> {
    value(body, "download_url").map(str::to_string)
}

/// A newer fork being built answers 404 to a ref read for a few seconds.
pub fn patient(status: u16) -> bool {
    status == 404
}

/// What a refused call means, from the status code.
pub fn said_of_status(status: u16, body: &str) -> Error {
    let detail = field(body, "message").unwrap_or_default();
    let said = match status {
        401 => said!(
            "GitHub did not take the token; check the token in the settings. {}",
            detail
        ),
        403 => said!("GitHub refused the token: {}", detail),
        404 => said!("GitHub has no repository there. {}", detail),
        422 => said!("GitHub would not write it: {}", detail),
        _ => said!("GitHub answered {}: {}", status, detail),
    };
    Error::network(said)
}

/// What came back from the network: the status and the body of the answer.
pub struct Reply {
    pub status: u16,
    pub body: String,
}

/// The pull request the publish opened.
#[derive(Debug, serde::Serialize)]
pub struct Published {
    pub url: String,
    pub rules: usize,
}

/// Sends the calls; `retry` buys a fork a moment to be built.
pub fn publish_with(
    send: impl Fn(&Call, &str) -> Result<Reply, Error>,
    mut retry: impl FnMut(Duration),
    token: &str,
    upstream: &Repo,
    branch: &str,
    own: &[OwnRule],
) -> Result<Published, Error> {
    if token.trim().is_empty() {
        return Err(Error::refused(say_no_token()));
    }
    let answer = |call: &Call| -> Result<Reply, Error> {
        let reply = send(call, token)?;
        match reply.status {
            200 | 201 => Ok(reply),
            202 if call.method == Method::Post => Ok(reply),
            status => Err(said_of_status(status, &reply.body)),
        }
    };
    let default = answered_default_branch(&answer(&Call::repo(upstream))?.body)
        .map_err(|body| Error::format(said!("the repository named no default branch: {}", body)))?;
    let login = answered_login(&answer(&Call::who())?.body)
        .map_err(|body| Error::network(said!("the token's account could not be read: {}", body)))?;
    let fork = Repo {
        owner: login,
        name: upstream.name.clone(),
    };
    // The fork, asked for before asking for anything of it, is built meanwhile: a fresh one
    // answers 404 to a ref read for a few seconds.
    answer(&Call::fork(upstream))?;
    let base_sha = {
        let mut tries = 0;
        loop {
            let reply = send(&Call::ref_sha(&fork, &default), token)?;
            match reply.status {
                200 => break answered_sha(&reply.body).map_err(Error::format),
                status if patient(status) && tries < 4 => {
                    tries += 1;
                    retry(Duration::from_secs(2));
                }
                status => return Err(said_of_status(status, &reply.body)),
            }
        }
    }?;
    // The branch before the merge, so what it writes against is fixed.
    answer(&Call::create_ref(&fork, branch, &base_sha))?;
    let file = answer(&Call::file(upstream, &default))?;
    let sha = answered_sha(&file.body).map_err(Error::format)?;
    let download = answered_download_url(&file.body).map_err(Error::format)?;
    let text = &answer(&Call::raw(&download))?.body;
    let merged = community::merge_own_rules(text, own)?;
    if merged.added.is_empty() {
        return Err(Error::refused(said!(
            "every own rule is already in the community rules"
        )));
    }
    let rules = merged.added.len();
    let title = format!(
        "Add {rules} load-order rule{}",
        if merged.added.len() == 1 { "" } else { "s" }
    );
    answer(&Call::write_file(&fork, branch, &merged.text, &title, &sha))?;
    let mut body = format!(
        "Adds {rules} rule{} made by @{}.",
        if merged.added.len() == 1 { "" } else { "s" },
        fork.owner
    );
    for rule in &merged.added {
        body.push_str(&format!("\n- `{}` loads after `{}`", rule.then, rule.first));
    }
    let opened = answer(&Call::open_request(
        upstream,
        branch,
        &fork.owner,
        &title,
        &body,
        &default,
    ))?;
    let url = answered_request(&opened.body).map_err(Error::format)?;
    Ok(Published { url, rules })
}

fn say_no_token() -> crate::base::sentence::Sentence {
    said!("no GitHub token kept; put one in the settings")
}

/// The branch a publish writes, named so two of them never meet: `rimstack-<seconds>`.
pub fn branch(now: i64) -> String {
    format!("rimstack-{now}")
}

fn headers<T>(request: ureq::RequestBuilder<T>, token: &str) -> ureq::RequestBuilder<T> {
    request
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "RimStack")
        .header("Accept", "application/vnd.github+json")
}

/// Blocks on the network, off any window thread. `now` is seconds since 1970.
pub fn publish(
    token: &str,
    upstream: &Repo,
    own: &[OwnRule],
    now: i64,
) -> Result<Published, Error> {
    let agent = crate::base::web::agent(60);
    let send = move |call: &Call, token: &str| -> Result<Reply, Error> {
        let mut response = match call.method {
            Method::Get => headers(agent.get(&call.url), token).call(),
            Method::Post => headers(agent.post(&call.url), token)
                .header("Content-Type", "application/json")
                .send(call.body.as_str()),
            Method::Put => headers(agent.put(&call.url), token)
                .header("Content-Type", "application/json")
                .send(call.body.as_str()),
        }
        .map_err(crate::base::web::failed)?;
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(crate::base::web::failed)?;
        Ok(Reply {
            status: response.status().as_u16(),
            body,
        })
    };
    publish_with(send, std::thread::sleep, token, upstream, &branch(now), own)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::locks::Held;

    const UPSTREAM_RULES: &str = "{\n    \"rules\": {\n        \"b.mod\": {\n            \"loadAfter\": {\n                \"a.mod\": {\n                    \"name\": [\n                        \"A mod\"\n                    ]\n                }\n            }\n        }\n    }\n}";

    fn repo() -> Repo {
        repo_of(COMMUNITY).unwrap()
    }

    fn own() -> Vec<OwnRule> {
        vec![OwnRule {
            first: "mama.loom".into(),
            then: "Whee.".into(),
            first_name: "Mama loom".into(),
        }]
    }

    #[test]
    fn a_repository_address_is_read_forgivingly() {
        let repo = repo();
        assert_eq!(repo.owner, "RimSort");
        assert_eq!(repo.name, "Community-Rules-Database");
        assert_eq!(
            repo_of("https://github.com/Me/my-repo.git").unwrap().name,
            "my-repo"
        );
        let fails = repo_of("https://elsewhere.org/me/repo")
            .unwrap_err()
            .to_string();
        assert!(fails.contains("not a github.com address"));
    }

    #[test]
    fn the_calls_ask_what_they_must() {
        let repo = repo();
        let fork = Repo {
            owner: "me".into(),
            name: repo.name.clone(),
        };
        assert_eq!(Call::fork(&repo).method, Method::Post);
        assert_eq!(
            Call::fork(&repo).url,
            "https://api.github.com/repos/RimSort/Community-Rules-Database/forks"
        );
        let created = Call::create_ref(&fork, "rimstack-1", "senabe0");
        assert_eq!(
            created.body,
            "{\"ref\":\"refs/heads/rimstack-1\",\"sha\":\"senabe0\"}"
        );
        assert_eq!(
            Call::ref_sha(&fork, "main").url,
            "https://api.github.com/repos/me/Community-Rules-Database/git/ref/heads/main"
        );
        let written = Call::write_file(&fork, "rimstack-1", "inn", "Message", "10b");
        let body: Value = serde_json::from_str(&written.body).unwrap();
        assert_eq!(base64("inn"), "aW5u");
        assert_eq!(body["content"], "aW5u");
        assert_eq!(body["branch"], "rimstack-1");
        assert_eq!(body["sha"], "10b");
        let opened = Call::open_request(&repo, "rimstack-1", "me", "Add 1 rule", "Body", "main");
        let body: Value = serde_json::from_str(&opened.body).unwrap();
        assert_eq!(body["head"], "me:rimstack-1");
        assert_eq!(body["base"], "main");
    }

    #[test]
    fn base64_pads_as_github_decodes_it() {
        assert_eq!(base64(""), "");
        assert_eq!(base64("f"), "Zg==");
        assert_eq!(base64("fo"), "Zm8=");
        assert_eq!(base64("foo"), "Zm9v");
        assert_eq!(base64("foob"), "Zm9vYg==");
        assert_eq!(base64("fooba"), "Zm9vYmE=");
        assert_eq!(base64("foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_answers_are_read() {
        assert_eq!(answered_login("{\"login\": \"me\"}").unwrap(), "me");
        assert_eq!(
            answered_sha("{\"object\": {\"sha\": \"senabe0\"}}").unwrap(),
            "senabe0"
        );
        assert_eq!(
            answered_default_branch("{\"default_branch\": \"main\"}").unwrap(),
            "main"
        );
        assert_eq!(
            answered_download_url("{\"download_url\": \"https://raw/now\"}").unwrap(),
            "https://raw/now"
        );
        assert_eq!(
            answered_request("{\"html_url\": \"https://github.com/p/r/pull/2\"}").unwrap(),
            "https://github.com/p/r/pull/2"
        );
    }

    #[test]
    fn a_refusal_names_what_github_said() {
        let bad = said_of_status(401, "{\"message\": \"Bad credentials\"}").to_string();
        assert!(
            bad.contains("did not take the token") && bad.contains("Bad credentials"),
            "{bad}"
        );
        let missing = said_of_status(404, "{}").to_string();
        assert!(missing.contains("no repository"), "{missing}");
        let other = said_of_status(500, "{\"message\": \"On fire\"}").to_string();
        assert!(other.contains("500"), "{other}");
    }

    type Asked = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

    fn answering<'a>(
        asked: Asked,
        answers: &'a [(u16, &'a str)],
    ) -> impl Fn(&Call, &str) -> Result<Reply, Error> + 'a {
        move |call: &Call, _token: &str| {
            let at = asked.held().len();
            let (status, body) = answers
                .get(at)
                .copied()
                .unwrap_or((500, "{\"message\": \"too many calls\"}"));
            let method = match call.method {
                Method::Get => "GET",
                Method::Post => "POST",
                Method::Put => "PUT",
            };
            asked
                .held()
                .push(format!("{method} {} {}", call.url, call.body));
            Ok(Reply {
                status,
                body: body.to_string(),
            })
        }
    }

    #[test]
    fn a_publish_forks_writes_and_opens_a_request() {
        let asked: Asked = std::sync::Arc::default();
        let send = answering(
            std::sync::Arc::clone(&asked),
            &[
                (200, "{\"default_branch\": \"main\"}"),
                (200, "{\"login\": \"me\"}"),
                (
                    202,
                    "{\"html_url\": \"https://github.com/me/Community-Rules-Database\"}",
                ),
                (404, "{\"message\": \"not yet\"}"),
                (200, "{\"object\": {\"sha\": \"senabe0\"}}"),
                (201, "{}"),
                (
                    200,
                    "{\"sha\": \"10b\", \"download_url\": \"https://raw/now\"}",
                ),
                (200, UPSTREAM_RULES),
                (200, "{}"),
                (
                    201,
                    "{\"html_url\": \"https://github.com/RimSort/Community-Rules-Database/pull/2\"}",
                ),
            ],
        );
        let mut retries = 0;
        let published = publish_with(
            |call, token| send(call, token),
            |_| retries += 1,
            "token",
            &repo(),
            "rimstack-1",
            &own(),
        )
        .unwrap();
        assert_eq!(retries, 1);
        assert_eq!(published.rules, 1);
        assert_eq!(
            published.url,
            "https://github.com/RimSort/Community-Rules-Database/pull/2"
        );
        let asked = asked.held();
        assert_eq!(asked.len(), 10);
        let written = &asked[8];
        assert!(
            written.starts_with("PUT ")
                && written.contains("\"sha\":\"10b\"")
                && written.contains("me/Community-Rules-Database/contents"),
            "{written}"
        );
        assert!(
            asked[9].contains("\"head\":\"me:rimstack-1\"")
                && asked[9].contains("whee.` loads after `mama.loom`"),
            "{}",
            asked[9]
        );
    }

    #[test]
    fn without_a_token_or_a_rule_nothing_is_sent() {
        let send = answering(std::sync::Arc::new(std::sync::Mutex::new(Vec::new())), &[]);
        let refused = publish_with(
            |call, token| send(call, token),
            |_| {},
            "",
            &repo(),
            "b",
            &own(),
        )
        .unwrap_err()
        .to_string();
        assert!(refused.contains("no GitHub token"));
        let asked: Asked = std::sync::Arc::default();
        let send = answering(
            std::sync::Arc::clone(&asked),
            &[
                (200, "{\"default_branch\": \"main\"}"),
                (200, "{\"login\": \"me\"}"),
                (202, "{}"),
                (200, "{\"object\": {\"sha\": \"senabe0\"}}"),
                (201, "{}"),
                (
                    200,
                    "{\"sha\": \"10b\", \"download_url\": \"https://raw/now\"}",
                ),
                (200, UPSTREAM_RULES),
            ],
        );
        let already = publish_with(
            |call, token| send(call, token),
            |_| {},
            "token",
            &repo(),
            "b",
            &[OwnRule {
                first: "a.mod".into(),
                then: "b.mod".into(),
                first_name: "A".into(),
            }],
        )
        .unwrap_err();
        assert!(
            already
                .to_string()
                .contains("already in the community rules")
        );
        assert_eq!(asked.held().len(), 7);
    }
}
