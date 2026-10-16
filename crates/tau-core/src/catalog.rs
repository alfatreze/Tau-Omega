//! The openFPGA Library catalog (API v3: `https://openfpga-library.github.io/analogue-pocket/api/v3/cores.json` plus
//! `platforms.json`): parsing, searching and filtering. Pure logic; the host fetches the bytes and caches them.
//!
//! v3 lists every release of every core with its direct `download_url` and the compatibility data from the core's own
//! `core.json` (minimum Pocket firmware, Dock, link port), so nothing needs the GitHub API to find or vet a download. It
//! carries no checksum, size, description or art: the art is pre-rendered on the same site ([`CatalogCore::icon_url`]).
//! The list is newest-release-first (checked against the library's own "latest" for 332 cores). Parsing is tolerant
//! (null `updaters`, empty `releases`, unknown keys ignored) so a field the service adds never breaks the list, and an
//! entry whose id could not be a folder name is skipped.

use crate::{ErrorCode, TauError};
use serde_json::Value;

/// The pre-rendered art on the library site (the same pictures the cores carry): a 36x36 icon per core and a 521x165
/// banner per platform, plain grayscale PNGs. The JSON does not link them; the addresses follow from the ids.
pub const ART_BASE: &str = "https://openfpga-library.github.io/analogue-pocket/assets/images/";
/// The core list and the platform list (two GETs, no identifiers).
pub const CATALOG_URL: &str =
    "https://openfpga-library.github.io/analogue-pocket/api/v3/cores.json";
pub const PLATFORMS_URL: &str =
    "https://openfpga-library.github.io/analogue-pocket/api/v3/platforms.json";

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
    /// The `Assets/<platform>` folder name the slot reads from (`platform_ids[platform_index]`).
    pub platform: String,
    /// The filename the core expects, when it is a fixed one (a BIOS), else empty.
    pub filename: String,
    pub extensions: Vec<String>,
    pub core_specific: bool,
}

/// One published release (older ones are kept so a version can be chosen later).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReleaseSummary {
    pub version: String,
    pub release_date: String,
    pub download_url: String,
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
    pub sponsor: Sponsor,
    pub category: String,
    pub manufacturer: String,
    pub year: u32,
    /// The first platform id of the newest release: also the name of the platform banner.
    pub platform_id: String,
    // ---- the newest release, flattened ----
    pub version: String,
    pub release_date: String,
    /// Direct link to the newest release's zip; empty when the author has published nothing.
    pub download_url: String,
    pub requires_license: bool,
    /// The file the user must supply when `requires_license` (`beta.bin`).
    pub license_filename: String,
    /// Older core ids this core replaces (the author renamed it).
    pub replaces: Vec<String>,
    /// AI tools the author says they used (`updaters.declare_ai`), shown as provenance.
    pub declares_ai: Vec<String>,
    /// Minimum Pocket firmware (`framework.version_required`), empty when not stated.
    pub firmware_required: String,
    pub dock_supported: Option<bool>,
    pub link_port: Option<bool>,
    pub sleep_supported: Option<bool>,
    pub assets: Vec<CatalogAsset>,
    /// Every release, newest first (includes the newest).
    pub history: Vec<ReleaseSummary>,
    /// Which marketplace source lists this core ([`LIBRARY_SOURCE`], or another source's id such as `tau-alpha`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub source: String,
    /// The release channel of a core from a source that has them (`Stable`, `Preview`, `Dev`); empty otherwise.
    #[cfg_attr(feature = "serde", serde(default))]
    pub channel: String,
}

/// The id of the openFPGA Library source (the default marketplace source).
pub const LIBRARY_SOURCE: &str = "openfpga-library";

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

fn flag(v: &Value, pointer: &str) -> Option<bool> {
    v.pointer(pointer).and_then(Value::as_bool)
}

