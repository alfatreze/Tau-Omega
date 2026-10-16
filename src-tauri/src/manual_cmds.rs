//! Tauri commands for adding a core that is not in the openFPGA Library (`docs/MARKETPLACE_SPEC.md` section 7). Thin: classification, packing,
//! vetting, the preview and the registry are `tau_core`; every request is `tau_net::github_source`. Nothing here writes to a card: the vetted
//! zip goes through the same reviewed install plan as every package (`plan_core_update`).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tau_core::{
    ErrorCode, TauError,
    core_package::Limits,
    core_lint, core_pack, core_preview,
    github_release::{self, GhAsset, GhRelease, Pick},
    manual_registry::{self, ManualEntry},
    manual_source::{self, GitHubSource, Source},
};
use tau_net::{github_source, marketplace::{self, Checksum, Http}};
use tauri::Manager;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn err(code: ErrorCode, message: impl Into<String>) -> TauError {
    TauError { code, message: message.into() }
}

fn cache(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    app.path().app_cache_dir().map(|d| d.join("manual")).map_err(|e| err(ErrorCode::Io, e.to_string()))
}

fn registry_path(app: &tauri::AppHandle) -> Result<PathBuf, TauError> {
    app.path().app_data_dir().map(|d| d.join("manual_cores.json")).map_err(|e| err(ErrorCode::Io, e.to_string()))
}

#[derive(Serialize)]
pub struct Classified {
    /// `zip`, `folder`, `github-repo`, `github-release`, `github-file`.
    pub kind: String,
    pub ok: bool,
    pub reason: String,
}

/// What the text box holds, as you type: a valid source, or the one sentence that says what to change. Reads nothing beyond whether a path exists.
#[tauri::command]
pub fn manual_classify(input: String) -> Classified {
    match manual_source::classify(&input) {
        Ok(Source::Zip(_)) => Classified { kind: "zip".into(), ok: true, reason: String::new() },
        Ok(Source::Folder(_)) => Classified { kind: "folder".into(), ok: true, reason: String::new() },
        Ok(Source::GitHub(g)) => Classified {
            kind: match g {
                GitHubSource::Repo { .. } => "github-repo",
                GitHubSource::Tag { .. } => "github-release",
                GitHubSource::File(_) => "github-file",
            }
            .into(),
            ok: true,
            reason: String::new(),
        },
        Err(e) => Classified { kind: String::new(), ok: false, reason: e.message },
    }
}

#[derive(Deserialize)]
pub struct InspectArgs {
    pub input: String,
    /// A release tag chosen from the list (GitHub repository sources).
    pub tag: Option<String>,
    /// A zip chosen when a release has several.
    pub asset: Option<String>,
    /// Also read the README and its pictures (GitHub sources).
    pub readme: bool,
    /// The open card, only to say whether the core is already installed.
    pub card: Option<String>,
}

#[derive(Serialize)]
pub struct PreviewView {
    #[serde(flatten)]
    pub preview: core_preview::CorePreview,
    pub icon: Option<String>,
    pub banner: Option<String>,
}

#[derive(Serialize)]
pub struct Provenance {
    pub kind: String,
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub asset: String,
    pub bytes: u64,
    pub sha256: String,
    /// `Verified` (GitHub's checksum matched), `NotPublished`, or `Local` (a file or folder on this computer).
    pub checksum: String,
    /// A folder that was packed first (`Package` or `BareCore`), and the wrapper folder it was found inside.
    pub packed: Option<String>,
}

#[derive(Serialize)]
pub struct ReadmeView {
    pub text: String,
    pub images: Vec<String>,
}

#[derive(Serialize)]
pub struct ReleaseChoice {
    pub tag: String,
    pub name: String,
    pub published: String,
    pub prerelease: bool,
    pub zips: Vec<String>,
}

#[derive(Serialize)]
#[serde(tag = "status")]
#[allow(clippy::large_enum_variant)] // built once per inspection and serialised straight away
pub enum ManualInspect {
    /// The core was read; `zip_path` goes to the reviewed install plan.
    #[serde(rename = "ready")]
    Ready {
        preview: Box<PreviewView>,
        zip_path: String,
        provenance: Provenance,
        readme: Option<ReadmeView>,
        /// The core's id is in the openFPGA Library (it gets updates there; offer that instead).
        in_library: bool,
        /// The version already on the open card, when the same id is installed.
        installed_version: Option<String>,
        /// The registry says this id was added from a different source before.
        different_source: bool,
        /// The core checker's findings for this package (errors block the install unless the user overrides).
        lint: core_lint::LintReport,
    },
    /// A release has several zips (or the repository has several releases to pick from): the user chooses.
    #[serde(rename = "choose")]
    Choose { owner: String, repo: String, releases: Vec<ReleaseChoice>, tag: String },
}

