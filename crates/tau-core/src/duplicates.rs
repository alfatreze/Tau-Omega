//! Read-only duplicate detection for media copies.

use crate::{Entry, TauError};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

pub fn find_duplicates(
    root: impl AsRef<Path>,
    entries: &[Entry],
) -> Result<Vec<Vec<String>>, TauError> {
    let root = root.as_ref();
    let mut by_hash: BTreeMap<[u8; 32], Vec<String>> = BTreeMap::new();
    for entry in entries {
        let mut file = fs::File::open(root.join(&entry.rel))?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(TauError::from)?;
        let hash: [u8; 32] = hasher.finalize().into();
        by_hash.entry(hash).or_default().push(entry.rel.clone());
    }
    Ok(by_hash
        .into_values()
        .filter(|group| group.len() > 1)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn groups_only_identical_files() {
        let root = std::env::temp_dir().join(format!(
            "tau-duplicates-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("one.mp3"), b"same").unwrap();
        fs::write(root.join("two.mp3"), b"same").unwrap();
        fs::write(root.join("three.mp3"), b"different").unwrap();
        let entry = |rel: &str| Entry {
            rel: rel.into(),
            dir: String::new(),
            file: rel.into(),
            tags: BTreeMap::new(),
            secs: 0,
            fmt: 1,
        };
        assert_eq!(
            find_duplicates(
                &root,
                &[entry("one.mp3"), entry("two.mp3"), entry("three.mp3")]
            )
            .unwrap(),
            vec![vec![String::from("one.mp3"), String::from("two.mp3")]]
        );
        fs::remove_dir_all(root).unwrap();
    }
}
