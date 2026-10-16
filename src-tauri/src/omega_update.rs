//! Updating Tau Omega itself (`tauri-plugin-updater`). Separate from `updates.rs`, which checks for new *Tau core* releases.
//!
//! Two release channels (Settings > Tau Omega updates): **Stable** reads `latest.json` of the newest published release; **Dev** also reads the
//! rolling `dev-channel` release (published by `tools/publish_dev.sh`) and offers whichever of the two is newer.
//!
//! What leaves the machine: one GET of `latest.json` (two on the Dev channel) from this repository's GitHub releases, and, only when the user presses
//! Update, the signed installer bundle it names. The bundle is verified against the public key in `tauri.conf.json` before it is
//! installed; an unsigned or tampered download is refused. Nothing is ever downloaded or installed without the user's click.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tau_core::{ErrorCode, TauError};

/// True while an update is being installed; `begin_card_write` refuses to start a card write then.
pub static UPDATING: AtomicBool = AtomicBool::new(false);
use tauri_plugin_updater::UpdaterExt;

pub const STABLE_FEED: &str = "https://github.com/alfatreze/Tau-Omega/releases/latest/download/latest.json";
pub const DEV_FEED: &str = "https://github.com/alfatreze/Tau-Omega/releases/download/dev-channel/latest.json";

#[derive(Serialize)]
pub struct OmegaUpdate {
    pub current: String,
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
    /// The channel the check followed: `stable` or `dev`.
    pub channel: String,
}

fn err(message: impl Into<String>) -> TauError {
    TauError { code: ErrorCode::Io, message: message.into() }
}

fn explain(error: &tauri_plugin_updater::Error) -> String {
    let text = error.to_string();
    if text.contains("404") || text.contains("Could not fetch") {
        "No Tau Omega update information was found on GitHub yet (there may be no release with an updater file).".into()
    } else if text.to_lowercase().contains("network") || text.contains("error sending request") {
        "Could not reach GitHub. Check the connection and try again.".into()
    } else {
        format!("The Tau Omega update check failed: {text}")
    }
}

/// The channel a setting (`auto`, `stable`, `dev`) means for the running version: `auto` follows the build (a pre-release build such as `0.5.1-alpha.3`
/// follows Dev, a release follows Stable).
pub fn effective_channel(setting: &str, current: &str) -> &'static str {
    match setting {
        "dev" => "dev",
        "stable" => "stable",
        _ => {
            if current.contains('-') {
                "dev"
            } else {
                "stable"
            }
        }
    }
}

/// The feeds a channel reads, in order.
pub fn feeds(channel: &str) -> Vec<&'static str> {
    if channel == "dev" { vec![DEV_FEED, STABLE_FEED] } else { vec![STABLE_FEED] }
}

/// Orders two version strings like semver: the numbers first, a pre-release (`-alpha.3`) before the release it leads to, and pre-release parts one
/// by one (numbers by value, and before words).
pub fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let split = |v: &str| {
        let v = v.trim_start_matches('v');
        let v = v.split('+').next().unwrap_or(v);
        match v.split_once('-') {
            Some((core, pre)) => (core.to_string(), Some(pre.to_string())),
            None => (v.to_string(), None),
        }
    };
    let (ac, ap) = split(a);
    let (bc, bp) = split(b);
    let nums = |c: &str| c.split('.').map(|n| n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    match nums(&ac).cmp(&nums(&bc)) {
        Ordering::Equal => {}
        other => return other,
    }
    match (ap, bp) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => {
            let (xs, ys): (Vec<_>, Vec<_>) = (x.split('.').collect(), y.split('.').collect());
            for (p, q) in xs.iter().zip(&ys) {
                let ord = match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(m), Ok(n)) => m.cmp(&n),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => p.cmp(q),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            xs.len().cmp(&ys.len())
        }
    }
}