fn data_url(png: &[u8], mime: &str) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(png))
}

fn choice(r: &GhRelease) -> ReleaseChoice {
    ReleaseChoice {
        tag: r.tag.clone(),
        name: r.name.clone(),
        published: r.published.clone(),
        prerelease: r.prerelease,
        zips: github_release::zip_assets(r).iter().map(|a| a.name.clone()).collect(),
    }
}

type Downloaded = (String, String, String, tau_net::marketplace::CoreDownloaded, String);

#[allow(clippy::large_enum_variant)]
enum Step {
    Done(Downloaded),
    Choose(ManualInspect),
}

/// The release of a repository the user means (typed tag, chosen tag, else the newest that is not a pre-release) and its zip: downloaded, or a
/// request for the user to choose when several zips (or no zip in this release) leave it open.
fn github_release_zip(
    t: &Http,
    downloads: &Path,
    limits: Limits,
    owner: &str,
    repo: &str,
    typed_tag: Option<String>,
    args: &InspectArgs,
) -> Result<Step, TauError> {
    let releases = match github_source::list_releases(t, owner, repo, now()) {
        Ok(r) => r,
        Err(e) if typed_tag.is_some() => {
            let _ = e;
            Vec::new()
        }
        Err(e) => return Err(e),
    };
    let wanted = args.tag.clone().or(typed_tag);
    let release: GhRelease = match &wanted {
        Some(tag) => match releases.iter().find(|r| &r.tag == tag) {
            Some(r) => r.clone(),
            None => github_source::release_by_tag(t, owner, repo, tag, now())?,
        },
        None => releases
            .iter()
            .find(|r| !r.prerelease)
            .or(releases.first())
            .cloned()
            .ok_or_else(|| err(ErrorCode::NotFound, "This repository has no releases. Ask the author for the zip and add it as a file."))?,
    };
    let choose = |releases: &[GhRelease]| {
        let mut list: Vec<ReleaseChoice> = releases.iter().map(choice).collect();
        if !list.iter().any(|c| c.tag == release.tag) {
            list.insert(0, choice(&release));
        }
        Step::Choose(ManualInspect::Choose { owner: owner.to_string(), repo: repo.to_string(), releases: list, tag: release.tag.clone() })
    };
    let asset: GhAsset = match &args.asset {
        Some(name) => github_release::zip_assets(&release)
            .into_iter()
            .find(|a| &a.name == name)
            .cloned()
            .ok_or_else(|| err(ErrorCode::NotFound, "That zip is not part of the release."))?,
        None => match github_release::pick_asset(&release, repo) {
            Pick::One(a) => a.clone(),
            Pick::Choose(_) => return Ok(choose(&releases)),
            Pick::None => {
                if releases.iter().any(|r| !github_release::zip_assets(r).is_empty()) {
                    return Ok(choose(&releases));
                }
                return Err(err(ErrorCode::NotFound, "No release of this repository has a .zip download. Ask the author for the zip and add it as a file."));
            }
        },
    };
    let d = github_source::download_asset(t, &asset, downloads, limits.max_zip_bytes)?;
    Ok(Step::Done((owner.to_string(), repo.to_string(), release.tag.clone(), d, asset.name.clone())))
}