/// `cores_json` is `api/v3/cores.json`, `platforms_json` is `api/v3/platforms.json` (names, categories, makers, years).
pub fn parse(cores_json: &[u8], platforms_json: &[u8]) -> Result<Catalog, TauError> {
    let json = |bytes: &[u8], what: &str| -> Result<Value, TauError> {
        serde_json::from_slice(bytes).map_err(|e| {
            TauError::e(
                ErrorCode::Json,
                format!("the {what} is not valid JSON: {e}"),
            )
        })
    };
    let cores_value = json(cores_json, "core catalog")?;
    let list = cores_value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| TauError::e(ErrorCode::Json, "the core catalog has no list of cores"))?;
    let platforms_value = json(platforms_json, "platform list")?;
    let platforms: Vec<&Value> = platforms_value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    let platform = |id: &str| {
        platforms
            .iter()
            .find(|p| p.get("id").and_then(Value::as_str) == Some(id))
            .copied()
    };

    let mut cores = Vec::new();
    for item in list {
        let id = text(item, "id");
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
        let funding = repo.get("funding").cloned().unwrap_or(Value::Null);
        let mut links = strings(funding.get("github"));
        links.extend(strings(funding.get("custom")));
        for key in ["patreon", "ko_fi", "buy_me_a_coffee"] {
            if let Some(p) = funding.get(key).and_then(Value::as_str) {
                links.push(p.to_string());
            }
        }
        links.retain(|l| l.starts_with("https://"));

        // Newest first; entries without a usable `core` section are dropped (nothing to say about them).
        let releases: Vec<&Value> = item
            .get("releases")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|r| r.get("core").is_some_and(|c| !c.is_null()))
            .collect();
        let history: Vec<ReleaseSummary> = releases
            .iter()
            .map(|r| ReleaseSummary {
                version: r
                    .pointer("/core/metadata/version")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                release_date: r
                    .pointer("/core/metadata/date_release")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                download_url: text(r, "download_url"),
            })
            .collect();
        let latest = releases.first().copied().unwrap_or(&Value::Null);
        let platform_ids = strings(latest.pointer("/core/metadata/platform_ids"));
        let platform_id = platform_ids.first().cloned().unwrap_or_default();
        let plat = platform(&platform_id);
        let updaters = latest.get("updaters").cloned().unwrap_or(Value::Null);
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
        let assets = latest
            .pointer("/data/data_slots")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|slot| {
                let index = slot
                    .pointer("/parameters/platform_index")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                let extensions = strings(slot.get("extensions"));
                let filename = text(slot, "filename");
                (!extensions.is_empty() || !filename.is_empty()).then(|| CatalogAsset {
                    platform: platform_ids
                        .get(index)
                        .or(platform_ids.first())
                        .cloned()
                        .unwrap_or_default(),
                    filename,
                    extensions,
                    core_specific: flag(slot, "/parameters/core_specific_file").unwrap_or(false),
                })
            })
            .collect();
        cores.push(CatalogCore {
            author: id.split('.').next().unwrap_or_default().to_string(),
            name: plat
                .map(|p| text(p, "name"))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| {
                    id.split_once('.')
                        .map_or(id.clone(), |(_, n)| n.to_string())
                }),
            id,
            repository: Repository {
                owner: text(&repo, "owner"),
                name: text(&repo, "name"),
            },
            sponsor: Sponsor { links },
            category: plat.map(|p| text(p, "category")).unwrap_or_default(),
            manufacturer: plat.map(|p| text(p, "manufacturer")).unwrap_or_default(),
            year: plat
                .and_then(|p| p.get("year"))
                .and_then(Value::as_u64)
                .and_then(|y| u32::try_from(y).ok())
                .unwrap_or(0),
            platform_id,
            version: history
                .first()
                .map(|r| r.version.clone())
                .unwrap_or_default(),
            release_date: history
                .first()
                .map(|r| r.release_date.clone())
                .unwrap_or_default(),
            download_url: history
                .first()
                .map(|r| r.download_url.clone())
                .unwrap_or_default(),
            requires_license: latest
                .get("requires_license")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            license_filename: text(updaters.get("license").unwrap_or(&Value::Null), "filename"),
            replaces,
            declares_ai: strings(updaters.get("declare_ai")),
            firmware_required: latest
                .pointer("/core/framework/version_required")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            dock_supported: flag(latest, "/core/framework/dock/supported"),
            link_port: flag(latest, "/core/framework/hardware/link_port"),
            sleep_supported: flag(latest, "/core/framework/sleep_supported"),
            assets,
            source: LIBRARY_SOURCE.to_string(),
            channel: String::new(),
            history,
        });
    }
    Ok(Catalog { cores })
}

