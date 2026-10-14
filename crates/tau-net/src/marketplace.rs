//! Network side of the core marketplace: the openFPGA Library catalog, the GitHub release of one catalog core, and
//! the verified download of its zip. What leaves the machine: one GET of the public catalog, one GET of the release
//! of a core the user opened (or of an installed core when updates are checked), and the zip only when the user
//! asks. No identifier, card name or path is sent.
//!
//! Everything goes through the [`Transport`] trait so caching, ETags, rate limits and verification are tested
//! without a network. The judgement (which asset, is the zip safe) lives in `tau_core`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use tau_core::{
    ErrorCode, TauError,
    catalog::{self, CATALOG_URL, Catalog, CatalogCore},
    catalog_release::{self, CoreRelease},
};

const CATALOG_LIMIT: u64 = 4 * 1024 * 1024;
const RELEASE_LIMIT: u64 = 2 * 1024 * 1024;
/// The catalog changes a few times a day; do not ask more often than this unless the user presses Refresh.
pub const CATALOG_TTL: u64 = 6 * 3600;
/// A release is looked up at most once an hour per core (GitHub allows 60 unauthenticated requests an hour).
pub const RELEASE_TTL: u64 = 3600;

fn err(code: ErrorCode, message: impl Into<String>) -> TauError {
    TauError {
        code,
        message: message.into(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct Response {
    pub status: u16,
    pub etag: Option<String>,
    /// `x-ratelimit-remaining` / `x-ratelimit-reset` (unix seconds), when GitHub sent them.
    pub rate_remaining: Option<u64>,
    pub rate_reset: Option<u64>,
    pub body: Vec<u8>,
}

pub trait Transport {
    /// One GET (conditional when `if_none_match` is given), body capped at `limit`. Never fails on an HTTP status:
    /// the status is returned for the caller to judge; only transport failures are errors.
    fn get(&self, url: &str, if_none_match: Option<&str>, limit: u64)
    -> Result<Response, TauError>;
    /// GET streamed into `dest` (which the caller owns), capped at `limit`; returns the status, the byte count and
    /// the SHA-256 (lowercase hex) of what was written.
    fn download(&self, url: &str, limit: u64, dest: &Path) -> Result<(u16, u64, String), TauError>;
}

/// The real transport. Only the three hosts the marketplace needs are ever contacted.
pub struct Http;

fn host_allowed(url: &str) -> bool {
    url.starts_with("https://openfpga-library.github.io/analogue-pocket/")
        || url.starts_with("https://api.github.com/repos/")
        || url.starts_with("https://github.com/")
}

fn explain(error: &ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => {
            "The server did not answer in time. Check the connection and try again.".into()
        }
        ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            "Could not reach the server. Are you offline?".into()
        }
        other => format!("The request failed: {other}"),
    }
}

fn agent(secs: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(8)))
        .timeout_global(Some(Duration::from_secs(secs)))
        .http_status_as_error(false)
        .user_agent(concat!("Tau-Omega/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn header_u64(r: &ureq::http::Response<ureq::Body>, name: &str) -> Option<u64> {
    r.headers().get(name)?.to_str().ok()?.parse().ok()
}

impl Transport for Http {
    fn get(
        &self,
        url: &str,
        if_none_match: Option<&str>,
        limit: u64,
    ) -> Result<Response, TauError> {
        if !host_allowed(url) {
            return Err(err(
                ErrorCode::InvalidPathReference,
                "Omega only contacts the openFPGA Library and GitHub.",
            ));
        }
        let mut req = agent(30)
            .get(url)
            .header("Accept", "application/vnd.github+json, application/json");
        if let Some(tag) = if_none_match {
            req = req.header("If-None-Match", tag);
        }
        let mut r = req.call().map_err(|e| err(ErrorCode::Io, explain(&e)))?;
        let (status, etag) = (
            r.status().as_u16(),
            r.headers()
                .get("etag")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
        );
        let (rate_remaining, rate_reset) = (
            header_u64(&r, "x-ratelimit-remaining"),
            header_u64(&r, "x-ratelimit-reset"),
        );
        let body = if status == 304 {
            Vec::new()
        } else {
            r.body_mut()
                .with_config()
                .limit(limit)
                .read_to_vec()
                .map_err(|e| err(ErrorCode::Io, explain(&e)))?
        };
        Ok(Response {
            status,
            etag,
            rate_remaining,
            rate_reset,
            body,
        })
    }

    fn download(&self, url: &str, limit: u64, dest: &Path) -> Result<(u16, u64, String), TauError> {
        if !host_allowed(url) {
            return Err(err(
                ErrorCode::InvalidPathReference,
                "Omega only contacts the openFPGA Library and GitHub.",
            ));
        }
        let mut r = agent(600)
            .get(url)
            .call()
            .map_err(|e| err(ErrorCode::Io, explain(&e)))?;
        let status = r.status().as_u16();
        if status != 200 {
            return Ok((status, 0, String::new()));
        }
        let mut reader = r.body_mut().with_config().limit(limit).reader();
        let mut file = fs::File::create(dest)?;
        let (mut hash, mut total, mut buf) = (Sha256::new(), 0u64, vec![0u8; 64 * 1024]);
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| err(ErrorCode::Io, format!("The download was interrupted: {e}")))?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            file.write_all(&buf[..n])?;
            total += n as u64;
        }
        file.sync_all()?;
        Ok((status, total, format!("{:x}", hash.finalize())))
    }
}