/// Reads a core from a zip, a folder or a GitHub address into a preview. A GitHub source downloads its zip (size and checksum checked) into the
/// app cache; nothing is written to the card.
#[tauri::command(async)]
pub fn manual_inspect(app: tauri::AppHandle, args: InspectArgs) -> Result<ManualInspect, TauError> {
    let dir = cache(&app)?;
    let limits = Limits::default();
    let t = Http;
    let mut readme_target: Option<(String, String)> = None;
    let (zip, provenance) = match manual_source::classify(&args.input)? {
        Source::Zip(z) => {
            (z.clone(), Provenance { kind: "zip".into(), owner: String::new(), repo: String::new(), tag: String::new(), asset: z.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), bytes: 0, sha256: String::new(), checksum: "Local".into(), packed: None })
        }
        Source::Folder(f) => {
            let out = dir.join("pack");
            let _ = std::fs::remove_dir_all(&out);
            let p = core_pack::pack_folder(&f, &out, limits)?;
            let note = format!("{:?}{}", p.layout, p.descended_into.as_ref().map_or(String::new(), |d| format!(" (inside {d})")));
            (p.zip, Provenance { kind: "folder".into(), owner: String::new(), repo: String::new(), tag: String::new(), asset: String::new(), bytes: p.bytes, sha256: String::new(), checksum: "Local".into(), packed: Some(note) })
        }
        Source::GitHub(g) => {
            let downloads = dir.join("downloads");
            let (owner, repo, tag, dl, asset_name) = match g {
                GitHubSource::File(addr) => {
                    let url = args.input.trim().to_string();
                    let d = github_source::download_zip(&t, &addr, &url, 0, None, &downloads, limits.max_zip_bytes)?;
                    (addr.owner.clone(), addr.repo.clone(), addr.reference.clone(), d, addr.file.clone())
                }
                GitHubSource::Repo { owner, repo } => match github_release_zip(&t, &downloads, limits, &owner, &repo, None, &args)? {
                    Step::Done(done) => done,
                    Step::Choose(c) => return Ok(c),
                },
                GitHubSource::Tag { owner, repo, tag } => match github_release_zip(&t, &downloads, limits, &owner, &repo, Some(tag), &args)? {
                    Step::Done(done) => done,
                    Step::Choose(c) => return Ok(c),
                },
            };
            readme_target = Some((owner.clone(), repo.clone()));
            let path = PathBuf::from(&dl.path);
            (path, Provenance { kind: "github".into(), owner, repo, tag, asset: asset_name, bytes: dl.bytes, sha256: dl.sha256.clone(), checksum: if dl.checksum == Checksum::Verified { "Verified".into() } else { "NotPublished".into() }, packed: None })
        }
    };

    let preview = match core_preview::preview_zip(&zip, limits) {
        Ok(p) => p,
        Err(e) => {
            if provenance.kind == "github" {
                let _ = std::fs::remove_file(&zip);
            }
            return Err(e);
        }
    };
    if preview.core_id.to_ascii_lowercase().starts_with("alfatreze.tau") {
        return Err(err(ErrorCode::InstallRefused, "This is a Tau core. Tau cores are installed and updated from Packages and the Tau update bar, not from here."));
    }
    let mut preview = preview;
    let icon = preview.icon_png.take().map(|b| data_url(&b, "image/png"));
    let banner = preview.banner_png.take().map(|b| data_url(&b, "image/png"));

    let in_library = marketplace::fetch_catalog(&t, &app.path().app_cache_dir().map_err(|e| err(ErrorCode::Io, e.to_string()))?, now(), false)
        .map(|c| c.catalog.find(&preview.core_id).is_some())
        .unwrap_or(false);
    let installed_version = args.card.as_deref().and_then(|c| tau_core::inspect_card(Path::new(c)).ok()).and_then(|card| {
        card.cores.iter().find(|c| c.id.eq_ignore_ascii_case(&preview.core_id)).map(|c| c.version.clone())
    });
    let reg = manual_registry::load(&registry_path(&app)?);
    let different_source = reg.get(&preview.core_id).is_some_and(|e| {
        e.kind != provenance.kind || (provenance.kind == "github" && (!e.owner.eq_ignore_ascii_case(&provenance.owner) || !e.repo.eq_ignore_ascii_case(&provenance.repo)))
    });

    let readme = match (&readme_target, args.readme) {
        (Some((o, r)), true) => github_source::readme(&t, o, r).map(|text| {
            let images = github_release::readme_images(&text, o, r, 3)
                .iter()
                .filter_map(|u| github_source::fetch_image(&t, u).map(|(b, m)| data_url(&b, m)))
                .collect();
            ReadmeView { text, images }
        }),
        _ => None,
    };

    let lint = core_lint::lint_zip(&zip).unwrap_or_default();
    Ok(ManualInspect::Ready {
        zip_path: zip.to_string_lossy().into_owned(),
        preview: Box::new(PreviewView { preview, icon, banner }),
        provenance,
        readme,
        in_library,
        installed_version,
        different_source,
        lint,
    })
}

#[derive(Deserialize)]
pub struct RegisterArgs {
    pub core_id: String,
    pub kind: String,
    pub owner: String,
    pub repo: String,
    pub tag: String,
    pub asset: String,
    pub sha256: String,
    pub card: String,
}

/// Remembers where a manually added core came from (Omega's own data folder; nothing is written to the card).
#[tauri::command]
pub fn manual_register(app: tauri::AppHandle, args: RegisterArgs) -> Result<(), TauError> {
    let path = registry_path(&app)?;
    let mut reg = manual_registry::load(&path);
    reg.upsert(ManualEntry { core_id: args.core_id, kind: args.kind, owner: args.owner, repo: args.repo, tag: args.tag, asset: args.asset, sha256: args.sha256, added: now(), card: args.card });
    manual_registry::save(&path, &reg)
}

/// Every manually added core Omega remembers.
#[tauri::command]
pub fn manual_registry_list(app: tauri::AppHandle) -> Result<Vec<ManualEntry>, TauError> {
    Ok(manual_registry::load(&registry_path(&app)?).entries)
}
