//! What GitHub says about a core's repository, for the marketplace's details sheet: the repository's own description, its licence and
//! topics, and how often the release files were downloaded. Judgement only (no network): the bytes come from `tau_net`. Everything is
//! plain text for the screen: the repository's words are never treated as markup.

use crate::{ErrorCode, TauError};
use serde_json::Value;

/// How long a release note is kept for the screen.
pub const NOTES_CHARS: usize = 1500;
/// The release list is asked for with this page size; a full page means there may be more releases than were counted.
pub const RELEASES_PER_PAGE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RepoInfo {
    /// The repository's one-line description, empty when it has none.
    pub description: String,
    /// The project's own web page, only when it is an https address.
    pub homepage: String,
    /// The licence's short name (`MIT`), empty when GitHub does not know it.
    pub license: String,
    pub stars: u64,
    pub topics: Vec<String>,
    /// `YYYY-MM-DD` of the last push, empty when unknown.
    pub pushed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Downloads {
    /// Every download of every release file (zips and anything else attached), counted by GitHub.
    pub total: u64,
    /// The newest release's downloads.
    pub newest: u64,
    pub newest_tag: String,
    /// How many releases were counted.
    pub releases: u32,
    /// False when the list was a full page: there may be older releases that were not counted, so the total is "at least".
    pub complete: bool,
    /// The newest release's notes, cut to [`NOTES_CHARS`], empty when it has none.
    pub notes: String,
}

fn text(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn day(s: &str) -> String {
    s.get(..10).unwrap_or_default().to_string()
}

/// From `GET /repos/<o>/<r>`.
pub fn parse_repo(bytes: &[u8]) -> Result<RepoInfo, TauError> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| {
        TauError::e(
            ErrorCode::Json,
            format!("GitHub's repository page could not be read: {e}"),
        )
    })?;
    if !v.is_object() {
        return Err(TauError::e(
            ErrorCode::Json,
            "GitHub's repository page could not be read.",
        ));
    }
    let homepage = text(&v, "homepage");
    Ok(RepoInfo {
        description: text(&v, "description"),
        homepage: if homepage.starts_with("https://") {
            homepage
        } else {
            String::new()
        },
        license: v
            .get("license")
            .map(|l| {
                let id = text(l, "spdx_id");
                if id.is_empty() || id == "NOASSERTION" {
                    text(l, "name")
                } else {
                    id
                }
            })
            .unwrap_or_default(),
        stars: v
            .get("stargazers_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        topics: v
            .get("topics")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .take(12)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        pushed: day(&text(&v, "pushed_at")),
    })
}

/// From `GET /repos/<o>/<r>/releases?per_page=100` (newest first). Drafts are not counted.
pub fn parse_downloads(bytes: &[u8]) -> Result<Downloads, TauError> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| {
        TauError::e(
            ErrorCode::Json,
            format!("GitHub's release list could not be read: {e}"),
        )
    })?;
    let list: Vec<&Value> = v
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|r| r.get("draft").and_then(Value::as_bool) != Some(true))
                .collect()
        })
        .unwrap_or_default();
    let count = |r: &Value| -> u64 {
        r.get("assets")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("download_count").and_then(Value::as_u64))
                    .sum()
            })
            .unwrap_or(0)
    };
    let mut d = Downloads {
        total: list.iter().map(|r| count(r)).sum(),
        releases: list.len() as u32,
        complete: v.as_array().map_or(0, Vec::len) < RELEASES_PER_PAGE,
        ..Downloads::default()
    };
    if let Some(first) = list.first() {
        d.newest = count(first);
        d.newest_tag = text(first, "tag_name");
        d.notes = text(first, "body").chars().take(NOTES_CHARS).collect();
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_page_gives_description_licence_topics_and_only_an_https_homepage() {
        let r = parse_repo(br#"{"description":" A gallery. ","homepage":"http://x.example","license":{"spdx_id":"MIT","name":"MIT License"},
            "stargazers_count":7,"topics":["analogue-pocket","openfpga"],"pushed_at":"2026-11-14T10:00:00Z"}"#).unwrap();
        assert_eq!(r.description, "A gallery.");
        assert_eq!(r.homepage, "", "a plain http address is not offered");
        assert_eq!(
            (r.license.as_str(), r.stars, r.pushed.as_str()),
            ("MIT", 7, "2026-11-14")
        );
        assert_eq!(r.topics, ["analogue-pocket", "openfpga"]);
        let none = parse_repo(br#"{"description":null,"license":null}"#).unwrap();
        assert_eq!((none.description.as_str(), none.license.as_str()), ("", ""));
        let other = parse_repo(br#"{"license":{"spdx_id":"NOASSERTION","name":"Other"}}"#).unwrap();
        assert_eq!(other.license, "Other");
        assert!(parse_repo(b"[]").is_err() && parse_repo(b"nope").is_err());
    }

    #[test]
    fn downloads_add_up_every_release_file_and_skip_drafts() {
        let d = parse_downloads(
            br#"[{"tag_name":"v2","body":"New things","assets":[{"download_count":10},{"download_count":5}]},
                 {"tag_name":"v1.1","draft":true,"assets":[{"download_count":1000}]},
                 {"tag_name":"v1","assets":[{"download_count":30},{"name":"no count"}]}]"#,
        )
        .unwrap();
        assert_eq!(
            (d.total, d.newest, d.releases, d.complete),
            (45, 15, 2, true)
        );
        assert_eq!(
            (d.newest_tag.as_str(), d.notes.as_str()),
            ("v2", "New things")
        );
    }

    #[test]
    fn a_full_page_of_releases_is_marked_incomplete_and_notes_are_cut() {
        let item = r#"{"tag_name":"t","assets":[{"download_count":1}]}"#;
        let json = format!("[{}]", vec![item; RELEASES_PER_PAGE].join(","));
        let d = parse_downloads(json.as_bytes()).unwrap();
        assert_eq!((d.total, d.complete), (RELEASES_PER_PAGE as u64, false));
        let long = format!(
            r#"[{{"tag_name":"t","body":"{}","assets":[]}}]"#,
            "x".repeat(5000)
        );
        assert_eq!(
            parse_downloads(long.as_bytes()).unwrap().notes.len(),
            NOTES_CHARS
        );
        assert_eq!(
            parse_downloads(b"[]").unwrap(),
            Downloads {
                complete: true,
                ..Downloads::default()
            }
        );
    }
}
