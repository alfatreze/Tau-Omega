//! Where each manually added core came from, kept in Omega's own app-data folder (never on the card): the only way a GitHub-sourced core can be
//! checked for new releases later, and the record that tells "the same source again" (an update) from "a different author's core with the same id".
//! The file is advice, not authority: a missing or damaged file just means "unknown source", never an error.

#[cfg(feature = "serde")]
use crate::{ErrorCode, TauError};
#[cfg(feature = "serde")]
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ManualEntry {
    pub core_id: String,
    /// `github`, `zip` or `folder`.
    pub kind: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub owner: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub repo: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub tag: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub asset: String,
    pub sha256: String,
    /// Unix seconds.
    pub added: u64,
    /// The card's name when it was added (display only).
    #[cfg_attr(feature = "serde", serde(default))]
    pub card: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Registry {
    #[cfg_attr(feature = "serde", serde(default))]
    pub entries: Vec<ManualEntry>,
}

impl Registry {
    pub fn get(&self, core_id: &str) -> Option<&ManualEntry> {
        self.entries
            .iter()
            .find(|e| e.core_id.eq_ignore_ascii_case(core_id))
    }

    /// Records (or replaces) the entry for a core.
    pub fn upsert(&mut self, entry: ManualEntry) {
        self.entries
            .retain(|e| !e.core_id.eq_ignore_ascii_case(&entry.core_id));
        self.entries.push(entry);
        self.entries.sort_by_key(|e| e.core_id.to_lowercase());
    }

    /// Forgets a core (it is now in the Library, or was removed).
    pub fn retire(&mut self, core_id: &str) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|e| !e.core_id.eq_ignore_ascii_case(core_id));
        self.entries.len() != before
    }

    /// The cores whose source is a GitHub repository (the ones that can be checked for updates).
    pub fn github(&self) -> impl Iterator<Item = &ManualEntry> {
        self.entries
            .iter()
            .filter(|e| e.kind == "github" && !e.owner.is_empty() && !e.repo.is_empty())
    }
}

/// Reads the registry; anything unreadable is an empty registry.
#[cfg(feature = "serde")]
pub fn load(path: &Path) -> Registry {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes the registry atomically (a temporary file, then a rename).
#[cfg(feature = "serde")]
pub fn save(path: &Path, reg: &Registry) -> Result<(), TauError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.part");
    let body =
        serde_json::to_vec_pretty(reg).map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?;
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    fn entry(id: &str, tag: &str) -> ManualEntry {
        ManualEntry {
            core_id: id.into(),
            kind: "github".into(),
            owner: "o".into(),
            repo: "r".into(),
            tag: tag.into(),
            asset: "a.zip".into(),
            sha256: "0".repeat(64),
            added: 1,
            card: "CARD".into(),
        }
    }

    #[test]
    fn upsert_replaces_by_id_case_insensitively_and_retire_forgets() {
        let mut r = Registry::default();
        r.upsert(entry("a.B", "v1"));
        r.upsert(entry("c.D", "v1"));
        r.upsert(entry("A.b", "v2"));
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.get("a.b").unwrap().tag, "v2");
        assert!(r.retire("c.d") && !r.retire("c.d"));
        assert_eq!(r.github().count(), 1);
        let mut zip = entry("z.Z", "");
        zip.kind = "zip".into();
        r.upsert(zip);
        assert_eq!(r.github().count(), 1);
    }

    #[test]
    fn the_file_round_trips_and_a_damaged_one_is_just_empty() {
        let dir = std::env::temp_dir().join(format!(
            "tau-reg-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        let path = dir.join("manual_cores.json");
        assert!(load(&path).entries.is_empty());
        let mut r = Registry::default();
        r.upsert(entry("a.B", "v1"));
        save(&path, &r).unwrap();
        assert_eq!(load(&path), r);
        fs::write(&path, b"{ not json").unwrap();
        assert!(load(&path).entries.is_empty());
        // an older file with fewer fields still reads
        fs::write(
            &path,
            br#"{"entries":[{"core_id":"a.B","kind":"zip","sha256":"x","added":5}]}"#,
        )
        .unwrap();
        assert_eq!(load(&path).get("a.B").unwrap().added, 5);
    }
}
