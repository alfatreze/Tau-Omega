//! Carrying media from the previous build to a new one (`docs/DEV_LOOP_SPEC.md`, section 1 rows 5 and 11). A new test build is a new core id, so the files
//! the old build kept in its own asset folder (`Assets/<platform>/<old id>`) are not where the new one looks (`Assets/<platform>/<new id>`). This module
//! finds what could be carried, plans the copy, and does it: every file is written beside its final name, read back and compared, and **nothing already on
//! the card is ever overwritten** (a different file at the destination is left and reported). A music library that lives in a platform's shared `common`
//! folder is a different job (it has an index with a root path in it): that is reported as a candidate for the existing, index-aware core copy, or as
//! "shared" when the two builds already read the same library.

use crate::{ErrorCode, TauError, core_files::walk};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum CandidateKind {
    /// Files in the old build's own folder: carried here.
    Folder,
    /// A music library in another platform's `common` folder: use the index-aware core copy.
    Library,
    /// Both builds read the same library already: nothing to carry.
    Shared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Candidate {
    pub kind: CandidateKind,
    pub from_id: String,
    pub from_name: String,
    pub platform: String,
    /// Relative to the card root.
    pub source_rel: String,
    pub dest_rel: String,
    pub files: usize,
    pub bytes: u64,
    pub note: String,
}

fn counts(dir: &Path) -> (usize, u64) {
    let w = walk(dir);
    (
        w.len(),
        w.iter()
            .filter_map(|(_, p)| fs::metadata(p).ok())
            .map(|m| m.len())
            .sum(),
    )
}

/// What could be carried from each of `older` (installed cores, as `(id, shortname)`) to `new_id`, on the card.
pub fn candidates(
    card: &Path,
    new_id: &str,
    older: &[(String, String)],
) -> Result<Vec<Candidate>, TauError> {
    let cores = crate::inspect_card(card)?.cores;
    let Some(new) = cores.iter().find(|c| c.id.eq_ignore_ascii_case(new_id)) else {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            format!("{new_id} is not installed on this card."),
        ));
    };
    let mut out = Vec::new();
    for (old_id, old_name) in older {
        if old_id.eq_ignore_ascii_case(new_id) {
            continue;
        }
        for p in &new.platforms {
            let src = card.join("Assets").join(p).join(old_id);
            let (files, bytes) = counts(&src);
            if files > 0 {
                out.push(Candidate {
                    kind: CandidateKind::Folder,
                    from_id: old_id.clone(),
                    from_name: old_name.clone(),
                    platform: p.clone(),
                    source_rel: format!("Assets/{p}/{old_id}"),
                    dest_rel: format!("Assets/{p}/{new_id}"),
                    files,
                    bytes,
                    note: format!("{old_name} kept these files in its own folder; {} reads Assets/{p}/{new_id}.", new.shortname),
                });
            }
        }
        if let Some(old) = cores
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(old_id))
            .filter(|o| o.library_capable && new.library_capable)
        {
            if old.media_platform == new.media_platform {
                out.push(Candidate {
                        kind: CandidateKind::Shared,
                        from_id: old_id.clone(),
                        from_name: old_name.clone(),
                        platform: new.media_platform.clone(),
                        source_rel: String::new(),
                        dest_rel: String::new(),
                        files: 0,
                        bytes: 0,
                        note: format!("{old_name} and {} read the same music library (Assets/{}/common): nothing to carry.", new.shortname, new.media_platform),
                    });
            } else {
                let rel = format!("Assets/{}/common", old.media_platform);
                let (files, bytes) = counts(&card.join(&rel));
                out.push(Candidate {
                        kind: CandidateKind::Library,
                        from_id: old_id.clone(),
                        from_name: old_name.clone(),
                        platform: old.media_platform.clone(),
                        source_rel: rel,
                        dest_rel: format!("Assets/{}/common", new.media_platform),
                        files,
                        bytes,
                        note: format!("{old_name} reads another library than {}. Copying it rebuilds the index with the right root path (the Compare cores copy does exactly this).", new.shortname),
                    });
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ItemState {
    New,
    Identical,
    /// A different file is already there: it stays.
    Different,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Item {
    pub rel: String,
    pub bytes: u64,
    pub state: ItemState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CarryPlan {
    pub id: String,
    pub source_rel: String,
    pub dest_rel: String,
    pub items: Vec<Item>,
    pub new_files: usize,
    pub identical: usize,
    pub different: usize,
    pub bytes_to_write: u64,
    pub blocked: Option<String>,
}

fn ok_id(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', '\\', '\0']) && !s.contains("..") && !s.starts_with('.')
}

fn file_hash(p: &Path) -> Result<String, TauError> {
    let mut f = fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Plans copying `Assets/<platform>/<from_id>` to `Assets/<platform>/<to_id>`. Reads only.
pub fn plan(
    card: &Path,
    from_id: &str,
    to_id: &str,
    platform: &str,
) -> Result<CarryPlan, TauError> {
    if ![from_id, to_id, platform].iter().all(|s| ok_id(s)) {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            "That is not a core or platform id.",
        ));
    }
    if from_id.eq_ignore_ascii_case(to_id) {
        return Err(TauError::e(
            ErrorCode::InstallRefused,
            "The two builds are the same core.",
        ));
    }
    let source_rel = format!("Assets/{platform}/{from_id}");
    let dest_rel = format!("Assets/{platform}/{to_id}");
    let (src, dst) = (card.join(&source_rel), card.join(&dest_rel));
    if !src.is_dir() {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            format!("{source_rel} does not exist on this card."),
        ));
    }
    if !card.join("Cores").join(to_id).join("core.json").is_file() {
        return Err(TauError::e(
            ErrorCode::InvalidPathReference,
            format!("{to_id} is not installed on this card."),
        ));
    }
    let mut items = Vec::new();
    for (rel, p) in walk(&src) {
        let bytes = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        let target = dst.join(&rel);
        let state = if !target.is_file() {
            ItemState::New
        } else if fs::metadata(&target).map(|m| m.len()).unwrap_or(0) == bytes
            && file_hash(&target)? == file_hash(&p)?
        {
            ItemState::Identical
        } else {
            ItemState::Different
        };
        items.push(Item { rel, bytes, state });
    }
    let count = |s: ItemState| items.iter().filter(|i| i.state == s).count();
    let (new_files, identical, different) = (
        count(ItemState::New),
        count(ItemState::Identical),
        count(ItemState::Different),
    );
    let bytes_to_write = items
        .iter()
        .filter(|i| i.state == ItemState::New)
        .map(|i| i.bytes)
        .sum();
    let mut h = Sha256::new();
    h.update(format!("{from_id}\0{to_id}\0{platform}").as_bytes());
    for i in &items {
        h.update(format!("\0{}\0{}\0{:?}", i.rel, i.bytes, i.state).as_bytes());
    }
    let blocked = items
        .is_empty()
        .then(|| "There is nothing in that folder to carry.".to_string());
    Ok(CarryPlan {
        id: format!("{:x}", h.finalize())[..32].to_string(),
        source_rel,
        dest_rel,
        items,
        new_files,
        identical,
        different,
        bytes_to_write,
        blocked,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CarryReport {
    pub copied: usize,
    pub identical: usize,
    pub left_different: usize,
    pub bytes_written: u64,
}

/// Copies the planned files. The plan is rebuilt and must carry the same id (the card changed since it was shown otherwise). Each file is written to
/// a `.part` name, read back and compared with the source, then renamed; on a mismatch the part is removed and the run stops with what it had done.
pub fn execute(
    card: &Path,
    from_id: &str,
    to_id: &str,
    platform: &str,
    confirmation: &str,
) -> Result<CarryReport, TauError> {
    let p = plan(card, from_id, to_id, platform)?;
    if confirmation != p.id {
        return Err(TauError::e(
            ErrorCode::ConfirmationMismatch,
            "The card changed since this was shown. Review it again.",
        ));
    }
    if let Some(b) = &p.blocked {
        return Err(TauError::e(ErrorCode::InstallRefused, b.clone()));
    }
    let (src, dst) = (card.join(&p.source_rel), card.join(&p.dest_rel));
    let mut report = CarryReport {
        copied: 0,
        identical: p.identical,
        left_different: p.different,
        bytes_written: 0,
    };
    for item in p.items.iter().filter(|i| i.state == ItemState::New) {
        let (from, to) = (src.join(&item.rel), dst.join(&item.rel));
        if let Some(dir) = to.parent() {
            fs::create_dir_all(dir)?;
        }
        let part = to.with_file_name(format!(
            "{}.part",
            to.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        ));
        fs::copy(&from, &part)?;
        if file_hash(&part)? != file_hash(&from)? {
            let _ = fs::remove_file(&part);
            return Err(TauError::e(
                ErrorCode::Io,
                format!(
                    "{} did not copy correctly ({} files were done before it); nothing after it was written.",
                    item.rel, report.copied
                ),
            ));
        }
        fs::rename(&part, &to)?;
        report.copied += 1;
        report.bytes_written += item.bytes;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::{card, core_json, write};

    fn two_builds(name: &str) -> std::path::PathBuf {
        let c = card(name);
        write(
            &c.join("Cores/x.B1/core.json"),
            core_json("x", "B1", "1", &["xp"]).as_bytes(),
        );
        write(
            &c.join("Cores/x.B2/core.json"),
            core_json("x", "B2", "1", &["xp"]).as_bytes(),
        );
        write(&c.join("Assets/xp/x.B1/a.bin"), b"alpha");
        write(&c.join("Assets/xp/x.B1/sub/b.bin"), b"bravo");
        write(&c.join("Assets/xp/x.B1/._junk"), b"x");
        c
    }

    #[test]
    fn a_folder_candidate_names_the_files_and_where_they_go() {
        let c = two_builds("carry1");
        let v = candidates(
            &c,
            "x.B2",
            &[("x.B1".into(), "B1".into()), ("x.B2".into(), "B2".into())],
        )
        .unwrap();
        let f: Vec<&Candidate> = v
            .iter()
            .filter(|x| x.kind == CandidateKind::Folder)
            .collect();
        assert_eq!(f.len(), 1, "the build itself is never a source");
        assert_eq!((f[0].files, f[0].bytes), (2, 10));
        assert_eq!(
            (f[0].source_rel.as_str(), f[0].dest_rel.as_str()),
            ("Assets/xp/x.B1", "Assets/xp/x.B2")
        );
        assert!(candidates(&c, "x.Nope", &[]).is_err());
    }

    #[test]
    fn carrying_copies_new_files_verified_and_never_overwrites_a_different_one() {
        let c = two_builds("carry2");
        write(&c.join("Assets/xp/x.B2/a.bin"), b"ALPHA-DIFFERENT");
        let p = plan(&c, "x.B1", "x.B2", "xp").unwrap();
        assert_eq!(
            (p.new_files, p.identical, p.different, p.bytes_to_write),
            (1, 0, 1, 5)
        );
        assert!(execute(&c, "x.B1", "x.B2", "xp", "wrong").is_err());
        let r = execute(&c, "x.B1", "x.B2", "xp", &p.id).unwrap();
        assert_eq!((r.copied, r.left_different, r.bytes_written), (1, 1, 5));
        assert_eq!(
            fs::read(c.join("Assets/xp/x.B2/sub/b.bin")).unwrap(),
            b"bravo"
        );
        assert_eq!(
            fs::read(c.join("Assets/xp/x.B2/a.bin")).unwrap(),
            b"ALPHA-DIFFERENT",
            "a different file is left alone"
        );
        assert_eq!(
            fs::read(c.join("Assets/xp/x.B1/a.bin")).unwrap(),
            b"alpha",
            "the source is untouched"
        );
        assert!(!c.join("Assets/xp/x.B2/sub/b.bin.part").exists());
        // a second run finds everything done
        let again = plan(&c, "x.B1", "x.B2", "xp").unwrap();
        assert_eq!(
            (again.new_files, again.identical, again.different),
            (0, 1, 1)
        );
        assert_eq!(
            execute(&c, "x.B1", "x.B2", "xp", &again.id).unwrap().copied,
            0
        );
    }

    #[test]
    fn a_changed_card_stops_the_run_and_bad_ids_are_refused() {
        let c = two_builds("carry3");
        let p = plan(&c, "x.B1", "x.B2", "xp").unwrap();
        write(&c.join("Assets/xp/x.B1/c.bin"), b"new since the plan");
        assert!(execute(&c, "x.B1", "x.B2", "xp", &p.id).is_err());
        assert!(
            !c.join("Assets/xp/x.B2/a.bin").exists(),
            "nothing was written"
        );
        assert!(plan(&c, "../x", "x.B2", "xp").is_err());
        assert!(plan(&c, "x.B1", "x.B1", "xp").is_err());
        assert!(plan(&c, "x.B1", "x.Missing", "xp").is_err());
    }
}
