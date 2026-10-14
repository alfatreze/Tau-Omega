//! The core marketplace from the command line (same engine and cache as the app): browse the openFPGA Library
//! catalog, look up one core's release, compare a card's cores with the catalog, and download-and-vet a core zip.
//! Nothing here writes to a card: a vetted zip is installed with `package-plan` / `package-install` like any other.

use crate::{CliError, value};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tau_core::{
    catalog::{Catalog, CatalogCore, Filter, Sort},
    core_package::{self, Limits},
    core_update,
};
use tau_net::marketplace::{self, Http, Source};

fn cache_dir(a: &[String]) -> PathBuf {
    value(a, "--cache")
        .map(PathBuf::from)
        .or_else(tau_core::ledger_host::default_cache_dir)
        .unwrap_or_else(|| std::env::temp_dir().join("tau-omega-cli"))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn emit<T: Serialize>(j: bool, data: &T, text: impl FnOnce() -> String) -> Result<(), CliError> {
    if j {
        println!(
            "{}",
            serde_json::to_string_pretty(data).map_err(|e| CliError::Usage(e.to_string()))?
        );
    } else {
        println!("{}", text());
    }
    Ok(())
}

fn refresh(a: &[String]) -> bool {
    a.iter().any(|x| x == "--refresh")
}

fn load_catalog(a: &[String]) -> Result<(Catalog, Source), CliError> {
    let r = marketplace::fetch_catalog(&Http, &cache_dir(a), now(), refresh(a))?;
    Ok((r.catalog, r.source))
}

fn source_note(s: &Source) -> String {
    match s {
        Source::Network => "fetched now".into(),
        Source::NotModified => "unchanged, checked now".into(),
        Source::Cache => "from the saved copy".into(),
        Source::Offline(why) => format!("saved copy ({why})"),
    }
}

/// `tau catalog [--search words] [--category c] [--license yes|no] [--sort name|newest] [--limit n] [--refresh] [--cache dir]`
pub fn catalog(a: &[String], j: bool) -> Result<(), CliError> {
    let (cat, source) = load_catalog(a)?;
    let filter = Filter {
        query: value(a, "--search").unwrap_or_default().to_string(),
        category: value(a, "--category").map(str::to_string),
        manufacturer: None,
        needs_license: match value(a, "--license") {
            Some("yes") => Some(true),
            Some("no") => Some(false),
            Some(_) => return Err("--license is yes or no".into()),
            None => None,
        },
    };
    let sort = match value(a, "--sort") {
        Some("name") => Sort::Name,
        Some("newest") | None => Sort::Newest,
        Some(_) => return Err("--sort is name or newest".into()),
    };
    let limit = value(a, "--limit")
        .map_or(Ok(usize::MAX), str::parse)
        .map_err(|_| CliError::Usage("--limit is a number".into()))?;
    let found: Vec<&CatalogCore> = cat.search(&filter, sort).into_iter().take(limit).collect();
    emit(j, &found, || {
        let mut s = format!(
            "{} of {} cores ({})\n",
            found.len(),
            cat.cores.len(),
            source_note(&source)
        );
        for c in &found {
            s += &format!(
                "{:<34} {:<10} {:<12} {}{}\n",
                c.id,
                c.version,
                c.release_date,
                c.category,
                if c.requires_license {
                    "  needs a licence file"
                } else {
                    ""
                }
            );
        }
        s
    })
}

/// `tau catalog-core <id> [--version v] [--refresh]`: the catalog entry (compatibility, where your files go) and the release it would
/// install. Needs only the catalog: no GitHub lookup is made.
pub fn catalog_core(a: &[String], j: bool) -> Result<(), CliError> {
    let id = a
        .first()
        .ok_or("catalog-core needs a core id (for example agg23.Arduboy)")?;
    let (cat, source) = load_catalog(a)?;
    let core = cat
        .find(id)
        .ok_or_else(|| format!("{id} is not in the catalog"))?
        .clone();
    let release = tau_core::catalog_release::release_from_catalog(&core, value(a, "--version"))?;
    #[derive(Serialize)]
    struct Out<'a> {
        core: &'a CatalogCore,
        release: &'a tau_core::catalog_release::CoreRelease,
        source: &'a Source,
    }
    emit(
        j,
        &Out {
            core: &core,
            release: &release,
            source: &source,
        },
        || {
            let yes_no = |v: Option<bool>| match v {
                Some(true) => "yes",
                Some(false) => "no",
                None => "not stated",
            };
            format!(
                "{} {} ({}), {}\n  repository: github.com/{}/{}\n  download: {}\n  needs Pocket firmware {} · Dock: {} · {}{}",
                core.id,
                release.version,
                release.published,
                source_note(&source),
                core.repository.owner,
                core.repository.name,
                release.download.url,
                if core.firmware_required.is_empty() {
                    "not stated"
                } else {
                    &core.firmware_required
                },
                yes_no(core.dock_supported),
                if core.requires_license {
                    if core.license_filename.is_empty() {
                        "needs a licence file (the author names it)".to_string()
                    } else {
                        format!("needs a licence file: {}", core.license_filename)
                    }
                } else {
                    "no licence file needed".into()
                },
                if core.declares_ai.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n  author declares AI assistance: {}",
                        core.declares_ai.join(", ")
                    )
                }
            )
        },
    )
}

