//! The two verbs that touch the network (through `tau-net`, the only crate that does): look for a newer Tau release on GitHub and
//! download a verified zip. One unauthenticated GET of the public release list per check; no identifier, card name or path is sent.
//! Nothing is installed here: `update-download` leaves a verified zip that `update-plan` / `update` then use.

use crate::{CliError, value};
use serde::Serialize;
use std::path::{Path, PathBuf};

fn cache_dir(a: &[String]) -> PathBuf {
    value(a, "--cache")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("tau-omega-cli"))
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

/// `tau update-check [--card <card-root>] [--cache <dir>]`: is there a newer Tau release? With a card, each Tau core on it is judged
/// against the releases of its own channel. Prints the release, whether it is newer, and the verified zips on offer.
pub fn update_check(a: &[String], j: bool) -> Result<(), CliError> {
    let card = value(a, "--card");
    let cache = cache_dir(a);
    let found = tau_net::check(card, &cache)?;
    emit(j, &found, || match &found {
        Some(c) => format!(
            "{} (newer: {:?}, {} zip{})",
            c.message,
            c.newer,
            c.zips.len(),
            if c.zips.len() == 1 { "" } else { "s" }
        ),
        None => "no release information".to_string(),
    })
}

/// `tau update-download <tag> <zip-name>... --out <folder> [--cache <dir>]`: downloads the named zips of release `<tag>` into
/// `<folder>/<tag>/`, each verified against the release's `SHA256SUMS.txt` (and its `tau-compat.json`). Zips only.
pub fn update_download(a: &[String], j: bool) -> Result<(), CliError> {
    let tag = a
        .first()
        .ok_or("update-download needs a release tag (for example v0.6.0-preview.1)")?;
    let out = value(a, "--out").ok_or("update-download needs --out FOLDER")?;
    let names: Vec<String> = a[1..]
        .iter()
        .take_while(|x| !x.starts_with("--"))
        .cloned()
        .collect();
    if names.is_empty() {
        return Err("update-download needs at least one zip name after the tag".into());
    }
    let got = tau_net::download(tag, &names, Path::new(out).to_path_buf(), &cache_dir(a))?;
    emit(j, &got, || {
        got.iter()
            .map(|d| format!("{} ({} bytes) -> {}", d.name, d.bytes, d.path))
            .collect::<Vec<_>>()
            .join("\n")
    })
}
