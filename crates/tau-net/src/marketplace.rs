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
    catalog::{self, CATALOG_URL, Catalog, CatalogCore, PLATFORMS_URL},
    catalog_release::{self, CoreRelease},
};

/// The v3 core list is about 4.5 MB (all releases of 333 cores); the platform list is small.
const CATALOG_LIMIT: u64 = 16 * 1024 * 1024;
const PLATFORMS_LIMIT: u64 = 1024 * 1024;
const DIGEST_LIMIT: u64 = 2 * 1024 * 1024;
const ART_LIMIT: u64 = 200 * 1024;
/// The catalog changes a few times a day; do not ask more often than this unless the user presses Refresh.
pub const CATALOG_TTL: u64 = 6 * 3600;
/// A checksum lookup is repeated at most once an hour per release (GitHub allows 60 unauthenticated requests an hour).
pub const RELEASE_TTL: u64 = 3600;
/// Art on the library site is static: ask again after a week (with the ETag, so an unchanged picture costs no download).
pub const ART_TTL: u64 = 7 * 24 * 3600;

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
        || url.starts_with("https://raw.githubusercontent.com/")
        || url.starts_with("https://user-images.githubusercontent.com/")
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

struct Fetched {
    body: Vec<u8>,
    source: Source,
    fetched: u64,
}

/// Plain words for GitHub's limit, with the reset time as minutes from `now`.
pub(crate) fn limit_message(reset: Option<u64>, now: u64) -> String {
    match reset {
        Some(r) if r > now => format!(
            "GitHub allows 60 lookups an hour and that limit was reached. Try again in {} minutes.",
            (r - now).div_ceil(60)
        ),
        _ => "GitHub is limiting lookups from this connection right now. Try again later.".into(),
    }
}

