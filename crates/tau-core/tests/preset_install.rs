//! Writing meter preset sets to a (scratch) card: plan, backup outside the card, verified write, rollback, recovery.
//! Built from the real card fixture (Layered Wave, 16 factory presets). Evidence label: host-tested on scratch dirs; no real card.

use std::{collections::BTreeMap, fs, path::PathBuf};
use tau_core::{
    ErrorCode,
    meter_presets::MeterRegistry,
    preset_files::{self, ParamSpec, PresetSet},
    preset_install::{self, FileState, Folder},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/preset_files")
}

fn lw() -> (PresetSet, Vec<ParamSpec>) {
    let reg =
        MeterRegistry::from_json(&fs::read(root().join("meters_schema.json")).unwrap()).unwrap();
    let m = reg.find("layered_wave").unwrap();
    let specs = preset_files::param_specs(m).unwrap();
    let json = fs::read(root().join("meters/layered_wave/presets.json")).unwrap();
    (preset_files::factory_set(m, &json).unwrap(), specs)
}

struct Card {
    dir: PathBuf,
    media: PathBuf,
}
impl Card {
    fn new(tag: &str) -> Card {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "tau-pinst-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        let media = dir.join("Assets/tau/common");
        fs::create_dir_all(&media).unwrap();
        fs::create_dir_all(dir.join("Cores")).unwrap();
        Card { dir, media }
    }
    fn backup(&self) -> PathBuf {
        let b = std::env::temp_dir().join(format!(
            "{}-backup",
            self.dir.file_name().unwrap().to_string_lossy()
        ));
        let _ = fs::remove_dir_all(&b);
        b
    }
}
impl Drop for Card {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn desired(set: &PresetSet, specs: &[ParamSpec]) -> BTreeMap<u16, Folder> {
    BTreeMap::from([(set.meter_id, set.folder(specs).unwrap())])
}

fn install(
    card: &Card,
    d: &BTreeMap<u16, Folder>,
    backup: Option<&std::path::Path>,
) -> preset_install::PresetInstallReport {
    let plan = preset_install::plan(&card.media, d).unwrap();
    preset_install::execute(&card.media, d, &plan, &plan.id, backup).unwrap()
}

#[test]
fn a_first_write_creates_the_folder_and_needs_no_backup() {
    let (set, specs) = lw();
    let card = Card::new("first");
    let d = desired(&set, &specs);
    let plan = preset_install::plan(&card.media, &d).unwrap();
    assert!(plan.writes.iter().all(|w| w.state == FileState::New));
    assert_eq!(plan.writes.len(), 17);
    assert!(plan.backup.is_empty() && plan.removals.is_empty());
    let r = preset_install::execute(&card.media, &d, &plan, &plan.id, None).unwrap();
    assert_eq!((r.written, r.removed), (17, 0));
    assert_eq!(preset_install::read_folder(&card.media, 16), d[&16]);
    // Writing again changes nothing.
    let again = preset_install::plan(&card.media, &d).unwrap();
    assert_eq!(again.unchanged, 17);
    assert!(again.writes.iter().all(|w| w.state == FileState::Unchanged));
}

#[test]
fn an_edit_writes_only_what_changed_backs_up_outside_the_card_and_removes_deleted_presets() {
    let (mut set, specs) = lw();
    let card = Card::new("edit");
    install(&card, &desired(&set, &specs), None);
    let before = preset_install::read_folder(&card.media, 16);
    // Rename AURORA, hide RIDGE, delete the preset with id 3, move id 5 to the front, add a custom preset with a fresh id.
    set.presets[0].name = "AURORA 2".into();
    set.presets.iter_mut().find(|p| p.id == 7).unwrap().hidden = true;
    set.presets.retain(|p| p.id != 3);
    let five = set.presets.iter().position(|p| p.id == 5).unwrap();
    let p = set.presets.remove(five);
    set.presets.insert(0, p);
    set.presets.push(preset_files::SetPreset {
        id: 16,
        name: "MINE".into(),
        hidden: false,
        values: BTreeMap::from([("layers".to_string(), 4)]),
    });
    let d = desired(&set, &specs);
    let plan = preset_install::plan(&card.media, &d).unwrap();
    let changed: Vec<&str> = plan
        .writes
        .iter()
        .filter(|w| w.state == FileState::Changed)
        .map(|w| w.name.as_str())
        .collect();
    assert_eq!(
        changed,
        ["index.tpi", "p0.tpr"],
        "the index (order, hidden, names) and the renamed preset's own file (the name is in both); nothing else"
    );
    assert_eq!(
        plan.writes
            .iter()
            .filter(|w| w.state == FileState::New)
            .count(),
        1
    );
    assert_eq!(plan.removals, [(16, "p3.tpr".to_string())]);
    assert_eq!(plan.unchanged, 14);
    // Refusals: no backup given, a backup inside the card, a wrong token.
    let e = preset_install::execute(&card.media, &d, &plan, &plan.id, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::UnsafeBackupLocation);
    let inside = card.dir.join("backup");
    let e = preset_install::execute(&card.media, &d, &plan, &plan.id, Some(&inside)).unwrap_err();
    assert_eq!(e.code, ErrorCode::UnsafeBackupLocation);
    let e = preset_install::execute(
        &card.media,
        &d,
        &plan,
        "not the token",
        Some(&card.backup()),
    )
    .unwrap_err();
    assert_eq!(e.code, ErrorCode::ConfirmationMismatch);
    assert_eq!(
        preset_install::read_folder(&card.media, 16),
        before,
        "nothing was touched by the refusals"
    );
    let backup = card.backup();
    let r = preset_install::execute(&card.media, &d, &plan, &plan.id, Some(&backup)).unwrap();
    assert_eq!((r.written, r.removed), (3, 1));
    assert_eq!(preset_install::read_folder(&card.media, 16), d[&16]);
    // The backup holds the old index and the deleted preset, byte for byte.
    let base = r.backup.unwrap().join("16");
    assert_eq!(
        fs::read(base.join("index.tpi")).unwrap(),
        before["index.tpi"]
    );
    assert_eq!(fs::read(base.join("p3.tpr")).unwrap(), before["p3.tpr"]);
    let _ = fs::remove_dir_all(&backup);
}

#[test]
fn a_plan_goes_stale_when_the_card_changes_after_review() {
    let (set, specs) = lw();
    let card = Card::new("stale");
    install(&card, &desired(&set, &specs), None);
    let mut edited = set.clone();
    edited.presets[1].name = "CHANGED".into();
    let d = desired(&edited, &specs);
    let plan = preset_install::plan(&card.media, &d).unwrap();
    // Someone changes a file on the card between the review and the confirmation.
    fs::write(card.media.join("tau-presets/16/p2.tpr"), b"someone else").unwrap();
    let e = preset_install::execute(&card.media, &d, &plan, &plan.id, Some(&card.backup()))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::SourceChangedSincePlan);
}

