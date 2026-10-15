//! The network side of a manual add from a GitHub address (`docs/MARKETPLACE_SPEC.md` section 7): the releases of one repository, the zip of one
//! release, and (optionally) the README and the pictures it shows. Every request goes through [`Transport`] (so it is tested with scripted
//! answers) and to the GitHub hosts only; nothing about the user, the card or the machine is sent. The judgement (which zip, is it safe) is in
//! `tau_core::github_release` and `core_package`.

use crate::marketplace::{Checksum, CoreDownloaded, Transport, limit_message};
use std::{fs, path::Path};
use tau_core::{
    ErrorCode, TauError,
    catalog_release::{DownloadAddress, parse_download_url},
    github_release::{self, GhAsset, GhRelease},
};

const LIST_LIMIT: u64 = 4 * 1024 * 1024;
const README_LIMIT: u64 = 400 * 1024;
const IMAGE_LIMIT: u64 = 600 * 1024;
/// How much of a README is kept for the preview.
pub const README_CHARS: usize = 6000;

fn err(code: ErrorCode, message: impl Into<String>) -> TauError {
    TauError {
        code,
        message: message.into(),
    }
}

fn judge(status: u16, rate_reset: Option<u64>, now: u64, what: &str) -> Result<(), TauError> {
    match status {
        200 => Ok(()),
        404 => Err(err(
            ErrorCode::NotFound,
            format!("{what} was not found on GitHub. Check the address."),
        )),
        403 | 429 => Err(err(ErrorCode::Io, limit_message(rate_reset, now))),
        s => Err(err(
            ErrorCode::Io,
            format!("GitHub answered with an error ({s})."),
        )),
    }
}

/// The releases of `owner/repo`, newest first (drafts dropped).
pub fn list_releases(
    t: &dyn Transport,
    owner: &str,
    repo: &str,
    now: u64,
) -> Result<Vec<GhRelease>, TauError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=20");
    let r = t.get(&url, None, LIST_LIMIT)?;
    judge(r.status, r.rate_reset, now, "That repository")?;
    github_release::parse_releases(&r.body)
}

/// One release by its tag.
pub fn release_by_tag(
    t: &dyn Transport,
    owner: &str,
    repo: &str,
    tag: &str,
    now: u64,
) -> Result<GhRelease, TauError> {
    if !tag
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "That release tag has characters Omega does not look up.",
        ));
    }
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}");
    let r = t.get(&url, None, LIST_LIMIT)?;
    judge(r.status, r.rate_reset, now, "That release")?;
    github_release::parse_release(&r.body)
}

fn safe_part(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Downloads `url` (a release asset or a pinned file, already checked by `parse_download_url`) into `dest_dir`. `size` and `sha256` are what
/// GitHub published for it, when it did; a mismatch deletes the file and fails. Written as `.part` first.
pub fn download_zip(
    t: &dyn Transport,
    addr: &DownloadAddress,
    url: &str,
    size: u64,
    sha256: Option<&str>,
    dest_dir: &Path,
    max_bytes: u64,
) -> Result<CoreDownloaded, TauError> {
    if parse_download_url(url).as_ref() != Some(addr) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "That download address is not one Omega accepts.",
        ));
    }
    if size > max_bytes {
        return Err(err(
            ErrorCode::InstallRefused,
            format!(
                "The download is {} MB, over the {} MB limit.",
                size >> 20,
                max_bytes >> 20
            ),
        ));
    }
    fs::create_dir_all(dest_dir)?;
    let name = format!(
        "{}.{}.{}",
        safe_part(&addr.repo),
        safe_part(&addr.reference),
        safe_part(&addr.file)
    );
    let path = dest_dir.join(&name[..name.len().min(120)]);
    let part = path.with_extension("zip.part");
    let (status, bytes, sha) = t.download(url, max_bytes, &part)?;
    let fail = |m: String| {
        let _ = fs::remove_file(&part);
        Err(err(ErrorCode::Io, m))
    };
    if status != 200 {
        return fail(match status {
            404 => "That file is no longer on GitHub.".into(),
            403 | 429 => limit_message(None, 0),
            s => format!("GitHub answered with an error ({s})."),
        });
    }
    if size != 0 && bytes != size {
        return fail(format!(
            "The download is {bytes} bytes but the release lists {size}: it was cut short or changed."
        ));
    }
    let checksum = match sha256 {
        Some(want) if want != sha => {
            return fail(
                "The download does not match the checksum GitHub publishes for it.".into(),
            );
        }
        Some(_) => Checksum::Verified,
        None => Checksum::NotPublished,
    };
    fs::rename(&part, &path)?;
    Ok(CoreDownloaded {
        path: path.to_string_lossy().into_owned(),
        bytes,
        sha256: sha,
        checksum,
    })
}

