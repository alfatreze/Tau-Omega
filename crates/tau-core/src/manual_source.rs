//! Where a manually added core comes from: a zip file, a folder, or a GitHub address typed in. This module only **classifies and validates**
//! what the user gave (no network, no reading of the contents); `core_pack` turns a folder into a package, `core_package::vet_zip` vets it and
//! `core_preview` describes it. See `docs/MARKETPLACE_SPEC.md` section 7.
//!
//! GitHub addresses are accepted only in these shapes, by exact host and path (no lookalike hosts, no `user@`, no port, no `.git`, no query):
//! `https://github.com/<owner>/<repo>` (also `/releases` and `/releases/latest`: the newest release), `.../releases/tag/<tag>`,
//! `.../releases/download/<tag>/<file>.zip`, and a commit-pinned `https://raw.githubusercontent.com/<owner>/<repo>/<40-hex>/<path>.zip`.

use crate::{
    ErrorCode, TauError,
    catalog_release::{DownloadAddress, parse_download_url},
};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GitHubSource {
    /// A repository: its newest release.
    Repo { owner: String, repo: String },
    /// One release of a repository.
    Tag {
        owner: String,
        repo: String,
        tag: String,
    },
    /// One zip: a release asset or a commit-pinned file.
    File(DownloadAddress),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Source {
    Zip(PathBuf),
    Folder(PathBuf),
    GitHub(GitHubSource),
}

fn refuse(msg: impl Into<String>) -> TauError {
    TauError::e(ErrorCode::InstallRefused, msg)
}

fn owner_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 39
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn repo_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn tag_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && !s.split('/').any(|p| p.is_empty() || p == "." || p == "..")
        && !s.contains(['\\', '\0', ' ', '?', '#', '%', '@', ':'])
}

/// Validates a typed GitHub address. The error is one plain sentence saying what to change.
pub fn parse_github(input: &str) -> Result<GitHubSource, TauError> {
    let url = input.trim();
    if url.starts_with("http://") {
        return Err(refuse("Use the https:// address."));
    }
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| refuse("That is not a web address (it should start with https://)."))?;
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if host.contains(['@', ':']) {
        return Err(refuse(
            "The address has a user name or a port in it; paste the plain address from the repository page.",
        ));
    }
    if url.contains(['?', '#']) {
        return Err(refuse(
            "Remove everything after the repository or release name (no ? or # parts).",
        ));
    }
    match host {
        "raw.githubusercontent.com" => parse_download_url(url)
            .filter(|a| !a.is_release)
            .map(GitHubSource::File)
            .ok_or_else(|| refuse("That raw GitHub address is not a zip pinned to a commit.")),
        "github.com" => {
            let path = path.trim_end_matches('/');
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
                return Err(refuse(
                    "Paste the repository address: https://github.com/<owner>/<repository>.",
                ));
            }
            let (owner, repo) = (parts[0], parts[1]);
            if repo.to_ascii_lowercase().ends_with(".git") {
                return Err(refuse(
                    "Paste the repository page address, without .git at the end.",
                ));
            }
            if !owner_ok(owner) || !repo_ok(repo) {
                return Err(refuse(
                    "The owner or repository name has a character GitHub does not allow.",
                ));
            }
            match &parts[2..] {
                [] | ["releases"] | ["releases", "latest"] => Ok(GitHubSource::Repo {
                    owner: owner.into(),
                    repo: repo.into(),
                }),
                ["releases", "tag", tag @ ..] => {
                    let tag = tag.join("/");
                    if tag_ok(&tag) {
                        Ok(GitHubSource::Tag {
                            owner: owner.into(),
                            repo: repo.into(),
                            tag,
                        })
                    } else {
                        Err(refuse("That release tag is not valid."))
                    }
                }
                ["releases", "download", ..] => parse_download_url(url)
                    .filter(|a| a.is_release)
                    .map(GitHubSource::File)
                    .ok_or_else(|| refuse("That release download is not a .zip file address.")),
                _ => Err(refuse(
                    "That is a page inside the repository. Paste the repository address or a release address.",
                )),
            }
        }
        _ => Err(refuse(
            "Only github.com addresses are accepted. Download the zip yourself and add it as a file.",
        )),
    }
}