/// `tau core-updates <card-root> [--refresh]`: each core on the card against the catalog. Read-only.
pub fn core_updates(a: &[String], j: bool) -> Result<(), CliError> {
    let card = a.first().ok_or("core-updates needs a card root")?;
    let (cat, source) = load_catalog(a)?;
    let inspected = tau_core::inspect_card(Path::new(card))?;
    let rows = core_update::check(&inspected.cores, &cat);
    emit(j, &rows, || {
        let mut s = format!("catalog: {}\n", source_note(&source));
        for r in &rows {
            s += &format!(
                "{:<34} {:<12} -> {:<12} {:?}\n",
                r.id, r.installed_version, r.latest_version, r.status
            );
        }
        s
    })
}

/// `tau core-download <id> --out <folder> [--version v] [--refresh]`: downloads the core's newest release zip (size and checksum
/// checked), then vets it (paths, symlinks, size, identity). Leaves a zip for `package-plan`; writes nothing to a card.
pub fn core_download(a: &[String], j: bool) -> Result<(), CliError> {
    let id = a.first().ok_or("core-download needs a core id")?;
    let out = value(a, "--out").ok_or("core-download needs --out FOLDER")?;
    let (cat, _) = load_catalog(a)?;
    let core = cat
        .find(id)
        .ok_or_else(|| format!("{id} is not in the catalog"))?
        .clone();
    // The address comes from the catalog; one best-effort lookup adds GitHub's checksum and size when it publishes them.
    let rel = marketplace::resolve_release(
        &Http,
        &cache_dir(a),
        &core,
        value(a, "--version"),
        now(),
        true,
    )?;
    let limits = Limits::default();
    let got = marketplace::download_core(
        &Http,
        &core,
        &rel.release,
        Path::new(out),
        limits.max_zip_bytes,
    )?;
    let vetted = core_package::vet_zip(Path::new(&got.path), &core.id, limits);
    if let Err(e) = &vetted {
        // A zip that fails vetting must not stay around looking installable.
        let _ = std::fs::remove_file(&got.path);
        return Err(CliError::Usage(format!("downloaded, but refused: {e}")));
    }
    #[derive(Serialize)]
    struct Out<'a> {
        download: &'a marketplace::CoreDownloaded,
        vetted: &'a core_package::Vetted,
    }
    let v = vetted.map_err(|e| CliError::Usage(e.to_string()))?;
    emit(
        j,
        &Out {
            download: &got,
            vetted: &v,
        },
        || {
            format!(
                "{} -> {} ({} bytes, checksum {})\nvetted: {} {} ({}), needs Pocket firmware {}, {} files ({} asset files)",
                core.id,
                got.path,
                got.bytes,
                if got.checksum == marketplace::Checksum::Verified {
                    "verified"
                } else {
                    "not published"
                },
                v.core_id,
                v.version,
                v.date_release,
                if v.firmware_required.is_empty() {
                    "unspecified"
                } else {
                    &v.firmware_required
                },
                v.files,
                v.asset_files
            )
        },
    )
}
