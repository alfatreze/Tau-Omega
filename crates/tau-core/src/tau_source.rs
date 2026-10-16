//! The Tau Alpha marketplace source: the cores published on `alfatreze/Tau-Alpha` (Stable releases and Preview pre-releases)
//! shown as catalog cores next to the openFPGA Library's. Pure logic over the already-parsed release list and manifests;
//! the host fetches them (`tau_net::tau_source`). Dev builds are never published, so the Dev core carries no download:
//! installing one is the general local path (a zip or folder through Add a core manually).
//!
//! A catalog core here is one core id (`alfatreze.TAU`, `alfatreze.TAU Preview`, their Diagnostics cores), its history is
//! that core's zips across the releases, newest first. A download address is the release asset's own, so the tag and zip
//! name can be read back from it ([`asset_of_url`]) for the verified download.

use crate::{
    catalog::{CatalogCore, InstallOption, ReleaseSummary, Repository},
    compat::{self, CompatDoc},
    release_check::{self, Channel, GithubRelease},
};
use std::collections::BTreeMap;

/// The GitHub account that publishes these cores (its profile picture is the fallback card image).
pub const OWNER: &str = "alfatreze";
/// The id of this source in [`CatalogCore::source`].
pub const SOURCE_ID: &str = "tau-alpha";
/// The placeholder core for local Dev builds (no download; installed from a local zip or folder).
pub const DEV_CORE_ID: &str = "alfatreze.TAU DEV";

/// The core id a release zip carries: the release manifest names it exactly; without one it is read from the zip name
/// (`alfatreze.TAU_Preview_0.6.0-preview.1_2026-10-09.zip` is `alfatreze.TAU Preview`).
pub fn core_id_of_zip(release: &GithubRelease, zip: &str, docs: &[CompatDoc]) -> Option<String> {
    if let Some(doc) = docs.iter().find(|d| d.release == release.tag)
        && let Some(p) = doc.packages.iter().find(|p| p.zip == zip)
    {
        return Some(p.core_id.clone());
    }
    let stem = zip.strip_suffix(".zip")?;
    let bytes = stem.as_bytes();
    // The first `_<digit>` ends the id (versions start with a digit; ids do not).
    let at = (1..bytes.len()).find(|&i| bytes[i - 1] == b'_' && bytes[i].is_ascii_digit())?;
    let id = &stem[..at - 1];
    let lower = id.to_ascii_lowercase();
    if !lower.starts_with("alfatreze.tau") {
        return None;
    }
    Some(
        if lower.contains("preview") || lower.ends_with("diagnostics") {
            id.replace('_', " ")
        } else {
            id.to_string()
        },
    )
}

fn channel_name(id: &str) -> &'static str {
    match release_check::channel_of_name(id) {
        Channel::Stable => "Stable",
        Channel::Preview => "Preview",
        Channel::Dev => "Dev",
    }
}

/// The tag and zip name of a download address built by [`catalog_cores`].
pub fn asset_of_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix(release_check::DOWNLOAD_PREFIX)?;
    let (tag, name) = rest.split_once('/')?;
    (!tag.is_empty() && name.ends_with(".zip") && !name.contains('/'))
        .then(|| (tag.to_string(), name.to_string()))
}

/// Whether `id` belongs to this source (a Tau core of any channel).
pub fn is_source_core(id: &str) -> bool {
    crate::core_update::is_tau_core(id)
}

/// The id of the single Tau card.
pub const CARD_ID: &str = "alfatreze.TAU";

/// The Tau cores of `releases` as ONE catalog core ("Tau") whose [`CatalogCore::options`] are the installable choices: Stable,
/// Preview and Dev, each standard or Diagnostics, every one with its releases newest first. The card's own version is the newest
/// Stable (else Preview) standard build.
pub fn catalog_cores(releases: &[GithubRelease], docs: &[CompatDoc]) -> Vec<CatalogCore> {
    let mut sorted: Vec<&GithubRelease> = releases.iter().collect();
    sorted.sort_by(|a, b| compat::compare_tags(&b.tag, &a.tag));
    let mut by_id: BTreeMap<String, Vec<ReleaseSummary>> = BTreeMap::new();
    for release in sorted {
        for asset in release.assets.iter().filter(|a| a.name.ends_with(".zip")) {
            let Some(id) = core_id_of_zip(release, &asset.name, docs) else {
                continue;
            };
            by_id.entry(id).or_default().push(ReleaseSummary {
                version: release.tag.trim_start_matches('v').to_string(),
                release_date: release.published.clone(),
                download_url: asset.url.clone(),
            });
        }
    }
    let option = |core_id: String, channel: &str, history: Vec<ReleaseSummary>| InstallOption {
        diagnostics: release_check::is_diagnostics_zip(&core_id),
        core_id,
        channel: channel.to_string(),
        history,
    };
    let mut options: Vec<InstallOption> = by_id
        .into_iter()
        .map(|(id, history)| {
            let channel = channel_name(&id);
            option(id, channel, history)
        })
        .collect();
    // Stable, then Preview; standard before Diagnostics; the Dev choice last (installed from a local file).
    options.sort_by_key(|o| (u8::from(o.channel != "Stable"), o.diagnostics));
    options.push(option(DEV_CORE_ID.to_string(), "Dev", Vec::new()));
    let newest = options
        .iter()
        .find(|o| !o.diagnostics && !o.history.is_empty())
        .and_then(|o| o.history.first().cloned())
        .unwrap_or_default();
    let mut card = base(CARD_ID, "");
    card.name = "Tau".into();
    card.version = newest.version;
    card.release_date = newest.release_date;
    card.download_url = newest.download_url;
    card.options = options;
    vec![card]
}

