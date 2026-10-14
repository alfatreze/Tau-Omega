//! Split out of the parent module unchanged (see the parent's docs).

use super::*;

/// Puts the card back as it was before the install that made `backup_dir`: every backed-up file is restored (its
/// backup copy is hash-checked first and the restored file read back), then the files the install created are
/// removed. Safe to run twice.
pub fn rollback(card_root: &Path, backup_dir: &Path) -> Result<RollbackReport, TauError> {
    let journal = read_journal(backup_dir)?;
    if journal.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(refuse(
            "this install journal is from a version Omega does not know",
        ));
    }
    if journal.get("card").and_then(Value::as_str) != Some(&card_root.to_string_lossy()) {
        return Err(refuse("this backup was made for a different card folder"));
    }
    let mut restored = 0;
    for entry in journal
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(path), Some(hash)) = (
            entry.get("path").and_then(Value::as_str),
            entry.get("sha256").and_then(Value::as_str),
        ) else {
            return Err(refuse("the install journal has a malformed entry"));
        };
        if path.split('/').any(|c| c == ".." || c.is_empty()) || path.starts_with('/') {
            return Err(refuse(format!(
                "the install journal names an unsafe path ({path})"
            )));
        }
        let saved = fs::read(backup_dir.join("files").join(path))
            .map_err(|_| refuse(format!("the backup of {path} is missing")))?;
        if sync::sha256_bytes(&saved) != hash {
            return Err(refuse(format!(
                "the backup of {path} no longer matches its recorded hash; it was not restored"
            )));
        }
        let dest = card_root.join(path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        // The OS creates a `._` companion for what it writes; one that was not there before this restore is ours to remove.
        let companion = stub_companions(path).pop();
        let had_companion = companion
            .as_ref()
            .is_some_and(|c| card_root.join(c).exists());
        sync::write_durable(&dest, &saved)?;
        if sync::sha256_bytes(&fs::read(&dest)?) != hash {
            return Err(TauError::e(
                ErrorCode::VerificationFailed,
                format!("{path} did not read back the same after it was restored"),
            ));
        }
        if let (Some(companion), false) = (companion, had_companion) {
            remove_file(card_root, &companion)?;
        }
        restored += 1;
    }
    let mut created_removed = 0;
    let created: Vec<String> = journal
        .get("created")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    for path in &created {
        if path.split('/').any(|c| c == ".." || c.is_empty()) || path.starts_with('/') {
            return Err(refuse(format!(
                "the install journal names an unsafe path ({path})"
            )));
        }
        let file = card_root.join(path);
        if file.is_file() {
            fs::remove_file(&file)?;
            created_removed += 1;
        }
        // The OS's `._` companion of the file goes with it.
        if let Some(stub) = stub_companions(path).pop() {
            remove_file(card_root, &stub)?;
        }
    }
    // Folders the install created, deepest first, once only OS companions are left in them. (A journal written before
    // this was recorded falls back to every folder above a created file.)
    let mut dirs: Vec<String> = match journal.get("created_dirs").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        None => created.iter().flat_map(|p| ancestors(p)).collect(),
    };
    dirs.sort();
    dirs.dedup();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for dir in &dirs {
        if dir.split('/').any(|c| c == ".." || c.is_empty()) || dir.starts_with('/') {
            return Err(refuse(format!(
                "the install journal names an unsafe folder ({dir})"
            )));
        }
        let path = card_root.join(dir);
        let Ok(read) = fs::read_dir(&path) else {
            continue;
        };
        let entries: Vec<_> = read.flatten().collect();
        if entries
            .iter()
            .all(|e| e.file_name().to_string_lossy().starts_with("._"))
        {
            for e in &entries {
                if e.path().is_file() {
                    fs::remove_file(e.path())?;
                }
            }
            if fs::remove_dir(&path).is_ok()
                && let Some(stub) = stub_companions(dir).pop()
            {
                remove_file(card_root, &stub)?;
            }
        }
    }
    let mut journal = journal;
    journal["state"] = json!("rolled_back");
    write_journal(backup_dir, &journal)?;
    Ok(RollbackReport {
        restored,
        created_removed,
    })
}
