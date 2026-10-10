//! Drops the operating system's cached copy of a file, so that a read-back right
//! after a write is served by the device and not by memory.
//!
//! Measured on macOS (2026-10-02) against a disk image whose content was changed
//! underneath a mounted volume: a plain read after a write returned the stale
//! bytes; `F_NOCACHE` on the reading side returned them too; invalidating the
//! file's pages (`mmap` + `msync(MS_INVALIDATE)`) made the next read return the
//! changed bytes. A write made through an `F_NOCACHE` descriptor also reached the
//! device, but that only helps files this process wrote itself.
//!
//! This is the host-specific half of verification: `tau-core` forbids `unsafe`
//! and calls the function registered here (`register_cache_evictor`).
//! Windows is not covered (its no-buffering reads need aligned buffers); there
//! `readback_report` says the read-back is not checked against the device.

use std::path::Path;

/// Whether this build can drop cached pages on this platform.
pub const SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "linux"));

/// Drops the cached pages of `path`. `true` if the platform call succeeded.
#[cfg(target_os = "macos")]
pub fn evict(path: &Path) -> bool {
    use std::os::fd::AsRawFd;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(meta) = file.metadata() else {
        return false;
    };
    let Ok(len) = usize::try_from(meta.len()) else {
        return false;
    };
    if len == 0 {
        return true;
    }
    // SAFETY: the mapping is read-only and private to this call, covers exactly the
    // file's current length, and is unmapped before returning; no reference into it
    // escapes.
    unsafe {
        let addr = libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ,
            libc::MAP_SHARED,
            file.as_raw_fd(),
            0,
        );
        if addr == libc::MAP_FAILED {
            return false;
        }
        let ok = libc::msync(addr, len, libc::MS_INVALIDATE) == 0;
        libc::munmap(addr, len);
        ok
    }
}

/// Linux: drop the clean pages of a file that was just written and synced.
#[cfg(target_os = "linux")]
pub fn evict(path: &Path) -> bool {
    use std::os::fd::AsRawFd;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    // SAFETY: plain advisory call on a descriptor this function owns.
    unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) == 0 }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn evict(_path: &Path) -> bool {
    false
}

/// Registers the evictor with the engine where this platform has one.
pub fn register() {
    if SUPPORTED {
        tau_core::sync::register_cache_evictor(evict);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod real_device_checks {
    use super::*;
    use std::{fs, io::Write, path::PathBuf, process::Command};

    fn first_byte(path: &Path) -> u8 {
        let mut buf = [0u8; 1];
        std::io::Read::read_exact(&mut fs::File::open(path).unwrap(), &mut buf).unwrap();
        buf[0]
    }

    /// Mounts a throwaway FAT disk image, writes a file, reads it (so it is
    /// cached), changes the file's bytes **in the image underneath the mount**, and
    /// shows that a plain read is stale while `evict` makes the read see the
    /// device. `#[ignore]`d: it mounts a volume. Run with
    /// `cargo test --features tau-core/serde -- --ignored real_device`.
    #[test]
    #[ignore]
    fn evict_makes_the_next_read_come_from_the_device() {
        let dir = std::env::temp_dir().join(format!("tau-evict-dmg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let image: PathBuf = dir.join("t.dmg");
        let ok = |c: &mut Command| c.output().map(|o| o.status.success()).unwrap_or(false);
        assert!(ok(Command::new("hdiutil")
            .args([
                "create", "-size", "32m", "-fs", "MS-DOS", "-volname", "TAUEVICT", "-type", "UDIF"
            ])
            .arg(&image)));
        let attach = Command::new("hdiutil")
            .args(["attach", "-nobrowse"])
            .arg(&image)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&attach.stdout).into_owned();
        let device = text
            .lines()
            .find(|l| l.contains("/Volumes/TAUEVICT"))
            .and_then(|l| l.split_whitespace().next())
            .unwrap()
            .to_string();
        let result = std::panic::catch_unwind(|| {
            let file = Path::new("/Volumes/TAUEVICT/probe.bin");
            let mut f = fs::File::create(file).unwrap();
            f.write_all(&vec![0x11u8; 4 << 20]).unwrap();
            f.sync_all().unwrap();
            drop(f);
            assert_eq!(first_byte(file), 0x11); // now cached
            let bytes = fs::read(&image).unwrap();
            let offset = bytes
                .windows(65536)
                .position(|w| w.iter().all(|b| *b == 0x11))
                .expect("pattern in image");
            let mut f = fs::OpenOptions::new().write(true).open(&image).unwrap();
            std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(offset as u64)).unwrap();
            f.write_all(&[0x22u8; 4096]).unwrap();
            f.sync_all().unwrap();
            assert_eq!(
                first_byte(file),
                0x11,
                "a plain read is served from the cache (stale)"
            );
            assert!(evict(file));
            assert_eq!(
                first_byte(file),
                0x22,
                "after evict the read reaches the device"
            );
        });
        let _ = Command::new("hdiutil")
            .args(["detach", &device, "-force"])
            .output();
        let _ = fs::remove_dir_all(&dir);
        result.unwrap();
    }
}
