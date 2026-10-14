//! Comparing the cores installed on a card with the catalog: which have a newer release, which are unknown.
//! Pure logic. Tau's own cores are left to the Tau update path (`release_check`) and reported as [`Status::Excluded`].

use crate::{
    card::Core,
    catalog::Catalog,
    core_version::{Verdict, compare},
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Status {
    UpToDate,
    UpdateAvailable,
    /// The card has a newer version than the catalog lists.
    NewerThanCatalog,
    /// Same text expected but the versions cannot be ordered with confidence.
    Unknown,
    NotInCatalog,
    /// The catalog lists a replacement under a new id; the old core stays until the user removes it.
    ReplacedBy(String),
    /// Handled elsewhere (Tau's own cores).
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoreStatus {
    pub id: String,
    pub installed_version: String,
    /// The catalog's newest version (of this core, or of its replacement), empty when not listed.
    pub latest_version: String,
    pub release_date: String,
    pub status: Status,
}

/// Tau's cores (`alfatreze.TAU...`) use their own release channels.
pub fn is_tau_core(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l == "alfatreze.tau" || l.starts_with("alfatreze.tau ") || l.starts_with("alfatreze.tau_")
}

pub fn check(installed: &[Core], catalog: &Catalog) -> Vec<CoreStatus> {
    installed
        .iter()
        .map(|core| {
            let mk = |latest: &str, date: &str, status| CoreStatus {
                id: core.id.clone(),
                installed_version: core.version.clone(),
                latest_version: latest.to_string(),
                release_date: date.to_string(),
                status,
            };
            if is_tau_core(&core.id) {
                return mk("", "", Status::Excluded);
            }
            if let Some(c) = catalog.find(&core.id) {
                let status = if c.version.is_empty() {
                    Status::Unknown
                } else {
                    match compare(&core.version, &c.version) {
                        Verdict::Newer => Status::UpdateAvailable,
                        Verdict::Same => Status::UpToDate,
                        Verdict::Older => Status::NewerThanCatalog,
                        Verdict::Unknown => Status::Unknown,
                    }
                };
                return mk(&c.version, &c.release_date, status);
            }
            match catalog
                .cores
                .iter()
                .find(|c| c.replaces.iter().any(|r| r.eq_ignore_ascii_case(&core.id)))
            {
                Some(c) => mk(
                    &c.version,
                    &c.release_date,
                    Status::ReplacedBy(c.id.clone()),
                ),
                None => mk("", "", Status::NotInCatalog),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn core(id: &str, version: &str) -> Core {
        Core {
            id: id.to_string(),
            author: id.split('.').next().unwrap_or_default().to_string(),
            shortname: String::new(),
            version: version.to_string(),
            platform: String::new(),
            platforms: Vec::new(),
            media_platform: String::new(),
            library_capable: false,
            index_status: crate::card::IndexStatus::NoIndex,
            platform_category: None,
        }
    }

    fn cat() -> Catalog {
        let release = |v: &str, d: &str| {
            format!(
                r#"{{"download_url":"https://github.com/x/y/releases/download/{v}/z.zip","core":{{"metadata":{{"platform_ids":["p"],"version":"{v}","date_release":"{d}"}}}}}}"#
            )
        };
        let entry = |id: &str, owner: &str, releases: &str, updaters: &str| {
            format!(
                r#"{{"id":"{id}","repository":{{"platform":"github","owner":"{owner}","name":"y"}},"releases":[{releases}]{updaters}}}"#
            )
        };
        // `updaters` lives inside the release in v3; build the replacement entry by hand.
        let new_core = r#"{"id":"a.New","repository":{"platform":"github","owner":"a","name":"n"},"releases":[{"download_url":"https://github.com/a/n/releases/download/2.0/z.zip","core":{"metadata":{"platform_ids":["p"],"version":"2.0","date_release":"2026-02-01"}},"updaters":{"previous":[{"shortname":"Old","author":"a"}]}}]}"#;
        let json = format!(
            r#"{{"data":[{new_core},{},{},{}]}}"#,
            entry("b.Same", "b", &release("1.0", "2025-01-01"), ""),
            entry("c.Odd", "c", &release("final", "2025-01-01"), ""),
            entry("d.None", "d", "", "")
        );
        catalog::parse(json.as_bytes(), br#"{"data":[]}"#).unwrap()
    }

    #[test]
    fn verdicts() {
        let installed = [
            core("a.New", "1.5"),
            core("b.same", "1.0"),
            core("b.Same ", "9.9"),
            core("c.Odd", "1.0"),
            core("d.None", "1.0"),
            core("a.Old", "1.0"),
            core("zz.Unknown", "1.0"),
            core("alfatreze.TAU Preview", "0.6.0-preview.1"),
        ];
        let s: Vec<Status> = check(&installed, &cat())
            .into_iter()
            .map(|c| c.status)
            .collect();
        assert_eq!(
            s,
            [
                Status::UpdateAvailable,
                Status::UpToDate,
                Status::NotInCatalog,
                Status::Unknown,
                Status::Unknown,
                Status::ReplacedBy("a.New".into()),
                Status::NotInCatalog,
                Status::Excluded,
            ]
        );
        assert_eq!(
            check(&[core("b.Same", "9.9")], &cat())[0].status,
            Status::NewerThanCatalog
        );
    }

    #[test]
    fn tau_detection() {
        for id in [
            "alfatreze.TAU",
            "alfatreze.TAU Preview",
            "alfatreze.TAU_DEV_105",
            "alfatreze.TAU DEV 111",
        ] {
            assert!(is_tau_core(id), "{id}");
        }
        assert!(!is_tau_core("alfatreze.Tauntaun") && !is_tau_core("agg23.NES"));
    }
}