fn art_name_ok(name: &str, allow_upper: bool) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| {
            c.is_ascii_alphanumeric() && (allow_upper || !c.is_ascii_uppercase())
                || matches!(c, '_' | '-' | '.')
        })
        && !name.contains("..")
        && !name.starts_with('.')
}

impl CatalogCore {
    /// The 36x36 icon (`authors/<id>.png`), when the id is a plain file name (no spaces or odd characters).
    pub fn icon_url(&self) -> Option<String> {
        art_name_ok(&self.id, true).then(|| format!("{ART_BASE}authors/{}.png", self.id))
    }

    /// The 521x165 platform banner (`platforms/<platform>.png`).
    pub fn banner_url(&self) -> Option<String> {
        art_name_ok(&self.platform_id, false)
            .then(|| format!("{ART_BASE}platforms/{}.png", self.platform_id))
    }
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

    const CORES: &[u8] = include_bytes!("../../../testdata/openfpga/cores_v3.json");
    const PLATFORMS: &[u8] = include_bytes!("../../../testdata/openfpga/platforms_v3.json");

    fn real() -> Catalog {
        parse(CORES, PLATFORMS).unwrap()
    }

    #[test]
    fn parses_the_real_v3_sample() {
        let c = real();
        assert_eq!(c.cores.len(), 44);
        let ard = c.find("agg23.arduboy").unwrap();
        assert_eq!(ard.id, "agg23.Arduboy");
        assert_eq!(ard.name, "Arduboy");
        assert_eq!(
            ard.repository,
            Repository {
                owner: "agg23".into(),
                name: "openfpga-arduboy".into()
            }
        );
        assert_eq!(
            (ard.version.as_str(), ard.release_date.as_str()),
            ("0.9.0", "2022-09-03")
        );
        assert_eq!(
            ard.download_url,
            "https://github.com/agg23/openfpga-arduboy/releases/download/0.9.0/agg23.Arduboy.0.9.0.zip"
        );
        assert_eq!(
            (ard.category.as_str(), ard.manufacturer.as_str(), ard.year),
            ("Handheld", "Arduboy", 2016)
        );
        // compatibility is known before any download
        assert_eq!(
            (
                ard.firmware_required.as_str(),
                ard.dock_supported,
                ard.link_port,
                ard.sleep_supported
            ),
            ("1.1", Some(true), Some(false), Some(false))
        );
        assert_eq!(
            ard.assets,
            [CatalogAsset {
                platform: "arduboy".into(),
                filename: String::new(),
                extensions: vec!["hex".into()],
                core_specific: false
            }]
        );
        assert!(!ard.requires_license);
        assert!(
            c.cores
                .iter()
                .any(|x| x.requires_license && !x.license_filename.is_empty())
        );
    }

    #[test]
    fn the_newest_release_is_the_first_and_history_is_kept() {
        let c = real();
        let b = c.find("HarpMudd.Berzerk").unwrap();
        assert_eq!(b.version, "1.1.1");
        assert_eq!(
            b.history
                .iter()
                .map(|r| r.version.as_str())
                .collect::<Vec<_>>(),
            ["1.1.1", "1.1.0"]
        );
        assert!(
            b.history
                .iter()
                .all(|r| r.download_url.starts_with("https://github.com/"))
        );
    }

    #[test]
    fn a_core_with_no_release_is_listed_without_a_download() {
        let c = real();
        let t = c.find("Obsidian.PocketTyrian").unwrap();
        assert_eq!(
            (t.version.as_str(), t.download_url.as_str(), t.history.len()),
            ("", "", 0)
        );
    }