/// One cached GET: served from the saved copy inside `ttl`, revalidated with its ETag after, the saved copy (labelled)
/// when the request fails, and only a body that `valid` accepts ever replaces the saved one. A 404 is `NotFound` and is
/// never hidden behind an old copy; any other failure falls back to the saved copy when there is one.
#[allow(clippy::too_many_arguments)]
fn fetch_cached(
    t: &dyn Transport,
    file: &Path,
    url: &str,
    limit: u64,
    ttl: u64,
    now: u64,
    force: bool,
    valid: &dyn Fn(&[u8]) -> Result<(), String>,
) -> Result<Fetched, TauError> {
    let cached = read_cached(file).filter(|(b, _)| valid(b).is_ok());
    if let Some((body, m)) = &cached
        && !force
        && now.saturating_sub(m.fetched) < ttl
    {
        return Ok(Fetched {
            body: body.clone(),
            source: Source::Cache,
            fetched: m.fetched,
        });
    }
    let etag = cached.as_ref().and_then(|(_, m)| m.etag.clone());
    let fall_back = |why: String, cached: Option<(Vec<u8>, Meta)>| match cached {
        Some((body, m)) => Ok(Fetched {
            body,
            source: Source::Offline(why),
            fetched: m.fetched,
        }),
        None => Err(err(ErrorCode::Io, why)),
    };
    let r = match t.get(url, etag.as_deref(), limit) {
        Ok(r) => r,
        Err(e) => return fall_back(e.message, cached),
    };
    match r.status {
        304 => match cached {
            Some((body, m)) => {
                let _ = write_cached(
                    file,
                    &body,
                    &Meta {
                        etag: m.etag,
                        fetched: now,
                    },
                );
                Ok(Fetched {
                    body,
                    source: Source::NotModified,
                    fetched: now,
                })
            }
            None => Err(err(
                ErrorCode::Io,
                "The server said the file is unchanged, but there is no saved copy.",
            )),
        },
        200 => match valid(&r.body) {
            Ok(()) => {
                write_cached(
                    file,
                    &r.body,
                    &Meta {
                        etag: r.etag,
                        fetched: now,
                    },
                )?;
                Ok(Fetched {
                    body: r.body,
                    source: Source::Network,
                    fetched: now,
                })
            }
            Err(why) => fall_back(format!("The file could not be read: {why}"), cached),
        },
        404 => Err(err(ErrorCode::NotFound, "That file is not on the server.")),
        403 | 429 => fall_back(limit_message(r.rate_reset, now), cached),
        other => fall_back(
            format!("The server answered with an error ({other})."),
            cached,
        ),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogResult {
    pub catalog: Catalog,
    pub source: Source,
    /// Unix seconds of the last successful fetch of the core list.
    pub fetched: u64,
    /// Plain-words notes about other marketplace sources that could not be added to the list (the Library list is still shown).
    #[serde(default)]
    pub notes: Vec<String>,
}

/// The catalog (v3 core list plus platform list): cached for [`CATALOG_TTL`], revalidated with ETags, and the saved
/// copies when offline.
pub fn fetch_catalog(
    t: &dyn Transport,
    cache: &Path,
    now: u64,
    force: bool,
) -> Result<CatalogResult, TauError> {
    let dir = root(cache);
    let platforms = fetch_cached(
        t,
        &dir.join("platforms.json"),
        PLATFORMS_URL,
        PLATFORMS_LIMIT,
        CATALOG_TTL,
        now,
        force,
        &|b| {
            serde_json::from_slice::<serde_json::Value>(b)
                .map(|_| ())
                .map_err(|e| e.to_string())
        },
    )?;
    let cores = fetch_cached(
        t,
        &dir.join("cores.json"),
        CATALOG_URL,
        CATALOG_LIMIT,
        CATALOG_TTL,
        now,
        force,
        &|b| {
            catalog::parse(b, &platforms.body)
                .map(|_| ())
                .map_err(|e| e.message)
        },
    )?;
    let catalog = catalog::parse(&cores.body, &platforms.body)?;
    // Either file being a saved copy makes the list a saved copy.
    let source = match (&cores.source, &platforms.source) {
        (Source::Offline(w), _) | (_, Source::Offline(w)) => Source::Offline(w.clone()),
        (s, _) => s.clone(),
    };
    Ok(CatalogResult {
        catalog,
        source,
        fetched: cores.fetched,
        notes: Vec::new(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResult {
    pub release: CoreRelease,
    pub source: Source,
    pub fetched: u64,
}

/// The release to install (newest unless `version` is given), straight from the catalog; when `lookup_digest` is true and
/// the address is a GitHub release asset, one best-effort API request adds the size and SHA-256 GitHub publishes for it
/// (cached for [`RELEASE_TTL`]). The lookup failing for any reason (offline, rate limit, no digest) is not an error: the
/// release simply has no checksum and the plan shows the notice.
pub fn resolve_release(
    t: &dyn Transport,
    cache: &Path,
    core: &CatalogCore,
    version: Option<&str>,
    now: u64,
    lookup_digest: bool,
) -> Result<ReleaseResult, TauError> {
    let mut release = catalog_release::release_from_catalog(core, version)?;
    let mut source = Source::Cache;
    let mut fetched = now;
    if lookup_digest && let Some(api) = catalog_release::digest_lookup_url(&release.download.url) {
        let key = format!("{:x}", Sha256::digest(api.as_bytes()));
        let file = root(cache)
            .join("digests")
            .join(format!("{}.json", &key[..24]));
        let name = release.download.name.clone();
        if let Ok(got) = fetch_cached(
            t,
            &file,
            &api,
            DIGEST_LIMIT,
            RELEASE_TTL,
            now,
            false,
            &|b| {
                catalog_release::asset_digest(b, &name)
                    .map(|_| ())
                    .ok_or_else(|| "the release does not list this file".to_string())
            },
        ) && let Some((size, sha)) = catalog_release::asset_digest(&got.body, &name)
        {
            release.download.size = size;
            release.download.sha256 = sha;
            source = got.source;
            fetched = got.fetched;
        }
    }
    Ok(ReleaseResult {
        release,
        source,
        fetched,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Checksum {
    /// GitHub's published SHA-256 matched the bytes.
    Verified,
    /// No checksum is published for this file: the bytes are what GitHub served, nothing more is known.
    NotPublished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreDownloaded {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub checksum: Checksum,
}

/// Downloads the release zip into `dest_dir` (`<id>.<tag>.zip`, written as `.part` first). The address must be one of the
/// two shapes the library uses; when GitHub's size is known it must match; a published digest must match the bytes.
pub fn download_core(
    t: &dyn Transport,
    core: &CatalogCore,
    release: &CoreRelease,
    dest_dir: &Path,
    max_bytes: u64,
) -> Result<CoreDownloaded, TauError> {
    let d = &release.download;
    if catalog_release::parse_download_url(&d.url).is_none() {
        return Err(err(
            ErrorCode::InvalidPathReference,
            "That download address is not one Omega accepts.",
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
    let path = dest_dir.join(format!(
        "{}.{}.zip",
        core.id.replace(' ', "_"),
        &safe_tag[..safe_tag.len().min(40)]
    ));
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

// ---- art -------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtKind {
    /// The 36x36 core icon.
    Icon,
    /// The 521x165 platform banner.
    Banner,
}

/// A PNG of the expected shape and size, else a reason. The library's art is plain grayscale; anything else is refused
/// rather than handed to the UI.
fn check_png(b: &[u8], kind: ArtKind) -> Result<(), String> {
    if b.len() < 33 || &b[..8] != b"\x89PNG\r\n\x1a\n" || &b[12..16] != b"IHDR" {
        return Err("not a PNG".into());
    }
    let (w, h) = (
        u32::from_be_bytes([b[16], b[17], b[18], b[19]]),
        u32::from_be_bytes([b[20], b[21], b[22], b[23]]),
    );
    let ok = match kind {
        ArtKind::Icon => (w, h) == (36, 36),
        ArtKind::Banner => w <= 640 && h <= 240 && w >= 100,
    };
    ok.then_some(())
        .ok_or_else(|| format!("unexpected picture size {w}x{h}"))
}

/// A GitHub account's profile picture (`https://github.com/<owner>.png`, which GitHub redirects to its own image host), as PNG or
/// JPEG bytes, cached for a week. Only a plain login is accepted (letters, digits, hyphens). `Ok(None)` when there is none.
pub fn fetch_avatar(
    t: &dyn Transport,
    cache: &Path,
    owner: &str,
    now: u64,
) -> Result<Option<Vec<u8>>, TauError> {
    if owner.is_empty()
        || owner.len() > 39
        || !owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Ok(None);
    }
    let dir = root(cache).join("art");
    let file = dir.join(format!("avatar-{}.img", owner.to_ascii_lowercase()));
    let url = format!("https://github.com/{owner}.png?size=72");
    let image = |b: &[u8]| {
        if b.starts_with(b"\x89PNG\r\n\x1a\n") || b.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Ok(())
        } else {
            Err("not a PNG or JPEG".to_string())
        }
    };
    match fetch_cached(t, &file, &url, ART_LIMIT, ART_TTL, now, false, &image) {
        Ok(f) => Ok(Some(f.body)),
        Err(e) if e.code == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// The core's icon or its platform's banner (PNG bytes), cached for a week and revalidated with the ETag. `Ok(None)` when
/// the library has no picture for it (a 404 is remembered for the same week so it is not asked again).
pub fn fetch_art(
    t: &dyn Transport,
    cache: &Path,
    core: &CatalogCore,
    kind: ArtKind,
    now: u64,
    force: bool,
) -> Result<Option<Vec<u8>>, TauError> {
    let (url, name) = match kind {
        ArtKind::Icon => (
            core.icon_url(),
            format!("icon-{}", core.id.to_ascii_lowercase()),
        ),
        ArtKind::Banner => (core.banner_url(), format!("banner-{}", core.platform_id)),
    };
    let Some(url) = url else { return Ok(None) };
    let dir = root(cache).join("art");
    let file = dir.join(format!("{name}.png"));
    let missing = dir.join(format!("{name}.none"));
    if !force
        && let Ok(m) = fs::metadata(&missing)
        && m.modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() < ART_TTL)
    {
        return Ok(None);
    }
    match fetch_cached(t, &file, &url, ART_LIMIT, ART_TTL, now, force, &|b| {
        check_png(b, kind)
    }) {
        Ok(f) => Ok(Some(f.body)),
        Err(e) if e.code == ErrorCode::NotFound => {
            fs::create_dir_all(&dir)?;
            fs::write(&missing, b"")?;
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const CORES: &[u8] = include_bytes!("../../../testdata/openfpga/cores_v3.json");
    const PLATFORMS: &[u8] = include_bytes!("../../../testdata/openfpga/platforms_v3.json");
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
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let d = std::env::temp_dir().join(format!(
            "tau-mkt-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn arduboy() -> CatalogCore {
        catalog::parse(CORES, PLATFORMS)
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
    /// A tiny valid PNG header of the given size (the checks read only the signature and IHDR).
    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        v.extend(w.to_be_bytes());
        v.extend(h.to_be_bytes());
        v.extend([8, 0, 0, 0, 0, 0, 0, 0, 0]);
        v
    }

    #[test]
    fn catalog_is_cached_revalidated_and_survives_offline() {
        let (dir, f) = (scratch(), Fake::default());
        f.push(ok(200, PLATFORMS, Some("\"p1\"")))
            .push(ok(200, CORES, Some("\"v1\"")));
        let r = fetch_catalog(&f, &dir, 1000, false).unwrap();
        assert_eq!(
            (r.source.clone(), r.catalog.cores.len()),
            (Source::Network, 44)
        );
        // inside the TTL: no request at all
        assert_eq!(
            fetch_catalog(&f, &dir, 1000 + 60, false).unwrap().source,
            Source::Cache
        );
        assert_eq!(f.asked.borrow().len(), 2);
        // after the TTL: conditional requests with the saved ETags; 304 keeps the copy and renews the age
        f.push(ok(304, b"", None)).push(ok(304, b"", None));
        let r = fetch_catalog(&f, &dir, 1000 + CATALOG_TTL + 1, false).unwrap();
        assert_eq!(r.source, Source::NotModified);
        assert_eq!(f.asked.borrow()[2].1.as_deref(), Some("\"p1\""));
        assert_eq!(f.asked.borrow()[3].1.as_deref(), Some("\"v1\""));
        assert_eq!(
            fetch_catalog(&f, &dir, 1000 + CATALOG_TTL + 100, false)
                .unwrap()
                .source,
            Source::Cache
        );
        // forced refresh while offline: the saved copy, labelled with the reason
        f.push(offline()).push(offline());
        let r = fetch_catalog(&f, &dir, 1000 + CATALOG_TTL + 200, true).unwrap();
        assert!(matches!(&r.source, Source::Offline(why) if why.contains("offline")));
        assert_eq!(r.catalog.cores.len(), 44);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn catalog_offline_without_a_copy_is_an_error_and_garbage_never_replaces_a_good_copy() {
        let (dir, f) = (scratch(), Fake::default());
        f.push(offline());
        assert!(fetch_catalog(&f, &dir, 1, false).is_err());
        f.push(ok(200, PLATFORMS, None)).push(ok(200, CORES, None));
        fetch_catalog(&f, &dir, 10, false).unwrap();
        f.push(ok(200, PLATFORMS, None))
            .push(ok(200, b"<html>captive portal</html>", Some("x")));
        let r = fetch_catalog(&f, &dir, 10 + CATALOG_TTL + 1, false).unwrap();
        assert!(matches!(r.source, Source::Offline(_)));
        assert_eq!(r.catalog.cores.len(), 44);
        f.push(ok(200, PLATFORMS, None)).push(ok(500, b"", None));
        assert!(matches!(
            fetch_catalog(&f, &dir, 10 + CATALOG_TTL + 2, true)
                .unwrap()
                .source,
            Source::Offline(_)
        ));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn the_release_needs_no_request_and_the_checksum_lookup_is_best_effort() {
        let (dir, f, core) = (scratch(), Fake::default(), arduboy());
        // no lookup asked for: nothing is requested
        let r = resolve_release(&f, &dir, &core, None, 5000, false).unwrap();
        assert_eq!(
            (r.release.tag.as_str(), r.release.download.sha256.clone()),
            ("0.9.0", None)
        );
        assert!(f.asked.borrow().is_empty());
        // lookup: the size and (absent) digest of the real release
        f.push(ok(200, RELEASE, Some("\"r1\"")));
        let r = resolve_release(&f, &dir, &core, None, 5000, true).unwrap();
        assert_eq!(
            (
                r.release.download.size,
                r.release.download.sha256.clone(),
                r.source.clone()
            ),
            (257_119, None, Source::Network)
        );
        assert_eq!(
            f.asked.borrow()[0].0,
            "https://api.github.com/repos/agg23/openfpga-arduboy/releases/tags/0.9.0"
        );
        // cached for the hour
        assert_eq!(
            resolve_release(&f, &dir, &core, None, 5100, true)
                .unwrap()
                .source,
            Source::Cache
        );
        assert_eq!(f.asked.borrow().len(), 1);
        // limit hit, offline, 404: still a release, with no checksum
        let dir2 = scratch();
        for answer in [
            Ok(Response {
                status: 403,
                rate_reset: Some(9999),
                ..Response::default()
            }),
            offline(),
            ok(404, b"{}", None),
            ok(200, b"{\"assets\":[]}", None),
        ] {
            f.push(answer);
            let r = resolve_release(&f, &dir2, &core, None, 5000, true).unwrap();
            assert_eq!(
                (r.release.download.size, r.release.download.sha256),
                (0, None)
            );
        }
        fs::remove_dir_all(dir).ok();
        fs::remove_dir_all(dir2).ok();
    }

    #[test]
    fn a_commit_pinned_download_never_asks_github_for_a_checksum() {
        let (dir, f) = (scratch(), Fake::default());
        let mut core = arduboy();
        core.history[0].download_url = "https://raw.githubusercontent.com/o/r/49b4091fb8d71116eb5920a88aae72b910322fa4/Releases/a.zip".into();
        let r = resolve_release(&f, &dir, &core, None, 1, true).unwrap();
        assert!(!r.release.is_release && f.asked.borrow().is_empty());
        fs::remove_dir_all(dir).ok();
    }

    fn release_for(core: &CatalogCore, body: &[u8], sha: Option<String>) -> CoreRelease {
        let mut r = catalog_release::release_from_catalog(core, None).unwrap();
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
        // size not known yet (catalog only): accepted
        let mut unknown = release_for(&core, &body, None);
        unknown.download.size = 0;
        assert!(download_core(&f, &core, &unknown, &dir, 1 << 20).is_ok());
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
        fs::remove_file(&d.path).ok();
        let _ = fs::remove_file(dir.join("agg23.Arduboy.0.9.0.zip"));
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
        // over the limit, and an address that is not a GitHub zip
        let mut big = release_for(&core, &body, None);
        big.download.size = 5 << 20;
        assert!(download_core(&f, &core, &big, &dir, 3).is_err());
        let mut r = release_for(&core, &body, None);
        r.download.url = "https://evil.example/a.zip".into();
        assert_eq!(
            download_core(&f, &core, &r, &dir, 1 << 20)
                .unwrap_err()
                .code,
            ErrorCode::InvalidPathReference
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_github_avatar_is_fetched_once_checked_for_an_image_and_only_for_a_plain_login() {
        let (dir, f) = (scratch(), Fake::default());
        // Not a login: nothing is asked.
        for bad in ["", "../x", "a/b", "name with space", &"a".repeat(40)] {
            assert!(fetch_avatar(&f, &dir, bad, 1).unwrap().is_none(), "{bad:?}");
        }
        assert!(f.asked.borrow().is_empty());
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];
        f.push(ok(200, &jpeg, None));
        assert_eq!(
            fetch_avatar(&f, &dir, "alfatreze", 10).unwrap().unwrap(),
            jpeg
        );
        assert_eq!(
            f.asked.borrow()[0].0,
            "https://github.com/alfatreze.png?size=72"
        );
        // inside the week it comes from the cache
        assert!(fetch_avatar(&f, &dir, "alfatreze", 20).unwrap().is_some());
        assert_eq!(f.asked.borrow().len(), 1);
        // something that is not an image is refused
        let g = Fake::default();
        g.push(ok(200, b"<html>", None));
        assert!(fetch_avatar(&g, &scratch(), "someone", 1).is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn art_is_cached_validated_and_a_missing_picture_is_remembered() {
        let (dir, f, core) = (scratch(), Fake::default(), arduboy());
        f.push(ok(200, &png(36, 36), Some("\"a\"")));
        assert_eq!(
            fetch_art(&f, &dir, &core, ArtKind::Icon, 100, false)
                .unwrap()
                .unwrap()
                .len(),
            png(36, 36).len()
        );
        assert_eq!(
            f.asked.borrow()[0].0,
            "https://openfpga-library.github.io/analogue-pocket/assets/images/authors/agg23.Arduboy.png"
        );
        // inside the week: no request
        assert!(
            fetch_art(&f, &dir, &core, ArtKind::Icon, 200, false)
                .unwrap()
                .is_some()
        );
        assert_eq!(f.asked.borrow().len(), 1);
        // after the week: conditional, 304 keeps it
        f.push(ok(304, b"", None));
        assert!(
            fetch_art(&f, &dir, &core, ArtKind::Icon, 100 + ART_TTL + 1, false)
                .unwrap()
                .is_some()
        );
        assert_eq!(f.asked.borrow()[1].1.as_deref(), Some("\"a\""));
        // a banner of the wrong shape, or something that is not a PNG, is refused and nothing is saved
        let (dir2, g) = (scratch(), Fake::default());
        g.push(ok(200, &png(36, 36), None));
        assert!(fetch_art(&g, &dir2, &core, ArtKind::Banner, 1, false).is_err());
        g.push(ok(200, b"<html>", None));
        assert!(fetch_art(&g, &dir2, &core, ArtKind::Icon, 1, false).is_err());
        // a 404 means "no picture", and is not asked again inside the week
        g.push(ok(404, b"", None));
        assert!(
            fetch_art(&g, &dir2, &core, ArtKind::Banner, 1, false)
                .unwrap()
                .is_none()
        );
        let asked = g.asked.borrow().len();
        assert!(
            fetch_art(&g, &dir2, &core, ArtKind::Banner, 2, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(g.asked.borrow().len(), asked);
        // a name that is not a plain file name never reaches the network
        let mut odd = core.clone();
        odd.id = "a.B C".into();
        assert!(
            fetch_art(&g, &dir2, &odd, ArtKind::Icon, 1, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(g.asked.borrow().len(), asked);
        fs::remove_dir_all(dir).ok();
        fs::remove_dir_all(dir2).ok();
    }

    #[test]
    fn only_the_known_hosts_are_ever_contacted() {
        for url in [
            "https://example.com/a",
            "http://api.github.com/repos/x/y/releases/latest",
            "file:///etc/passwd",
            "https://api.github.com/user",
            "https://raw.githubusercontent.evil.com/x",
        ] {
            assert_eq!(
                Http.get(url, None, 10).unwrap_err().code,
                ErrorCode::InvalidPathReference,
                "{url}"
            );
        }
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Talks to the real library site and GitHub (read-only, one small download): `cargo test -p tau-net -- --ignored live_marketplace --nocapture`.
    #[test]
    #[ignore]
    fn live_marketplace_catalog_art_and_a_verified_download() {
        let dir = std::env::temp_dir().join("tau-marketplace-live");
        let _ = fs::remove_dir_all(&dir);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let cat = fetch_catalog(&Http, &dir, now, true).unwrap();
        assert!(cat.catalog.cores.len() > 300, "{}", cat.catalog.cores.len());
        let core = cat.catalog.find("HarpMudd.Berzerk").unwrap();
        for kind in [ArtKind::Icon, ArtKind::Banner] {
            let png = fetch_art(&Http, &dir, core, kind, now, true)
                .unwrap()
                .expect("the library has art for this core");
            println!("{kind:?}: {} bytes", png.len());
        }
        let rel = resolve_release(&Http, &dir, core, Some("1.1.0"), now, true).unwrap();
        assert!(
            rel.release.download.sha256.is_some(),
            "GitHub publishes a digest for this asset"
        );
        let got = download_core(&Http, core, &rel.release, &dir.join("dl"), 8 << 20).unwrap();
        assert_eq!(got.checksum, Checksum::Verified);
        let vetted = tau_core::core_package::vet_zip(
            Path::new(&got.path),
            &core.id,
            tau_core::core_package::Limits::default(),
        )
        .unwrap();
        println!(
            "{} {} vetted, {} files, firmware {}",
            vetted.core_id, vetted.version, vetted.files, vetted.firmware_required
        );
        let _ = fs::remove_dir_all(dir);
    }
}