/// Classifies what the user typed or dropped: a GitHub address, a folder or a zip file on this computer.
pub fn classify(input: &str) -> Result<Source, TauError> {
    let t = input.trim();
    if t.starts_with("http://") || t.starts_with("https://") {
        return parse_github(t).map(Source::GitHub);
    }
    let path = PathBuf::from(t);
    if path.is_dir() {
        Ok(Source::Folder(path))
    } else if path.is_file() {
        if t.to_ascii_lowercase().ends_with(".zip") {
            Ok(Source::Zip(path))
        } else {
            Err(refuse(
                "That file is not a .zip. A core is added from a .zip, a folder, or a GitHub address.",
            ))
        }
    } else {
        Err(refuse(
            "That is not a file or folder on this computer, and not a web address.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_accepted_github_shapes() {
        let repo = |o: &str, r: &str| GitHubSource::Repo {
            owner: o.into(),
            repo: r.into(),
        };
        for u in [
            "https://github.com/agg23/openfpga-arduboy",
            "https://github.com/agg23/openfpga-arduboy/",
            "https://github.com/agg23/openfpga-arduboy/releases",
            "https://github.com/agg23/openfpga-arduboy/releases/latest",
            "  https://github.com/agg23/openfpga-arduboy  ",
        ] {
            assert_eq!(
                parse_github(u).unwrap(),
                repo("agg23", "openfpga-arduboy"),
                "{u}"
            );
        }
        assert_eq!(
            parse_github("https://github.com/o/r/releases/tag/v1.2/rc1").unwrap(),
            GitHubSource::Tag {
                owner: "o".into(),
                repo: "r".into(),
                tag: "v1.2/rc1".into()
            }
        );
        match parse_github("https://github.com/agg23/openfpga-arduboy/releases/download/0.9.0/agg23.Arduboy.0.9.0.zip").unwrap() {
            GitHubSource::File(a) => assert!(a.is_release && a.file == "agg23.Arduboy.0.9.0.zip" && a.reference == "0.9.0"),
            other => panic!("{other:?}"),
        }
        let pinned = format!(
            "https://raw.githubusercontent.com/o/r/{}/dist/x.zip",
            "a".repeat(40)
        );
        match parse_github(&pinned).unwrap() {
            GitHubSource::File(a) => assert!(!a.is_release),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lookalikes_and_odd_forms_are_refused_with_a_reason() {
        for u in [
            "http://github.com/o/r",
            "https://github.com.evil.io/o/r",
            "https://evilgithub.com/o/r",
            "https://user@github.com/o/r",
            "https://github.com:8443/o/r",
            "https://github.com/o/r.git",
            "https://github.com/o",
            "https://github.com/o/r?tab=readme",
            "https://github.com/o/r#readme",
            "https://github.com/o/r/blob/main/README.md",
            "https://github.com/o/r/releases/tag/",
            "https://github.com/o/r/releases/tag/../x",
            "https://github.com/o/r/releases/download/v1/core.tar.gz",
            "https://github.com/-o/r",
            "https://github.com/o/r r",
            "https://raw.githubusercontent.com/o/r/main/x.zip",
            "git@github.com:o/r.git",
            "ftp://github.com/o/r",
        ] {
            assert!(parse_github(u).is_err(), "{u}");
        }
    }

    #[test]
    fn classify_tells_zip_folder_and_address_apart() {
        let dir = std::env::temp_dir().join(format!(
            "tau-msrc-{}-{}",
            std::process::id(),
            crate::test_uniq()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("core.zip");
        std::fs::write(&zip, b"x").unwrap();
        let txt = dir.join("core.txt");
        std::fs::write(&txt, b"x").unwrap();
        assert!(matches!(
            classify(dir.to_str().unwrap()).unwrap(),
            Source::Folder(_)
        ));
        assert!(matches!(
            classify(zip.to_str().unwrap()).unwrap(),
            Source::Zip(_)
        ));
        assert!(matches!(
            classify("https://github.com/o/r").unwrap(),
            Source::GitHub(_)
        ));
        assert!(classify(txt.to_str().unwrap()).is_err());
        assert!(classify("/no/such/place").is_err());
        assert!(classify("https://example.org/x.zip").is_err());
    }
}
