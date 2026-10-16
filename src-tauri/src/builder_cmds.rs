//! Tauri commands for the Builder page (`docs/DEV_LOOP_SPEC.md`, section 4): a raw bitstream plus a template core becomes an installable package zip in
//! the developer's build folder. Reads the inputs and the card (for an installed template); writes only the new zip and its build record.

use super::{read_prefs};
use serde::Serialize;
use std::path::Path;
use tau_core::{
    ErrorCode, TauError,
    build_series::{self, Field},
    builder::{self, BuildPlan, BuildResult, BuildSpec, TemplateSource},
    core_pack,
    core_package::Limits,
    core_preview,
};

fn err(m: impl Into<String>) -> TauError {
    TauError { code: ErrorCode::InvalidPathReference, message: m.into() }
}

#[derive(Serialize)]
pub struct TemplateInfo {
    core_id: String,
    shortname: String,
    author: String,
    version: String,
    description: String,
    platforms: Vec<String>,
}

/// What the template says about itself, to pre-fill the form (shortname, version, description).
#[tauri::command(async)]
pub fn builder_template_info(app: tauri::AppHandle, source: TemplateSource) -> Result<TemplateInfo, TauError> {
    match source {
        TemplateSource::Card { card, id } => {
            let d = tau_core::corehub::core_detail(Path::new(&card), &id)?;
            Ok(TemplateInfo { core_id: d.id, shortname: d.shortname, author: d.author, version: d.version, description: d.description, platforms: d.platforms.into_iter().map(|p| p.id).collect() })
        }
        TemplateSource::Zip { path } => info_of_zip(Path::new(&path)),
        TemplateSource::Folder { path } => {
            let tmp = tauri::Manager::path(&app).app_cache_dir().map_err(|e| TauError { code: ErrorCode::Io, message: e.to_string() })?.join("builder");
            std::fs::create_dir_all(&tmp)?;
            let packed = core_pack::pack_folder(Path::new(&path), &tmp, Limits::default())?;
            let r = info_of_zip(&packed.zip);
            let _ = std::fs::remove_file(&packed.zip);
            r
        }
    }
}

fn info_of_zip(zip: &Path) -> Result<TemplateInfo, TauError> {
    let p = core_preview::preview_zip(zip, Limits::default())?;
    Ok(TemplateInfo { core_id: p.core_id, shortname: p.shortname, author: p.author, version: p.version, description: p.description, platforms: p.platforms.into_iter().map(|x| x.id).collect() })
}

fn known(app: &tauri::AppHandle) -> Result<Vec<String>, TauError> {
    Ok(read_prefs(app)?.builds_dir.map(|d| builder::known_converted(Path::new(&d))).unwrap_or_default())
}

/// What would be built and what the core checker says. Writes nothing.
#[tauri::command(async)]
pub fn builder_plan(app: tauri::AppHandle, spec: BuildSpec) -> Result<BuildPlan, TauError> {
    builder::plan(&spec, &known(&app)?)
}

/// Builds the package into `out_dir` (default: the saved build folder).
#[tauri::command(async)]
pub fn builder_build(app: tauri::AppHandle, spec: BuildSpec, out_dir: Option<String>) -> Result<BuildResult, TauError> {
    let dir = out_dir.filter(|d| !d.trim().is_empty()).or(read_prefs(&app)?.builds_dir).ok_or_else(|| err("Choose the folder to build into (the build folder on the Builds page is the usual one)."))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    builder::build(&spec, &known(&app)?, Path::new(&dir), &format!("Tau Omega {}", env!("CARGO_PKG_VERSION")), now)
}

#[derive(Serialize)]
pub struct NextName {
    scheme: String,
    next: String,
}

/// The next name after `text` by the developer's schemes (then the built-in ones): `TAU DEV 112` gives `TAU DEV 113`.
#[tauri::command(async)]
pub fn builder_next_name(app: tauri::AppHandle, field: Field, text: String) -> Result<Option<NextName>, TauError> {
    let schemes = build_series::with_custom(read_prefs(&app)?.build_schemes);
    Ok(build_series::next_name(&schemes, field, &text).map(|(scheme, next)| NextName { scheme, next }))
}