#[test]
fn a_failure_in_the_middle_puts_every_file_back() {
    let (mut set, specs) = lw();
    let card = Card::new("rollback");
    install(&card, &desired(&set, &specs), None);
    let before = preset_install::read_folder(&card.media, 16);
    // Change three presets' values and the index; make the last write impossible (a folder where its temp file goes).
    for id in [0u8, 1, 9] {
        set.presets
            .iter_mut()
            .find(|p| p.id == id)
            .unwrap()
            .values
            .insert("layers".into(), 5);
    }
    set.presets.retain(|p| p.id != 4);
    let d = desired(&set, &specs);
    let plan = preset_install::plan(&card.media, &d).unwrap();
    let blocked = card.media.join("tau-presets/16/p9.tpr.part");
    fs::create_dir_all(&blocked).unwrap();
    let backup = card.backup();
    let e = preset_install::execute(&card.media, &d, &plan, &plan.id, Some(&backup)).unwrap_err();
    assert!(e.message.contains("put back"), "{}", e.message);
    fs::remove_dir_all(&blocked).unwrap();
    assert_eq!(
        preset_install::read_folder(&card.media, 16),
        before,
        "the card is exactly as it was"
    );
    let _ = fs::remove_dir_all(&backup);
}

#[test]
fn an_interrupted_write_is_recovered_before_the_next_one() {
    let (set, specs) = lw();
    let card = Card::new("interrupted");
    let d = desired(&set, &specs);
    install(&card, &d, None);
    let dir = card.media.join("tau-presets/16");
    // The state a pulled card can leave: a preset file renamed to .old and not yet replaced, and a half-written temp file.
    fs::rename(dir.join("p2.tpr"), dir.join("p2.tpr.old")).unwrap();
    fs::write(dir.join("index.tpi.part"), b"half").unwrap();
    // The plan already sees the folder as it will be once recovered, so the reviewed plan stays valid.
    let plan = preset_install::plan(&card.media, &d).unwrap();
    assert!(plan.interrupted);
    assert!(
        plan.writes.iter().all(|w| w.state == FileState::Unchanged),
        "{:?}",
        plan.writes
    );
    preset_install::execute(&card.media, &d, &plan, &plan.id, None).unwrap();
    assert!(
        dir.join("p2.tpr").is_file()
            && !dir.join("p2.tpr.old").exists()
            && !dir.join("index.tpi.part").exists()
    );
    assert_eq!(preset_install::read_folder(&card.media, 16), d[&16]);
}

#[test]
fn other_meters_other_files_and_apple_stubs_are_handled_as_they_should() {
    let (set, specs) = lw();
    let card = Card::new("others");
    let d = desired(&set, &specs);
    // Another meter's folder and a file Omega does not manage are left alone; a stub beside a managed file is swept.
    let other = card.media.join("tau-presets/12");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("index.tpi"), b"not ours to touch").unwrap();
    let mine = card.media.join("tau-presets/16");
    fs::create_dir_all(&mine).unwrap();
    fs::write(mine.join("notes.txt"), b"keep").unwrap();
    fs::write(mine.join("._index.tpi"), b"stub").unwrap();
    install(&card, &d, None);
    assert_eq!(
        fs::read(other.join("index.tpi")).unwrap(),
        b"not ours to touch"
    );
    assert_eq!(fs::read(mine.join("notes.txt")).unwrap(), b"keep");
    assert!(!mine.join("._index.tpi").exists());
}

#[test]
fn a_folder_the_firmware_would_refuse_is_refused_before_anything_is_written() {
    let (set, specs) = lw();
    let card = Card::new("invalid");
    let mut d = desired(&set, &specs);
    let folder = d.get_mut(&16).unwrap();
    folder.remove("p3.tpr");
    assert!(
        preset_install::plan(&card.media, &d)
            .unwrap_err()
            .message
            .contains("p3.tpr")
    );
    let mut d = desired(&set, &specs);
    let copy = d[&16]["p0.tpr"].clone();
    d.get_mut(&16).unwrap().insert("p99.tpr".into(), copy);
    assert!(
        preset_install::plan(&card.media, &d).is_err(),
        "an unlisted file is refused"
    );
    let mut d = desired(&set, &specs);
    d.get_mut(&16).unwrap().remove("index.tpi");
    assert!(
        preset_install::plan(&card.media, &d)
            .unwrap_err()
            .message
            .contains("index")
    );
    assert!(!card.media.join("tau-presets").exists());
    // A media root that is not Assets/<platform>/common is refused.
    let e = preset_install::plan(&card.dir, &desired(&set, &specs)).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidMediaRoot);
}