/// The newest update across the channel's feeds (`None` when this build is the newest). A feed that cannot be read is skipped as long as another
/// answers; with none answering the first error is returned.
async fn newest(app: &tauri::AppHandle, channel: &str) -> Result<Option<tauri_plugin_updater::Update>, TauError> {
    let mut best: Option<tauri_plugin_updater::Update> = None;
    let mut first_error: Option<String> = None;
    let mut answered = false;
    for feed in feeds(channel) {
        let url = tauri::Url::parse(feed).map_err(|e| err(e.to_string()))?;
        let updater = match app.updater_builder().endpoints(vec![url]).and_then(|b| b.build()) {
            Ok(u) => u,
            Err(e) => {
                first_error.get_or_insert(explain(&e));
                continue;
            }
        };
        match updater.check().await {
            Ok(found) => {
                answered = true;
                if let Some(u) = found
                    && best.as_ref().is_none_or(|b| cmp_versions(&u.version, &b.version) == std::cmp::Ordering::Greater)
                {
                    best = Some(u);
                }
            }
            Err(e) => {
                first_error.get_or_insert(explain(&e));
            }
        }
    }
    if answered { Ok(best) } else { Err(err(first_error.unwrap_or_else(|| "No update feed answered.".into()))) }
}

fn channel_of(app: &tauri::AppHandle) -> String {
    let setting = crate::read_prefs(app).map(|p| p.omega_channel).unwrap_or_else(|_| "auto".into());
    effective_channel(&setting, &app.package_info().version.to_string()).to_string()
}

/// Looks for a newer Tau Omega on the chosen channel. `None` means this one is current. Downloads nothing.
#[tauri::command]
pub async fn omega_update_check(app: tauri::AppHandle) -> Result<Option<OmegaUpdate>, TauError> {
    let channel = channel_of(&app);
    let found = newest(&app, &channel).await?;
    Ok(found.map(|u| OmegaUpdate { current: u.current_version.to_string(), version: u.version.clone(), notes: u.body.clone(), date: u.date.map(|d| d.to_string()), channel: channel.clone() }))
}

/// Downloads the update found by a fresh check, verifies its signature, installs it and restarts Tau Omega.
/// Refused while a card write is running (the app would be killed halfway through it).
#[tauri::command]
pub async fn omega_update_install(app: tauri::AppHandle) -> Result<(), TauError> {
    // Refuse while a card write runs, and stop one starting during the install (the guard cannot be held across an await).
    drop(crate::card_write_guard().map_err(|_| err("A card write is running. Wait for it to finish before updating Tau Omega."))?);
    UPDATING.store(true, Ordering::SeqCst);
    let result = install(&app).await;
    UPDATING.store(false, Ordering::SeqCst);
    result
}

async fn install(app: &tauri::AppHandle) -> Result<(), TauError> {
    let channel = channel_of(app);
    let Some(update) = newest(app, &channel).await? else {
        return Err(err("Tau Omega is already up to date."));
    };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| err(format!("The update could not be installed: {e}")))?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::{Equal, Greater, Less};

    #[test]
    fn versions_order_like_semver() {
        assert_eq!(cmp_versions("0.5.0", "0.5.0"), Equal);
        assert_eq!(cmp_versions("0.5.1", "0.5.0"), Greater);
        assert_eq!(cmp_versions("0.10.0", "0.9.9"), Greater, "numbers by value, not text");
        assert_eq!(cmp_versions("0.5.0-alpha.24", "0.5.0"), Less, "a pre-release comes before its release");
        assert_eq!(cmp_versions("0.5.1-alpha.1", "0.5.0"), Greater, "and after the release before it");
        assert_eq!(cmp_versions("0.5.1-alpha.10", "0.5.1-alpha.9"), Greater, "pre-release numbers by value");
        assert_eq!(cmp_versions("0.5.1-alpha.3", "0.5.1-beta.1"), Less, "words in order");
        assert_eq!(cmp_versions("v0.5.1", "0.5.1+build7"), Equal, "a leading v and build metadata do not count");
        assert_eq!(cmp_versions("0.5.1-alpha", "0.5.1-alpha.1"), Less, "a shorter pre-release comes first");
    }

    #[test]
    fn auto_follows_the_build_and_a_chosen_channel_is_kept() {
        assert_eq!(effective_channel("auto", "0.5.0"), "stable");
        assert_eq!(effective_channel("auto", "0.5.1-alpha.2"), "dev");
        assert_eq!(effective_channel("stable", "0.5.1-alpha.2"), "stable");
        assert_eq!(effective_channel("dev", "0.5.0"), "dev");
        assert_eq!(effective_channel("nonsense", "0.5.0"), "stable");
    }

    #[test]
    fn dev_reads_its_own_feed_and_the_stable_one_stable_reads_only_stable() {
        assert_eq!(feeds("stable"), vec![STABLE_FEED]);
        assert_eq!(feeds("dev"), vec![DEV_FEED, STABLE_FEED]);
    }
}