/// The install choice whose core folder is `core_id`, among the Tau card's options.
pub fn find_option<'a>(cards: &'a [CatalogCore], core_id: &str) -> Option<&'a InstallOption> {
    cards
        .iter()
        .flat_map(|c| &c.options)
        .find(|o| o.core_id.eq_ignore_ascii_case(core_id))
}

fn base(id: &str, channel: &str) -> CatalogCore {
    CatalogCore {
        id: id.to_string(),
        author: "alfatreze".into(),
        name: id.split_once('.').map_or(id, |(_, n)| n).to_string(),
        repository: Repository {
            owner: OWNER.into(),
            name: "Tau-Alpha".into(),
        },
        category: "Media Players".into(),
        manufacturer: "alfatreze".into(),
        platform_id: "tau".into(),
        source: SOURCE_ID.into(),
        channel: channel.into(),
        ..CatalogCore::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release_check::ReleaseAsset;

    fn asset(tag: &str, name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.into(),
            url: format!("{}{tag}/{name}", release_check::DOWNLOAD_PREFIX),
            size: 1,
        }
    }
    fn release(tag: &str, pre: bool, zips: &[&str]) -> GithubRelease {
        GithubRelease {
            tag: tag.into(),
            title: tag.into(),
            prerelease: pre,
            published: "2026-10-09".into(),
            assets: zips.iter().map(|z| asset(tag, z)).collect(),
        }
    }

    #[test]
    fn zip_names_give_core_ids_with_and_without_a_manifest() {
        let r = release("v0.6.0-preview.1", true, &[]);
        let id = |z| core_id_of_zip(&r, z, &[]);
        assert_eq!(
            id("alfatreze.TAU_Preview_0.6.0-preview.1_2026-10-09.zip").as_deref(),
            Some("alfatreze.TAU Preview")
        );
        assert_eq!(
            id("alfatreze.TAU_Preview_Diagnostics_0.6.0-preview.1_2026-10-09.zip").as_deref(),
            Some("alfatreze.TAU Preview Diagnostics")
        );
        assert_eq!(
            id("alfatreze.TAU_0.5.0_2026-09-27.zip").as_deref(),
            Some("alfatreze.TAU")
        );
        assert_eq!(
            id("alfatreze.TAU_DIAGNOSTIC_0.5.0_2026-09-27.zip").as_deref(),
            Some("alfatreze.TAU_DIAGNOSTIC")
        );
        assert_eq!(id("SHA256SUMS.txt"), None);
        assert_eq!(id("someone.Else_1.0.zip"), None);
    }

    #[test]
    fn one_tau_card_with_stable_preview_and_dev_choices_each_newest_first() {
        let releases = [
            release("v0.5.0", false, &["alfatreze.TAU_0.5.0_2026-09-27.zip"]),
            release(
                "v0.6.0-preview.1",
                true,
                &[
                    "alfatreze.TAU_Preview_0.6.0-preview.1_2026-10-09.zip",
                    "alfatreze.TAU_Preview_Diagnostics_0.6.0-preview.1_2026-10-09.zip",
                ],
            ),
            release(
                "v0.5.1",
                false,
                &[
                    "alfatreze.TAU_0.5.1_2026-10-16.zip",
                    "alfatreze.TAU_Diagnostics_0.5.1_2026-10-16.zip",
                ],
            ),
        ];
        let cards = catalog_cores(&releases, &[]);
        assert_eq!(cards.len(), 1);
        let card = &cards[0];
        assert_eq!(
            (card.id.as_str(), card.name.as_str(), card.version.as_str()),
            (CARD_ID, "Tau", "0.5.1")
        );
        let order: Vec<_> = card
            .options
            .iter()
            .map(|o| (o.channel.as_str(), o.diagnostics))
            .collect();
        assert_eq!(
            order,
            [
                ("Stable", false),
                ("Stable", true),
                ("Preview", false),
                ("Preview", true),
                ("Dev", false)
            ]
        );
        let stable = find_option(&cards, "alfatreze.TAU").unwrap();
        assert_eq!(
            stable
                .history
                .iter()
                .map(|h| h.version.as_str())
                .collect::<Vec<_>>(),
            ["0.5.1", "0.5.0"]
        );
        let dev = find_option(&cards, DEV_CORE_ID).unwrap();
        assert!(dev.history.is_empty());
        assert!(
            cards
                .iter()
                .all(|c| c.source == SOURCE_ID && is_source_core(&c.id))
        );
    }

    #[test]
    fn a_download_address_gives_back_its_tag_and_zip() {
        let url = format!(
            "{}v0.5.1/alfatreze.TAU_0.5.1_2026-10-16.zip",
            release_check::DOWNLOAD_PREFIX
        );
        assert_eq!(
            asset_of_url(&url),
            Some(("v0.5.1".into(), "alfatreze.TAU_0.5.1_2026-10-16.zip".into()))
        );
        assert_eq!(asset_of_url("https://example.com/v1/a.zip"), None);
        assert_eq!(
            asset_of_url(&format!("{}v1/x/../a.zip", release_check::DOWNLOAD_PREFIX)),
            None
        );
    }
}