// ---- cache -----------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Meta {
    etag: Option<String>,
    fetched: u64,
}

fn root(cache: &Path) -> PathBuf {
    cache.join("marketplace")
}

fn read_cached(path: &Path) -> Option<(Vec<u8>, Meta)> {
    let body = fs::read(path).ok()?;
    let meta = serde_json::from_slice(&fs::read(path.with_extension("meta")).ok()?).ok()?;
    Some((body, meta))
}

fn write_cached(path: &Path, body: &[u8], meta: &Meta) -> Result<(), TauError> {
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    fs::write(path, body)?;
    fs::write(
        path.with_extension("meta"),
        serde_json::to_vec(meta).unwrap_or_default(),
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// Fresh from the network.
    Network,
    /// The server said nothing changed.
    NotModified,
    /// Inside the time-to-live: no request made.
    Cache,
    /// The request failed; this is the cached copy, with the reason.
    Offline(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogResult {
    pub catalog: Catalog,
    pub source: Source,
    /// Unix seconds of the last successful fetch.
    pub fetched: u64,
}

/// The catalog: cached for [`CATALOG_TTL`], revalidated with its ETag, and the cached copy when offline.
pub fn fetch_catalog(
    t: &dyn Transport,
    cache: &Path,
    now: u64,
    force: bool,
) -> Result<CatalogResult, TauError> {
    let file = root(cache).join("cores.json");
    let cached = read_cached(&file).and_then(|(b, m)| catalog::parse(&b).ok().map(|c| (c, m)));
    if let Some((c, m)) = &cached
        && !force
        && now.saturating_sub(m.fetched) < CATALOG_TTL
    {
        return Ok(CatalogResult {
            catalog: c.clone(),
            source: Source::Cache,
            fetched: m.fetched,
        });
    }
    let etag = cached.as_ref().and_then(|(_, m)| m.etag.clone());
    let fall_back = |why: String, cached: Option<(Catalog, Meta)>| match cached {
        Some((c, m)) => Ok(CatalogResult {
            catalog: c,
            source: Source::Offline(why),
            fetched: m.fetched,
        }),
        None => Err(err(ErrorCode::Io, why)),
    };
    let r = match t.get(CATALOG_URL, etag.as_deref(), CATALOG_LIMIT) {
        Ok(r) => r,
        Err(e) => return fall_back(e.message, cached),
    };
    match r.status {
        304 => match cached {
            Some((c, m)) => {
                let meta = Meta {
                    etag: m.etag,
                    fetched: now,
                };
                let _ = write_cached(&file, &fs::read(&file).unwrap_or_default(), &meta);
                Ok(CatalogResult {
                    catalog: c,
                    source: Source::NotModified,
                    fetched: now,
                })
            }
            None => Err(err(
                ErrorCode::Io,
                "The server said the catalog is unchanged, but there is no saved copy.",
            )),
        },
        200 => match catalog::parse(&r.body) {
            // Only a list that parses replaces the saved one.
            Ok(c) => {
                write_cached(
                    &file,
                    &r.body,
                    &Meta {
                        etag: r.etag,
                        fetched: now,
                    },
                )?;
                Ok(CatalogResult {
                    catalog: c,
                    source: Source::Network,
                    fetched: now,
                })
            }
            Err(e) => fall_back(
                format!("The catalog could not be read: {}", e.message),
                cached,
            ),
        },
        other => fall_back(
            format!("The catalog server answered with an error ({other})."),
            cached,
        ),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResult {
    pub release: CoreRelease,
    pub source: Source,
    pub fetched: u64,
}

fn core_file(cache: &Path, core: &CatalogCore) -> PathBuf {
    // The id was validated by the catalog parser (no separators); the lowercase name keeps case-folding file systems consistent.
    root(cache).join("releases").join(format!(
        "{}.json",
        core.id.to_ascii_lowercase().replace(' ', "_")
    ))
}

/// Plain words for GitHub's limit, with the reset time as minutes from `now`.
fn limit_message(reset: Option<u64>, now: u64) -> String {
    match reset {
        Some(r) if r > now => format!(
            "GitHub allows 60 lookups an hour and that limit was reached. Try again in {} minutes.",
            (r - now).div_ceil(60)
        ),
        _ => "GitHub is limiting lookups from this connection right now. Try again later.".into(),
    }
}

/// The newest release of `core`'s repository, cached for [`RELEASE_TTL`]; a stale copy is returned (labelled) when
/// the lookup fails or the limit is hit.
pub fn fetch_release(
    t: &dyn Transport,
    cache: &Path,
    core: &CatalogCore,
    now: u64,
    force: bool,
) -> Result<ReleaseResult, TauError> {
    let file = core_file(cache, core);
    let cached = read_cached(&file).and_then(|(b, m)| {
        catalog_release::parse_release(&b, core)
            .ok()
            .map(|r| (r, m))
    });
    if let Some((r, m)) = &cached
        && !force
        && now.saturating_sub(m.fetched) < RELEASE_TTL
    {
        return Ok(ReleaseResult {
            release: r.clone(),
            source: Source::Cache,
            fetched: m.fetched,
        });
    }
    let etag = cached.as_ref().and_then(|(_, m)| m.etag.clone());
    let fall_back = |why: String, cached: Option<(CoreRelease, Meta)>| match cached {
        Some((r, m)) => Ok(ReleaseResult {
            release: r,
            source: Source::Offline(why),
            fetched: m.fetched,
        }),
        None => Err(err(ErrorCode::Io, why)),
    };
    let r = match t.get(
        &catalog_release::latest_release_url(core),
        etag.as_deref(),
        RELEASE_LIMIT,
    ) {
        Ok(r) => r,
        Err(e) => return fall_back(e.message, cached),
    };
    match r.status {
        304 => match cached {
            Some((rel, m)) => {
                let _ = write_cached(
                    &file,
                    &fs::read(&file).unwrap_or_default(),
                    &Meta {
                        etag: m.etag,
                        fetched: now,
                    },
                );
                Ok(ReleaseResult {
                    release: rel,
                    source: Source::NotModified,
                    fetched: now,
                })
            }
            None => Err(err(
                ErrorCode::Io,
                "The server said the release is unchanged, but there is no saved copy.",
            )),
        },
        200 => match catalog_release::parse_release(&r.body, core) {
            Ok(rel) => {
                write_cached(
                    &file,
                    &r.body,
                    &Meta {
                        etag: r.etag,
                        fetched: now,
                    },
                )?;
                Ok(ReleaseResult {
                    release: rel,
                    source: Source::Network,
                    fetched: now,
                })
            }
            // A release that cannot be used (no zip, foreign URLs) is the answer, not an outage: do not hide it behind an old copy.
            Err(e) => Err(e),
        },
        404 => Err(err(
            ErrorCode::NotFound,
            "The author has not published a download yet.",
        )),
        403 | 429 => fall_back(limit_message(r.rate_reset, now), cached),
        other => fall_back(format!("GitHub answered with an error ({other})."), cached),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Checksum {
    /// GitHub's published SHA-256 matched the bytes.
    Verified,
    /// The release publishes no checksum: the bytes are what GitHub served, nothing more is known.
    NotPublished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreDownloaded {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub checksum: Checksum,
}

/// Downloads the release zip into `dest_dir` (`<id>.<tag>.zip`, written as `.part` first). The URL must belong to the
/// core's own repository; the size must match what GitHub lists; a published digest must match the bytes.
pub fn download_core(
    t: &dyn Transport,
    core: &CatalogCore,
    release: &CoreRelease,
    dest_dir: &Path,
    max_bytes: u64,
) -> Result<CoreDownloaded, TauError> {
    let d = &release.download;
    if !catalog_release::url_allowed(core, &d.url) {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "That download is not from this core's own repository.",
        ));
    }
    if d.size > max_bytes {
        return Err(err(
            ErrorCode::InstallRefused,
            format!(
                "The download is {} MB, over the {} MB limit.",
                d.size >> 20,
                max_bytes >> 20
            ),
        ));
    }
    fs::create_dir_all(dest_dir)?;
    let safe_tag: String = release
        .tag
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dest_dir.join(format!("{}.{safe_tag}.zip", core.id.replace(' ', "_")));
    let part = path.with_extension("zip.part");
    let (status, bytes, sha) = t.download(&d.url, max_bytes, &part)?;
    let fail = |msg: String| {
        let _ = fs::remove_file(&part);
        Err(err(ErrorCode::Io, msg))
    };
    if status != 200 {
        return fail(match status {
            404 => "That release file is no longer on GitHub.".into(),
            403 | 429 => limit_message(None, 0),
            s => format!("GitHub answered with an error ({s})."),
        });
    }
    if d.size != 0 && bytes != d.size {
        return fail(format!(
            "The download is {bytes} bytes but the release lists {}: it was cut short or changed.",
            d.size
        ));
    }
    let checksum = match &d.sha256 {
        Some(want) if *want != sha => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const CORES: &[u8] = include_bytes!("../../../testdata/openfpga/cores.json");
    const RELEASE: &[u8] = include_bytes!("../../../testdata/openfpga/arduboy_release.json");

    /// Scripted answers, in order; records what was asked.
    #[derive(Default)]
    struct Fake {
        script: RefCell<Vec<Result<Response, TauError>>>,
        asked: RefCell<Vec<(String, Option<String>)>>,
        file: RefCell<Vec<u8>>,
    }

    impl Fake {
        fn push(&self, r: Result<Response, TauError>) -> &Self {
            self.script.borrow_mut().push(r);
            self
        }
    }

    fn ok(status: u16, body: &[u8], etag: Option<&str>) -> Result<Response, TauError> {
        Ok(Response {
            status,
            etag: etag.map(str::to_string),
            body: body.to_vec(),
            ..Response::default()
        })
    }

    impl Transport for Fake {
        fn get(&self, url: &str, inm: Option<&str>, _limit: u64) -> Result<Response, TauError> {
            self.asked
                .borrow_mut()
                .push((url.to_string(), inm.map(str::to_string)));
            self.script.borrow_mut().remove(0)
        }
        fn download(
            &self,
            url: &str,
            _limit: u64,
            dest: &Path,
        ) -> Result<(u16, u64, String), TauError> {
            self.asked.borrow_mut().push((url.to_string(), None));
            let body = self.file.borrow().clone();
            fs::write(dest, &body)?;
            Ok((
                200,
                body.len() as u64,
                format!("{:x}", Sha256::digest(&body)),
            ))
        }
    }

    fn scratch() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tau-mkt-{}-{}",
            std::process::id(),
            tau_core_uniq()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn tau_core_uniq() -> u128 {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        u128::from(N.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }
    fn arduboy() -> CatalogCore {
        catalog::parse(CORES)
            .unwrap()
            .find("agg23.Arduboy")
            .unwrap()
            .clone()
    }
    fn offline() -> Result<Response, TauError> {
        Err(err(
            ErrorCode::Io,
            "Could not reach the server. Are you offline?",
        ))
    }

    #[test]
    fn catalog_is_cached_revalidated_and_survives_offline() {
        let (dir, f) = (scratch(), Fake::default());
        f.push(ok(200, CORES, Some("\"v1\"")));
        let r = fetch_catalog(&f, &dir, 1000, false).unwrap();
        assert_eq!(
            (r.source.clone(), r.catalog.cores.len()),
            (Source::Network, 43)
        );
        // inside the TTL: no request at all
        assert_eq!(
            fetch_catalog(&f, &dir, 1000 + 60, false).unwrap().source,
            Source::Cache
        );
        assert_eq!(f.asked.borrow().len(), 1);
        // after the TTL: conditional request with the saved ETag; 304 keeps the copy and renews the age
        f.push(ok(304, b"", None));
        let r = fetch_catalog(&f, &dir, 1000 + CATALOG_TTL + 1, false).unwrap();
        assert_eq!(r.source, Source::NotModified);
        assert_eq!(f.asked.borrow()[1].1.as_deref(), Some("\"v1\""));
        assert_eq!(
            fetch_catalog(&f, &dir, 1000 + CATALOG_TTL + 100, false)
                .unwrap()
                .source,
            Source::Cache
        );
        // forced refresh while offline: the saved copy, labelled with the reason
        f.push(offline());
        let r = fetch_catalog(&f, &dir, 1000 + CATALOG_TTL + 200, true).unwrap();
        assert!(matches!(&r.source, Source::Offline(why) if why.contains("offline")));
        assert_eq!(r.catalog.cores.len(), 43);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn catalog_offline_without_a_copy_is_an_error_and_garbage_never_replaces_a_good_copy() {
        let (dir, f) = (scratch(), Fake::default());
        f.push(offline());
        assert!(fetch_catalog(&f, &dir, 1, false).is_err());
        f.push(ok(200, CORES, None));
        fetch_catalog(&f, &dir, 10, false).unwrap();
        f.push(ok(200, b"<html>captive portal</html>", Some("x")));
        let r = fetch_catalog(&f, &dir, 10 + CATALOG_TTL + 1, false).unwrap();
        assert!(matches!(r.source, Source::Offline(_)));
        assert_eq!(r.catalog.cores.len(), 43);
        f.push(ok(500, b"", None));
        assert!(matches!(
            fetch_catalog(&f, &dir, 10 + CATALOG_TTL + 2, true)
                .unwrap()
                .source,
            Source::Offline(_)
        ));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn release_lookup_is_cached_and_rate_limits_are_explained() {
        let (dir, f, core) = (scratch(), Fake::default(), arduboy());
        f.push(ok(200, RELEASE, Some("\"r1\"")));
        let r = fetch_release(&f, &dir, &core, 5000, false).unwrap();
        assert_eq!(
            (r.source.clone(), r.release.tag.as_str()),
            (Source::Network, "0.9.0")
        );
        assert_eq!(
            fetch_release(&f, &dir, &core, 5100, false).unwrap().source,
            Source::Cache
        );
        assert_eq!(f.asked.borrow().len(), 1);
        // limit hit with a copy saved: the copy, with the minutes to wait
        f.push(Ok(Response {
            status: 403,
            rate_remaining: Some(0),
            rate_reset: Some(5000 + RELEASE_TTL + 600),
            ..Response::default()
        }));
        let r = fetch_release(&f, &dir, &core, 5000 + RELEASE_TTL + 1, false).unwrap();
        assert!(
            matches!(&r.source, Source::Offline(why) if why.contains("10 minutes") || why.contains("9 minutes"))
        );
        // limit hit with no copy: the explanation as the error
        let (dir2, f2) = (scratch(), Fake::default());
        f2.push(Ok(Response {
            status: 429,
            rate_reset: Some(4200),
            ..Response::default()
        }));
        let e = fetch_release(&f2, &dir2, &core, 1000, false).unwrap_err();
        assert!(e.message.contains("54 minutes"), "{}", e.message);
        // no release at all
        f2.push(ok(404, b"{}", None));
        assert!(
            fetch_release(&f2, &dir2, &core, 1000, true)
                .unwrap_err()
                .message
                .contains("not published")
        );
        // a release without a usable zip is the answer, not an outage
        f2.push(ok(200, br#"{"tag_name":"1","assets":[]}"#, None));
        assert!(
            fetch_release(&f2, &dir2, &core, 1000, true)
                .unwrap_err()
                .message
                .contains("no downloadable zip")
        );
        fs::remove_dir_all(dir).ok();
        fs::remove_dir_all(dir2).ok();
    }

    fn release_for(core: &CatalogCore, body: &[u8], sha: Option<String>) -> CoreRelease {
        let mut r = catalog_release::parse_release(RELEASE, core).unwrap();
        r.download.size = body.len() as u64;
        r.download.sha256 = sha;
        r
    }

    #[test]
    fn download_verifies_size_and_checksum_and_labels_the_missing_one() {
        let (dir, f, core) = (scratch(), Fake::default(), arduboy());
        let body = b"PK pretend zip".to_vec();
        *f.file.borrow_mut() = body.clone();
        let good = format!("{:x}", Sha256::digest(&body));
        // no checksum published: allowed, labelled
        let d = download_core(&f, &core, &release_for(&core, &body, None), &dir, 1 << 20).unwrap();
        assert_eq!(d.checksum, Checksum::NotPublished);
        assert_eq!(fs::read(&d.path).unwrap(), body);
        assert!(d.path.ends_with("agg23.Arduboy.0.9.0.zip"));
        // published and matching
        let d = download_core(
            &f,
            &core,
            &release_for(&core, &body, Some(good.clone())),
            &dir,
            1 << 20,
        )
        .unwrap();
        assert_eq!((d.checksum, d.sha256), (Checksum::Verified, good));
        // published and wrong: refused, nothing left behind
        let _ = fs::remove_file(&d.path);
        let e = download_core(
            &f,
            &core,
            &release_for(&core, &body, Some("0".repeat(64))),
            &dir,
            1 << 20,
        )
        .unwrap_err();
        assert!(e.message.contains("checksum"));
        assert!(
            fs::read_dir(&dir).unwrap().next().is_none(),
            "no .part or zip left"
        );
        // listed size differs from what arrived
        let mut r = release_for(&core, &body, None);
        r.download.size += 5;
        assert!(
            download_core(&f, &core, &r, &dir, 1 << 20)
                .unwrap_err()
                .message
                .contains("cut short")
        );
        // over the limit, and a URL from another repository
        assert!(download_core(&f, &core, &release_for(&core, &body, None), &dir, 3).is_err());
        let mut r = release_for(&core, &body, None);
        r.download.url = "https://github.com/evil/x/releases/download/1/a.zip".into();
        assert_eq!(
            download_core(&f, &core, &r, &dir, 1 << 20)
                .unwrap_err()
                .code,
            ErrorCode::InvalidPathReference
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn only_the_three_hosts_are_ever_contacted() {
        for url in [
            "https://example.com/a",
            "http://api.github.com/repos/x/y/releases/latest",
            "file:///etc/passwd",
            "https://api.github.com/user",
        ] {
            assert_eq!(
                Http.get(url, None, 10).unwrap_err().code,
                ErrorCode::InvalidPathReference,
                "{url}"
            );
        }
    }
}
