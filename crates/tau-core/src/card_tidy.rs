//! Housekeeping done just before a Pocket card is ejected: removes the macOS leftovers that Finder and the OS drop on an
//! exFAT/FAT card (`._*` AppleDouble stubs and `.DS_Store`), anywhere on the card. SAFETY_RULES 7.
//!
//! Only a Pocket card (a root holding `Assets` and `Cores`) is touched, never an arbitrary volume. Regular files only:
//! symlinks and directories are left alone, a `._*` file larger than a stub is kept, and the OS's own folders
//! (`.Spotlight-V100`, `.fseventsd`, `.Trashes`, `System Volume Information`) are not entered. Errors are ignored: this
//! is housekeeping and must never stop an eject.

use crate::marker::find_card_root;
use std::{fs, path::Path};

/// A `._*` file above this size is not an AppleDouble stub and is kept.
const APPLEDOUBLE_MAX: u64 = 64 * 1024;

const SKIP_DIRS: [&str; 4] = [
    ".Spotlight-V100",
    ".fseventsd",
    ".Trashes",
    "System Volume Information",
];

/// What was removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tidied {
    pub appledouble: usize,
    pub ds_store: usize,
}

impl Tidied {
    pub fn total(&self) -> usize {
        self.appledouble + self.ds_store
    }
}

/// Sweeps the Pocket card holding `path`. Returns zero counts when `path` is on no Pocket card.
pub fn tidy_card(path: &Path) -> Tidied {
    let mut out = Tidied::default();
    let Some(root) = find_card_root(path) else {
        return out;
    };
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if kind.is_dir() {
                if !SKIP_DIRS.contains(&name.as_ref()) {
                    stack.push(entry.path());
                }
            } else if kind.is_file() {
                if name == ".DS_Store" {
                    if fs::remove_file(entry.path()).is_ok() {
                        out.ds_store += 1;
                    }
                } else if name.starts_with("._")
                    && entry.metadata().is_ok_and(|m| m.len() <= APPLEDOUBLE_MAX)
                    && fs::remove_file(entry.path()).is_ok()
                {
                    out.appledouble += 1;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn card() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "tau-tidy-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Assets/tau/common")).unwrap();
        fs::create_dir_all(dir.join("Cores")).unwrap();
        dir
    }

    fn put(p: &Path, bytes: &[u8]) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }

    #[test]
    fn removes_stubs_and_ds_store_anywhere_on_the_card_and_nothing_else() {
        let c = card();
        put(&c.join("._Cores"), b"x");
        put(&c.join(".DS_Store"), b"x");
        put(&c.join("Assets/tau/common/._song.mp3"), b"x");
        put(&c.join("Assets/tau/common/.DS_Store"), b"x");
        put(&c.join("Assets/tau/common/song.mp3"), b"music");
        put(&c.join("Cores/a.B/core.json"), b"{}");
        put(&c.join("Cores/a.B/._core.json"), b"x");
        // Kept: a large ._ file, the OS folders (even with a stub inside), and an ordinary dotfile.
        put(
            &c.join("Assets/tau/common/._big"),
            &vec![0u8; (APPLEDOUBLE_MAX + 1) as usize],
        );
        put(&c.join(".Spotlight-V100/._inner"), b"x");
        put(&c.join(".fseventsd/._inner"), b"x");
        put(&c.join(".metadata_never_index"), b"");
        let t = tidy_card(&c.join("Assets/tau/common"));
        assert_eq!(
            t,
            Tidied {
                appledouble: 3,
                ds_store: 2
            }
        );
        assert_eq!(t.total(), 5);
        assert!(c.join("Assets/tau/common/song.mp3").is_file());
        assert!(c.join("Cores/a.B/core.json").is_file());
        assert!(c.join("Assets/tau/common/._big").is_file());
        assert!(c.join(".Spotlight-V100/._inner").is_file());
        assert!(c.join(".fseventsd/._inner").is_file());
        assert!(c.join(".metadata_never_index").is_file());
        let _ = fs::remove_dir_all(&c);
    }

    #[test]
    fn a_folder_that_is_not_a_pocket_card_is_never_touched() {
        let dir = std::env::temp_dir().join(format!("tau-tidy-plain-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        put(&dir.join("._x"), b"x");
        put(&dir.join(".DS_Store"), b"x");
        assert_eq!(tidy_card(&dir), Tidied::default());
        assert!(dir.join("._x").is_file() && dir.join(".DS_Store").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed_or_removed() {
        let c = card();
        let outside = std::env::temp_dir().join(format!("tau-tidy-out-{}", std::process::id()));
        let _ = fs::remove_dir_all(&outside);
        put(&outside.join("._keep"), b"x");
        std::os::unix::fs::symlink(&outside, c.join("Assets/link")).unwrap();
        std::os::unix::fs::symlink(outside.join("._keep"), c.join("._link")).unwrap();
        assert_eq!(tidy_card(&c), Tidied::default());
        assert!(outside.join("._keep").is_file());
        let _ = fs::remove_dir_all(&c);
        let _ = fs::remove_dir_all(&outside);
    }
}