/// A release asset of a listed release: the address is rebuilt from what GitHub listed.
pub fn download_asset(
    t: &dyn Transport,
    asset: &GhAsset,
    dest_dir: &Path,
    max_bytes: u64,
) -> Result<CoreDownloaded, TauError> {
    let addr = github_release::address_of(asset).ok_or_else(|| {
        err(
            ErrorCode::InvalidPathReference,
            "That download address is not one Omega accepts.",
        )
    })?;
    download_zip(
        t,
        &addr,
        &asset.url,
        asset.size,
        asset.sha256.as_deref(),
        dest_dir,
        max_bytes,
    )
}

/// The README of the repository's default branch as plain text (first [`README_CHARS`] characters), or `None` when there is none.
pub fn readme(t: &dyn Transport, owner: &str, repo: &str) -> Option<String> {
    for name in ["README.md", "readme.md", "README.markdown", "README"] {
        let url = format!("https://raw.githubusercontent.com/{owner}/{repo}/HEAD/{name}");
        if let Ok(r) = t.get(&url, None, README_LIMIT)
            && r.status == 200
        {
            let text = String::from_utf8_lossy(&r.body);
            return Some(text.chars().take(README_CHARS).collect());
        }
    }
    None
}

fn image_kind(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// One README picture: its bytes and media type, only when it really is a PNG, JPEG, GIF or WebP (the file's own bytes decide, not the name).
pub fn fetch_image(t: &dyn Transport, url: &str) -> Option<(Vec<u8>, &'static str)> {
    let r = t.get(url, None, IMAGE_LIMIT).ok()?;
    if r.status != 200 {
        return None;
    }
    image_kind(&r.body).map(|k| (r.body, k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::Response;
    use sha2::{Digest, Sha256};
    use std::{cell::RefCell, path::PathBuf};

    #[derive(Default)]
    struct Fake {
        script: RefCell<Vec<Result<Response, TauError>>>,
        asked: RefCell<Vec<String>>,
        file: RefCell<Vec<u8>>,
    }
    impl Transport for Fake {
        fn get(&self, url: &str, _inm: Option<&str>, _limit: u64) -> Result<Response, TauError> {
            self.asked.borrow_mut().push(url.to_string());
            self.script.borrow_mut().remove(0)
        }
        fn download(
            &self,
            url: &str,
            _limit: u64,
            dest: &Path,
        ) -> Result<(u16, u64, String), TauError> {
            self.asked.borrow_mut().push(url.to_string());
            let body = self.file.borrow().clone();
            fs::write(dest, &body)?;
            Ok((
                200,
                body.len() as u64,
                format!("{:x}", Sha256::digest(&body)),
            ))
        }
    }
    fn ok(status: u16, body: &[u8]) -> Result<Response, TauError> {
        Ok(Response {
            status,
            body: body.to_vec(),
            ..Response::default()
        })
    }
    fn scratch() -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("tau-gh-{}-{}", std::process::id(), tau_core_uniq()));
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn tau_core_uniq() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    }
    const LIST: &str = r#"[{"tag_name":"v1","published_at":"2024-01-01T00:00:00Z","assets":[{"name":"a.B.zip","size":4,"browser_download_url":"https://github.com/o/r/releases/download/v1/a.B.zip"}]}]"#;

    #[test]
    fn releases_are_listed_and_failures_are_explained_in_words() {
        let f = Fake::default();
        f.script.borrow_mut().push(ok(200, LIST.as_bytes()));
        let r = list_releases(&f, "o", "r", 0).unwrap();
        assert_eq!(r[0].tag, "v1");
        assert_eq!(
            f.asked.borrow()[0],
            "https://api.github.com/repos/o/r/releases?per_page=20"
        );
        f.script.borrow_mut().push(ok(404, b""));
        assert!(
            list_releases(&f, "o", "r", 0)
                .unwrap_err()
                .message
                .contains("not found")
        );
        f.script.borrow_mut().push(Ok(Response {
            status: 403,
            rate_reset: Some(1000 + 600),
            ..Response::default()
        }));
        assert!(
            list_releases(&f, "o", "r", 1000)
                .unwrap_err()
                .message
                .contains("10 minutes")
        );
        f.script.borrow_mut().push(Err(TauError {
            code: ErrorCode::Io,
            message: "Could not reach the server. Are you offline?".into(),
        }));
        assert!(
            list_releases(&f, "o", "r", 0)
                .unwrap_err()
                .message
                .contains("offline")
        );
        // a tag with odd characters is never put in a request
        assert!(release_by_tag(&f, "o", "r", "v1/../x", 0).is_err());
    }

    #[test]
    fn a_download_is_checked_against_the_size_and_the_published_checksum() {
        let (f, dir) = (Fake::default(), scratch());
        *f.file.borrow_mut() = b"zip!".to_vec();
        let r = list_releases(
            &{
                let g = Fake::default();
                g.script.borrow_mut().push(ok(200, LIST.as_bytes()));
                g
            },
            "o",
            "r",
            0,
        )
        .unwrap();
        let asset = &r[0].assets[0];
        let got = download_asset(&f, asset, &dir, 1 << 20).unwrap();
        assert_eq!(
            (got.bytes, got.checksum.clone()),
            (4, Checksum::NotPublished)
        );
        assert!(Path::new(&got.path).is_file());
        // a published digest must match the bytes
        let good = format!("{:x}", Sha256::digest(b"zip!"));
        let with = GhAsset {
            sha256: Some(good),
            ..asset.clone()
        };
        assert_eq!(
            download_asset(&f, &with, &dir, 1 << 20).unwrap().checksum,
            Checksum::Verified
        );
        let bad = GhAsset {
            sha256: Some("0".repeat(64)),
            ..asset.clone()
        };
        assert!(
            download_asset(&f, &bad, &dir, 1 << 20)
                .unwrap_err()
                .message
                .contains("checksum")
        );
        assert!(!dir.join("r.v1.a.B.zip.part").exists());
        // the size GitHub lists must match what arrived; over the limit is refused before any request
        let wrong = GhAsset {
            size: 9,
            ..asset.clone()
        };
        assert!(
            download_asset(&f, &wrong, &dir, 1 << 20)
                .unwrap_err()
                .message
                .contains("cut short")
        );
        let big = GhAsset {
            size: 5 << 20,
            ..asset.clone()
        };
        let asked = f.asked.borrow().len();
        assert!(
            download_asset(&f, &big, &dir, 1 << 20).is_err() && f.asked.borrow().len() == asked
        );
        // an address that is not the listed shape is refused
        let off = GhAsset {
            url: "https://evil.example/a.zip".into(),
            ..asset.clone()
        };
        assert!(download_asset(&f, &off, &dir, 1 << 20).is_err());
    }

    #[test]
    fn readme_and_pictures_are_optional_and_checked_by_their_bytes() {
        let f = Fake::default();
        f.script.borrow_mut().push(ok(404, b"")); // README.md
        f.script
            .borrow_mut()
            .push(ok(200, "# Core\nhello".as_bytes())); // readme.md
        assert_eq!(readme(&f, "o", "r").as_deref(), Some("# Core\nhello"));
        let none = Fake::default();
        for _ in 0..4 {
            none.script.borrow_mut().push(ok(404, b""));
        }
        assert_eq!(readme(&none, "o", "r"), None);
        let img = Fake::default();
        img.script
            .borrow_mut()
            .push(ok(200, b"\x89PNG\r\n\x1a\nrest"));
        img.script
            .borrow_mut()
            .push(ok(200, b"<svg onload=alert(1)></svg>"));
        img.script.borrow_mut().push(ok(404, b""));
        assert_eq!(
            fetch_image(&img, "https://raw.githubusercontent.com/o/r/HEAD/a.png")
                .unwrap()
                .1,
            "image/png"
        );
        assert!(fetch_image(&img, "https://raw.githubusercontent.com/o/r/HEAD/b.png").is_none());
        assert!(fetch_image(&img, "https://raw.githubusercontent.com/o/r/HEAD/c.png").is_none());
    }
}
