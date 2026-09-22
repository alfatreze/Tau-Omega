//! Safe, temporary-only card fixtures for T0/T1 tests.

use std::{fs, io, path::Path};

/// Creates a minimal openFPGA card fixture. It never targets a mounted volume.
pub fn fake_card(root: &Path) -> io::Result<()> {
    if root.starts_with("/Volumes") {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "fixtures may not use mounted volumes",
        ));
    }
    for path in [
        "Cores/alfatreze.TAU",
        "Cores/example.legacy",
        "Assets/tau/common",
        "Assets/legacy/common",
        "Platforms/_images",
    ] {
        fs::create_dir_all(root.join(path))?;
    }
    fs::write(
        root.join("Cores/alfatreze.TAU/core.json"),
        r#"{"core":{"metadata":{"author":"alfatreze","shortname":"TAU","version":"0.4.0","platform_ids":["tau"]}}}"#,
    )?;
    // Shaped exactly like a real APF data.json (`data.data_slots`), copied from the
    // slot table the shipped v0.4.0 core declares. An earlier fixture invented a
    // flat `{"data":[...]}` array, which no real core uses, and that hid a bug where
    // library detection never matched a genuine card.
    fs::write(
        root.join("Cores/alfatreze.TAU/data.json"),
        r#"{"data":{"magic":"APF_VER_1","data_slots":[
            {"name":"Firmware","id":1,"required":true,"parameters":"0x5","filename":"tau.rom"},
            {"name":"Music file","id":2,"required":false,"parameters":"0x2"},
            {"name":"Playlist","id":3,"required":false,"parameters":"0x2","filename":"playlist.m3u"},
            {"name":"Loading image","id":4,"required":false,"deferload":true,"parameters":"0x1","filename":"tau-loading.bin"},
            {"name":"Media library index","id":5,"required":false,"deferload":true,"parameters":"0x1","filename":"tau-library.tdb"},
            {"name":"Cold image","id":6,"required":false,"deferload":true,"parameters":"0x1","filename":"tau-cold.bin"}
        ]}}"#,
    )?;
    fs::write(
        root.join("Cores/example.legacy/core.json"),
        r#"{"core":{"metadata":{"author":"example","shortname":"legacy","version":"1.0","platform_ids":["legacy"]}}}"#,
    )?;
    fs::write(
        root.join("Cores/example.legacy/data.json"),
        r#"{"data":{"magic":"APF_VER_1","data_slots":[
            {"name":"Music file","id":2,"required":false,"parameters":"0x2","filename":"track.mp3"}
        ]}}"#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_never_targets_a_volume() {
        assert!(fake_card(Path::new("/Volumes/never-write-here")).is_err());
    }

    #[test]
    fn fixture_has_the_expected_read_only_metadata() {
        let root = std::env::temp_dir().join(format!("tau-omega-testkit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fake_card(&root).unwrap();
        assert!(root.join("Cores/alfatreze.TAU/data.json").is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