    #[test]
    fn search_and_filters() {
        let c = real();
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
          {"id":"a.Ok","repository":{"platform":"github","owner":"a","name":"ok"},"releases":[]},
          {"id":"../evil.X","repository":{"platform":"github","owner":"a","name":"x"},"releases":[]},
          {"id":"b.Gitlab","repository":{"platform":"gitlab","owner":"b","name":"g"},"releases":[]},
          {"id":"c.Repl","repository":{"platform":"github","owner":"c","name":"r","funding":{"github":["https://github.com/sponsors/c","http://insecure"],"ko_fi":"https://ko-fi.com/c","buy_me_a_coffee":"https://buymeacoffee.com/c","patreon":null}},
           "releases":[{"download_url":"https://github.com/c/r/releases/download/2/c.Repl.zip","requires_license":true,
             "core":{"metadata":{"platform_ids":["old","okplat"],"version":"2.0","date_release":"2026-02-01"},"framework":{"version_required":"2.2","dock":{"supported":false}}},
             "data":{"data_slots":[{"name":"BIOS","filename":"bios.bin","parameters":{"core_specific_file":true,"platform_index":1}},{"name":"x","parameters":{}}]},
             "updaters":{"license":{"filename":"beta.bin"},"declare_ai":["Claude"],"previous":[{"shortname":"Old","author":"c","platform_id":"old"},{"shortname":"../x","author":"c"}]}},
            {"download_url":"https://github.com/c/r/releases/download/1/c.Repl.zip","core":null}],
           "unknown_future_key":1}
        ]}"#;
        let c = parse(json, br#"{"data":[{"id":"old","name":"Old Name","category":"Console","manufacturer":"M","year":1990}]}"#).unwrap();
        assert_eq!(
            c.cores.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["a.Ok", "c.Repl"]
        );
        assert_eq!(
            (c.cores[0].version.as_str(), c.cores[0].name.as_str()),
            ("", "Ok")
        );
        let r = &c.cores[1];
        assert_eq!(
            r.sponsor.links,
            [
                "https://github.com/sponsors/c",
                "https://ko-fi.com/c",
                "https://buymeacoffee.com/c"
            ]
        );
        assert_eq!(r.replaces, ["c.Old"]);
        assert_eq!(
            (
                r.declares_ai.as_slice(),
                r.license_filename.as_str(),
                r.requires_license
            ),
            (["Claude".to_string()].as_slice(), "beta.bin", true)
        );
        assert_eq!(
            (r.firmware_required.as_str(), r.dock_supported, r.link_port),
            ("2.2", Some(false), None)
        );
        assert_eq!(
            (r.name.as_str(), r.category.as_str(), r.year),
            ("Old Name", "Console", 1990)
        );
        // the slot reads from the second platform id, and a null-core release is not part of the history
        assert_eq!(
            (
                r.assets[0].platform.as_str(),
                r.assets[0].core_specific,
                r.assets.len()
            ),
            ("okplat", true, 1)
        );
        assert_eq!(r.history.len(), 1);
    }

    #[test]
    fn art_addresses_follow_the_ids_and_refuse_odd_names() {
        let c = real();
        let ard = c.find("agg23.Arduboy").unwrap();
        assert_eq!(
            ard.icon_url().unwrap(),
            "https://openfpga-library.github.io/analogue-pocket/assets/images/authors/agg23.Arduboy.png"
        );
        assert_eq!(
            ard.banner_url().unwrap(),
            "https://openfpga-library.github.io/analogue-pocket/assets/images/platforms/arduboy.png"
        );
        let mut odd = ard.clone();
        odd.id = "a.B C".into();
        odd.platform_id = "../x".into();
        assert_eq!((odd.icon_url(), odd.banner_url()), (None, None));
        odd.platform_id = "Up".into();
        assert_eq!(odd.banner_url(), None);
        odd.platform_id = String::new();
        assert_eq!(odd.banner_url(), None);
    }

    #[test]
    fn rejects_non_catalogs() {
        assert!(parse(b"not json", PLATFORMS).is_err());
        assert!(parse(br#"{"cores":[]}"#, PLATFORMS).is_err());
        assert!(parse(CORES, b"nope").is_err());
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
