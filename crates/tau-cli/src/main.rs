use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};
use tau_core::sync::{self, CopyState, SyncPlan};
use tau_core::{
    TauError, Warning, build_index, compare, inspect_card, parse, root_prefix, scan_dir, synth,
    verify,
};

/// A CLI-boundary error: either an argument-parsing mistake (no engine code)
/// or an engine `TauError`, which keeps its stable code. `main` uses the
/// distinction to pick the process exit code, so a script driving this CLI
/// can branch on failure without parsing English text.
enum CliError {
    Usage(String),
    Engine(TauError),
}
impl From<TauError> for CliError {
    fn from(error: TauError) -> Self {
        Self::Engine(error)
    }
}
impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::Usage(message)
    }
}
impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::Usage(message.into())
    }
}
impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(message) => f.write_str(message),
            Self::Engine(error) => write!(f, "{error}"),
        }
    }
}

fn usage() {
    eprintln!(
        "Tau Omega CLI\n\nRead-only: cards, scan, verify, report, compare\nCompare: tau compare <Assets/platform/common> --with <Assets/platform/common>\nCore copy: tau core-copy-plan <source-common> --dest <destination-common>\n           tau core-copy <source-common> --dest <destination-common> --confirm <plan-id> --yes --manifest report.json\nCore move: tau core-move <source-common> --dest <destination-common> --confirm <plan-id> --confirm-delete <plan-id> --backup-dir <host-folder> --yes --manifest report.json\nIndex: tau index <common> --out <file> --yes\nSync: tau plan <sources...> --dest <Assets/platform/common> [--mirror] [--embed-cover]\n      tau sync <sources...> --dest <Assets/platform/common> --confirm <plan-id> --yes --manifest report.json [--embed-cover]\n      mirror additionally needs --confirm-delete <plan-id> --backup-dir <host-folder>.\n\nSync never changes a source. --embed-cover adds a reviewed baseline JPEG to MP3/FLAC destination copies only."
    );
}
fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        usage();
        return ExitCode::SUCCESS;
    }
    let command = args.remove(0);
    let json = flag(&mut args, "--json");
    let result = match command.as_str() {
        "cards" => cards(&args, json),
        "scan" => scan(&args, json),
        "index" => index(&args, json),
        "verify" => check(&args, json),
        "report" => report(&args, json),
        "compare" => compare_media(&args, json),
        "core-copy-plan" => core_copy_plan(&args, json),
        "core-copy" => core_copy_execute(&args, json),
        "core-move" => core_move_execute(&args, json),
        "synth" => synthetic(&args, json),
        "plan" => sync_plan(&args, json),
        "sync" => sync_execute(&args, json),
        _ => Err(CliError::Usage(format!("unknown command: {command}"))),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
        Err(CliError::Engine(error)) => {
            eprintln!("error {}: {}", error.code(), error.message);
            ExitCode::from(u8::try_from(error.code().as_u16()).unwrap_or(255))
        }
    }
}
fn compare_media(a: &[String], j: bool) -> Result<(), CliError> {
    let left = Path::new(a.first().ok_or("compare needs a left media root")?);
    let right = Path::new(value(a, "--with").ok_or("compare needs --with MEDIA_ROOT")?);
    let comparison = compare::media_roots(left, right)?;
    let count = |state| comparison.count(state);
    if j {
        println!(
            r#"{{"left":{},"right":{},"only_left":{},"only_right":{},"different":{},"identical":{}}}"#,
            q(&comparison.left.to_string_lossy()),
            q(&comparison.right.to_string_lossy()),
            count(compare::DifferenceState::OnlyLeft),
            count(compare::DifferenceState::OnlyRight),
            count(compare::DifferenceState::Different),
            count(compare::DifferenceState::Identical),
        );
    } else {
        println!(
            "{} only on left · {} only on right · {} different · {} identical",
            count(compare::DifferenceState::OnlyLeft),
            count(compare::DifferenceState::OnlyRight),
            count(compare::DifferenceState::Different),
            count(compare::DifferenceState::Identical),
        );
    }
    Ok(())
}
fn core_copy_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let plan = make_core_copy_plan(a)?;
    show_plan(&plan, j);
    Ok(())
}
fn core_copy_execute(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let plan = make_core_copy_plan(a)?;
    let confirmation = value(a, "--confirm").ok_or("core-copy needs --confirm PLAN-ID")?;
    let journal =
        PathBuf::from(value(a, "--manifest").ok_or("core-copy needs --manifest HOST_REPORT.json")?);
    let report = tau_core::journal::execute_to_journal(&plan, confirmation, &journal, &mut None)?;
    if j {
        println!(
            r#"{{"plan_id":{},"copied":{},"unchanged":{},"index":{}}}"#,
            q(&report.plan_id),
            report.copied,
            report.unchanged,
            q(&report.index_path.to_string_lossy()),
        );
    } else {
        println!(
            "Core copy verified: {} copied · {} unchanged\nIndex: {}",
            report.copied,
            report.unchanged,
            report.index_path.display()
        );
    }
    Ok(())
}
fn core_move_execute(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let plan = make_core_copy_plan(a)?;
    let source = PathBuf::from(a.first().ok_or("core-move needs a source media root")?);
    let confirmation = value(a, "--confirm").ok_or("core-move needs --confirm PLAN-ID")?;
    let delete_confirmation =
        value(a, "--confirm-delete").ok_or("core-move needs --confirm-delete PLAN-ID")?;
    let backup =
        PathBuf::from(value(a, "--backup-dir").ok_or("core-move needs --backup-dir HOST_FOLDER")?);
    let journal =
        PathBuf::from(value(a, "--manifest").ok_or("core-move needs --manifest HOST_REPORT.json")?);
    let report = tau_core::journal::execute_core_move_to_journal(
        &plan,
        confirmation,
        delete_confirmation,
        &source,
        &root_prefix(&source)?,
        &backup,
        &journal,
        &mut None,
    )?;
    if j {
        println!(
            r#"{{"plan_id":{},"copied":{},"deleted":{},"index":{}}}"#,
            q(&report.plan_id),
            report.copied,
            report.deleted,
            q(&report.index_path.to_string_lossy())
        );
    } else {
        println!(
            "Core move verified: {} copied · {} source files backed up and removed\nDestination index: {}",
            report.copied,
            report.deleted,
            report.index_path.display()
        );
    }
    Ok(())
}
fn cards(a: &[String], j: bool) -> Result<(), CliError> {
    let card = inspect_card(a.first().ok_or("cards needs a folder")?)?;
    if j {
        println!(
            r#"{{"root":{},"pocket_card":{},"cores":[{}]}}"#,
            q(&card.root.to_string_lossy()),
            card.is_pocket_card,
            card.cores
                .iter()
                .map(|c| format!(
                    r#"{{"id":{},"platform":{},"library_capable":{}}}"#,
                    q(&c.id),
                    q(&c.platform),
                    c.library_capable
                ))
                .collect::<Vec<_>>()
                .join(",")
        );
    } else {
        println!("{}\n{} core(s)", card.root.display(), card.cores.len());
        for c in card.cores {
            println!(
                "{} · {} · {}",
                c.id,
                c.platform,
                if c.library_capable {
                    "library"
                } else {
                    "legacy"
                }
            );
        }
    }
    Ok(())
}
fn scan(a: &[String], j: bool) -> Result<(), CliError> {
    let scan = scan_dir(
        Path::new(a.first().ok_or("scan needs a media root")?),
        a.iter().any(|x| x == "--playlists"),
    )?;
    if j {
        println!(
            r#"{{"tracks":{},"playlists":{},"warnings":{}}}"#,
            scan.entries.len(),
            scan.playlists.len(),
            warnings_array(&scan.warnings)
        );
    } else {
        println!(
            "{} tracks · {} playlists",
            scan.entries.len(),
            scan.playlists.len()
        );
        for w in &scan.warnings {
            eprintln!("warning: {w}");
        }
    }
    Ok(())
}
fn index(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let common = PathBuf::from(a.first().ok_or("index needs a media root")?);
    let out = PathBuf::from(value(a, "--out").ok_or("index needs --out FILE")?);
    let scan = scan_dir(&common, a.iter().any(|x| x == "--playlists"))?;
    let mut warns = scan.warnings;
    let data = build_index(
        &scan.entries,
        &scan.playlists,
        &root_prefix(&common)?,
        &mut warns,
    )?;
    parse(&data)?;
    fs::write(&out, &data).map_err(TauError::from)?;
    if j {
        println!(
            r#"{{"written":{},"bytes":{},"warnings":{}}}"#,
            q(&out.to_string_lossy()),
            data.len(),
            warnings_array(&warns)
        );
    } else {
        println!(
            "Wrote verified index: {} ({} bytes)",
            out.display(),
            data.len()
        );
    }
    Ok(())
}
fn check(a: &[String], j: bool) -> Result<(), CliError> {
    let data = fs::read(a.first().ok_or("verify needs an index file")?).map_err(TauError::from)?;
    let root = value(a, "--root").map(PathBuf::from);
    let issues = verify(&data, root.as_deref())?;
    if j {
        println!(
            r#"{{"ok":{},"issues":{}}}"#,
            issues.is_empty(),
            array(&issues)
        );
    } else if issues.is_empty() {
        println!("Index is valid.");
    } else {
        for issue in &issues {
            println!("warning: {issue}");
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(CliError::Usage(format!(
            "{} verification issue(s)",
            issues.len()
        )))
    }
}
fn report(a: &[String], j: bool) -> Result<(), CliError> {
    let index =
        parse(fs::read(a.first().ok_or("report needs an index file")?).map_err(TauError::from)?)?;
    if j {
        println!(
            r#"{{"artists":{},"albums":{},"tracks":{},"playlists":{},"build_id":"{:08X}"}}"#,
            index.counts.artists,
            index.counts.albums,
            index.counts.tracks,
            index.counts.playlists,
            index.build_id
        );
    } else {
        println!(
            "{} artists · {} albums · {} tracks · {} playlists\nbuild {:08X}",
            index.counts.artists,
            index.counts.albums,
            index.counts.tracks,
            index.counts.playlists,
            index.build_id
        );
    }
    Ok(())
}
fn synthetic(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let tracks = num(a, "--tracks")?;
    let albums = num(a, "--albums")?;
    let artists = num(a, "--artists")?;
    let out = PathBuf::from(value(a, "--out").ok_or("synth needs --out FILE")?);
    let mut warns = Vec::new();
    let data = build_index(
        &synth(tracks, albums, artists),
        &[],
        tau_core::ROOT_PREFIX,
        &mut warns,
    )?;
    parse(&data)?;
    fs::write(&out, &data).map_err(TauError::from)?;
    if j {
        println!(r#"{{"tracks":{tracks},"bytes":{}}}"#, data.len());
    } else {
        println!(
            "Wrote verified synthetic index: {} ({} bytes)",
            out.display(),
            data.len()
        );
    }
    Ok(())
}
fn sync_plan(a: &[String], j: bool) -> Result<(), CliError> {
    let plan = make_plan(a)?;
    show_plan(&plan, j);
    Ok(())
}
fn sync_execute(a: &[String], j: bool) -> Result<(), CliError> {
    yes(a)?;
    let plan = make_plan(a)?;
    let confirmation = value(a, "--confirm").ok_or("sync needs --confirm PLAN-ID")?;
    let path =
        PathBuf::from(value(a, "--manifest").ok_or("sync needs --manifest HOST_REPORT.json")?);
    let report = if plan.deletions.is_empty() {
        tau_core::journal::execute_to_journal(&plan, confirmation, &path, &mut None)
    } else {
        tau_core::journal::execute_mirror_to_journal(
            &plan,
            confirmation,
            value(a, "--confirm-delete"),
            value(a, "--backup-dir").map(Path::new),
            &path,
            &mut None,
        )
    }?;
    if j {
        println!(
            r#"{{"plan_id":{},"copied":{},"unchanged":{},"bytes_written":{},"index":{},"warnings":{}}}"#,
            q(&report.plan_id),
            report.copied,
            report.unchanged,
            report.bytes_written,
            q(&report.index_path.to_string_lossy()),
            warnings_array(&report.warnings)
        );
    } else {
        println!(
            "Sync verified: {} copied · {} unchanged · {} bytes\nIndex: {}",
            report.copied,
            report.unchanged,
            report.bytes_written,
            report.index_path.display()
        );
    }
    Ok(())
}
fn make_plan(a: &[String]) -> Result<SyncPlan, CliError> {
    let dest =
        PathBuf::from(value(a, "--dest").ok_or("sync needs --dest Assets/<platform>/common")?);
    Ok(sync::plan(
        &sources(a),
        &dest,
        &root_prefix(&dest)?,
        sync::PlanOptions {
            mirror: a.iter().any(|arg| arg == "--mirror"),
            embed_covers: a.iter().any(|arg| arg == "--embed-cover"),
        },
        &mut None,
    )?)
}
fn make_core_copy_plan(a: &[String]) -> Result<SyncPlan, CliError> {
    let source = PathBuf::from(a.first().ok_or("core-copy needs a source media root")?);
    let destination =
        PathBuf::from(value(a, "--dest").ok_or("core-copy needs --dest Assets/<platform>/common")?);
    Ok(sync::plan_core_copy(
        &source,
        &destination,
        &root_prefix(&destination)?,
        &mut None,
    )?)
}
fn show_plan(p: &SyncPlan, j: bool) {
    let count = |state| p.items.iter().filter(|item| item.state == state).count();
    if j {
        println!(
            r#"{{"plan_id":{},"destination":{},"new":{},"update":{},"same":{},"delete":{},"bytes_to_write":{},"warnings":{}}}"#,
            q(&p.id),
            q(&p.destination.to_string_lossy()),
            count(CopyState::New),
            count(CopyState::Update),
            count(CopyState::Same),
            p.deletions.len(),
            p.bytes_to_write,
            warnings_array(&p.warnings)
        );
    } else {
        println!(
            "Plan {}\nDestination: {}\n{} new · {} update · {} unchanged · {} delete · {} bytes to write\nTo execute: tau sync ... --confirm {} --yes",
            p.id,
            p.destination.display(),
            count(CopyState::New),
            count(CopyState::Update),
            count(CopyState::Same),
            p.deletions.len(),
            p.bytes_to_write,
            p.id
        );
    }
}
fn value<'a>(a: &'a [String], key: &str) -> Option<&'a str> {
    a.iter()
        .position(|x| x == key)
        .and_then(|i| a.get(i + 1))
        .map(String::as_str)
}
fn sources(a: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut skip = false;
    for arg in a {
        if skip {
            skip = false;
            continue;
        }
        if matches!(
            arg.as_str(),
            "--dest" | "--confirm" | "--confirm-delete" | "--manifest" | "--backup-dir"
        ) {
            skip = true;
            continue;
        }
        if arg != "--yes" && arg != "--mirror" && arg != "--embed-cover" {
            out.push(PathBuf::from(arg));
        }
    }
    out
}
fn num(a: &[String], key: &str) -> Result<usize, CliError> {
    value(a, key)
        .ok_or_else(|| format!("synth needs {key}"))?
        .parse()
        .map_err(|_| CliError::Usage(format!("invalid {key}")))
}
fn yes(a: &[String]) -> Result<(), CliError> {
    if a.iter().any(|x| x == "--yes") {
        Ok(())
    } else {
        Err("refusing to write without --yes; review the plan first".into())
    }
}
fn flag(a: &mut Vec<String>, f: &str) -> bool {
    if let Some(i) = a.iter().position(|x| x == f) {
        a.remove(i);
        true
    } else {
        false
    }
}
fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
fn array(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| q(s)).collect::<Vec<_>>().join(","))
}
/// Renders structured warnings as `{"code": "...", "message": "..."}` objects
/// in `--json` mode, so a script can branch on `code` without parsing English.
fn warnings_array(warnings: &[Warning]) -> String {
    format!(
        "[{}]",
        warnings
            .iter()
            .map(|w| format!(
                r#"{{"code":{},"message":{}}}"#,
                q(w.code.as_str()),
                q(&w.message)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
