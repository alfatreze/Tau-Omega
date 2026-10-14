//! The openFPGA Library catalog (`https://openfpga-library.github.io/analogue-pocket/api/v1/cores.json`): parsing,
//! searching and filtering. Pure logic; the host fetches the bytes and caches them.
//!
//! The catalog is a hint, not a source of truth for what a download contains: it has no download URL and no checksum
//! (the package is the newest GitHub release of `repository`), and it may be stale. Parsing is tolerant (null
//! `release`, `sponsor`, `updaters`; unknown keys ignored) so a field the service adds never breaks the list.

use crate::{ErrorCode, TauError};
use serde_json::Value;

/// Where the catalog is read from (one GET, no identifiers).
pub const CATALOG_URL: &str =
    "https://openfpga-library.github.io/analogue-pocket/api/v1/cores.json";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Repository {
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sponsor {
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CatalogAsset {
    /// The `Assets/<platform>` folder name the core reads its files from.
    pub platform: String,
    /// The filename the core expects, when it is a fixed one (a BIOS), else empty.
    pub filename: String,
    pub extensions: Vec<String>,
    pub core_specific: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CatalogCore {
    /// `author.Name`, also the `Cores/<id>` folder name.
    pub id: String,
    pub author: String,
    /// The platform name shown to people.
    pub name: String,
    pub repository: Repository,
    pub requires_license: bool,
    /// The file the user must supply when `requires_license` (`beta.bin`).
    pub license_filename: String,
    pub sponsor: Sponsor,
    /// Older core ids this core replaces (the author renamed it).
    pub replaces: Vec<String>,
    pub version: String,
    pub release_date: String,
    pub category: String,
    pub manufacturer: String,
    pub year: u32,
    pub assets: Vec<CatalogAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Catalog {
    pub cores: Vec<CatalogCore>,
}

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Strings only; anything else is dropped.
fn strings(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

/// An id is `author.Name`: the folder it will be installed to, so no separators, no `..`, no empty parts.
pub fn is_safe_core_id(id: &str) -> bool {
    let ok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ' ');
    id.contains('.')
        && !id.starts_with('.')
        && !id.contains("..")
        && !id.ends_with('.')
        && id.len() <= 64
        && id.chars().all(ok)
}

pub fn parse(bytes: &[u8]) -> Result<Catalog, TauError> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| {
        TauError::e(
            ErrorCode::Json,
            format!("the core catalog is not valid JSON: {e}"),
        )
    })?;
    let list = json
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| TauError::e(ErrorCode::Json, "the core catalog has no list of cores"))?;
    let mut cores = Vec::new();
    for item in list {
        let id = text(item, "identifier");
        // A malformed or unsafe entry is skipped rather than failing the whole list.
        if !is_safe_core_id(&id) {
            continue;
        }
        let repo = item.get("repository").cloned().unwrap_or(Value::Null);
        if text(&repo, "platform") != "github"
            || text(&repo, "owner").is_empty()
            || text(&repo, "name").is_empty()
        {
            continue;
        }
        let release = item.get("release").cloned().unwrap_or(Value::Null);
        let platform = release.get("platform").cloned().unwrap_or(Value::Null);
        let sponsor = item.get("sponsor").cloned().unwrap_or(Value::Null);
        let mut links = strings(sponsor.get("github"));
        links.extend(strings(sponsor.get("custom")));
        if let Some(p) = sponsor.get("patreon").and_then(Value::as_str) {
            links.push(p.to_string());
        }
        links.retain(|l| l.starts_with("https://"));
        let updaters = item.get("updaters").cloned().unwrap_or(Value::Null);
        let replaces = updaters
            .get("previous")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| {
                Some(format!(
                    "{}.{}",
                    p.get("author")?.as_str()?,
                    p.get("shortname")?.as_str()?
                ))
            })
            .filter(|p| is_safe_core_id(p))
            .collect();
        let assets = release
            .get("assets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|a| CatalogAsset {
                platform: text(a, "platform"),
                filename: text(a, "filename"),
                extensions: strings(a.get("extensions")),
                core_specific: a
                    .get("core_specific")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
            .collect();
        cores.push(CatalogCore {
            author: id.split('.').next().unwrap_or_default().to_string(),
            name: text(item, "platform"),
            id,
            repository: Repository {
                owner: text(&repo, "owner"),
                name: text(&repo, "name"),
            },
            requires_license: item
                .get("requires_license")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            license_filename: text(updaters.get("license").unwrap_or(&Value::Null), "filename"),
            sponsor: Sponsor { links },
            replaces,
            version: text(&release, "version"),
            release_date: text(&release, "release_date"),
            category: text(&platform, "category"),
            manufacturer: text(&platform, "manufacturer"),
            year: platform
                .get("year")
                .and_then(Value::as_u64)
                .and_then(|y| u32::try_from(y).ok())
                .unwrap_or(0),
            assets,
        });
    }
    Ok(Catalog { cores })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub query: String,
    pub category: Option<String>,
    pub manufacturer: Option<String>,
    /// `Some(true)`: only cores that need a licence file; `Some(false)`: only those that do not.
    pub needs_license: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Newest,
    Name,
}

impl Catalog {
    pub fn find(&self, id: &str) -> Option<&CatalogCore> {
        self.cores.iter().find(|c| c.id.eq_ignore_ascii_case(id))
    }

    pub fn categories(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .cores
            .iter()
            .map(|c| c.category.clone())
            .filter(|c| !c.is_empty())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Every word of the query must appear in the id, name, author, manufacturer or category (case-insensitive).
    pub fn search(&self, filter: &Filter, sort: Sort) -> Vec<&CatalogCore> {
        let words: Vec<String> = filter
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut out: Vec<&CatalogCore> = self
            .cores
            .iter()
            .filter(|c| {
                let hay = format!(
                    "{} {} {} {} {}",
                    c.id, c.name, c.author, c.manufacturer, c.category
                )
                .to_lowercase();
                words.iter().all(|w| hay.contains(w))
                    && filter
                        .category
                        .as_ref()
                        .is_none_or(|x| x.eq_ignore_ascii_case(&c.category))
                    && filter
                        .manufacturer
                        .as_ref()
                        .is_none_or(|x| x.eq_ignore_ascii_case(&c.manufacturer))
                    && filter.needs_license.is_none_or(|n| n == c.requires_license)
            })
            .collect();
        match sort {
            Sort::Name => out.sort_by_key(|c| c.name.to_lowercase()),
            // ISO dates sort as text; newest first, undated last.
            Sort::Newest => out.sort_by(|a, b| {
                b.release_date
                    .cmp(&a.release_date)
                    .then_with(|| a.name.cmp(&b.name))
            }),
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &[u8] = include_bytes!("../../../testdata/openfpga/cores.json");

    #[test]
    fn parses_the_real_catalog_sample() {
        let c = parse(REAL).unwrap();
        assert_eq!(c.cores.len(), 43);
        let ard = c.find("agg23.arduboy").unwrap();
        assert_eq!(ard.id, "agg23.Arduboy");
        assert_eq!(
            ard.repository,
            Repository {
                owner: "agg23".into(),
                name: "openfpga-arduboy".into()
            }
        );
        assert_eq!(ard.version, "0.9.0");
        assert_eq!(ard.release_date, "2022-09-03");
        assert!(!ard.requires_license);
        assert!(
            c.cores
                .iter()
                .any(|x| x.requires_license && !x.license_filename.is_empty())
        );
    }

    #[test]
    fn search_and_filters() {
        let c = parse(REAL).unwrap();
        let none = c.search(
            &Filter {
                query: "zzzzzz".into(),
                ..Filter::default()
            },
            Sort::Name,
        );
        assert!(none.is_empty());
        let lic = c.search(
            &Filter {
                needs_license: Some(true),
                ..Filter::default()
            },
            Sort::Name,
        );
        assert!(!lic.is_empty() && lic.iter().all(|x| x.requires_license));
        let ard = c.search(
            &Filter {
                query: "AGG23 ardu".into(),
                ..Filter::default()
            },
            Sort::Name,
        );
        assert_eq!(ard.len(), 1);
        let newest = c.search(&Filter::default(), Sort::Newest);
        assert!(
            newest
                .windows(2)
                .all(|w| w[0].release_date >= w[1].release_date)
        );
        assert!(!c.categories().is_empty());
    }

    #[test]
    fn tolerant_of_missing_pieces_and_skips_unsafe_entries() {
        let json = br#"{"data":[
          {"identifier":"a.Ok","platform":"Ok","repository":{"platform":"github","owner":"a","name":"ok"}},
          {"identifier":"../evil.X","platform":"X","repository":{"platform":"github","owner":"a","name":"x"}},
          {"identifier":"b.Gitlab","platform":"G","repository":{"platform":"gitlab","owner":"b","name":"g"}},
          {"identifier":"c.Repl","platform":"R","repository":{"platform":"github","owner":"c","name":"r"},
           "sponsor":{"github":["https://github.com/sponsors/c","http://insecure"],"patreon":null},
           "updaters":{"previous":[{"shortname":"Old","author":"c","platform_id":"old"},{"shortname":"../x","author":"c"}]},
           "unknown_future_key":1}
        ]}"#;
        let c = parse(json).unwrap();
        assert_eq!(
            c.cores.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["a.Ok", "c.Repl"]
        );
        assert_eq!(c.cores[0].version, "");
        assert_eq!(c.cores[1].sponsor.links, ["https://github.com/sponsors/c"]);
        assert_eq!(c.cores[1].replaces, ["c.Old"]);
    }

    #[test]
    fn rejects_non_catalogs() {
        assert!(parse(b"not json").is_err());
        assert!(parse(br#"{"cores":[]}"#).is_err());
    }

    #[test]
    fn core_id_safety() {
        assert!(is_safe_core_id("agg23.Arduboy"));
        assert!(is_safe_core_id("alfatreze.TAU DEV 111"));
        for bad in [
            "",
            "x",
            "a/b.c",
            "..a.b",
            "a..b",
            ".hidden.x",
            "a.b/",
            "a\\b.c",
            "a.b\0",
        ] {
            assert!(!is_safe_core_id(bad), "{bad:?}");
        }
    }
}
