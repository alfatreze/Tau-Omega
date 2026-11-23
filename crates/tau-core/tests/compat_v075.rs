//! Tau Alpha's real v0.7.5 `tau-compat.json` (`testdata/compat/`, SHA-256 in its README) against Omega's compatibility check.
//! Omega 0.6.0 said "v0.7.5 writes persisted settings in layout 2; this Tau Omega reads layout 1. Update Tau Omega first." and could not
//! install the release (found 2026-11-23, Tau Alpha message 033). This pins the fix.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tau_core::compat::{self, Role};

fn bytes() -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/compat/tau-compat_v0.7.5.json"),
    )
    .unwrap()
}

#[test]
fn the_copy_is_the_file_tau_alpha_published() {
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes())),
        "5a8edac594d223b74c16f1fe39ff5be50dcefe87c107c4be7b169a5b29ffbd94"
    );
}

#[test]
fn omega_accepts_v0_7_5_and_says_what_it_needs_to_know_about_it() {
    let doc = compat::parse_compat(&bytes()).unwrap();
    assert_eq!(
        (doc.release.as_str(), doc.min_omega.as_str()),
        ("v0.7.5", "0.6.0")
    );
    assert_eq!(
        (
            doc.persist_layout,
            doc.packs_abi,
            doc.packs_abi_minor,
            doc.meter_settings_version
        ),
        (Some(2), Some(1), Some(2), Some(1))
    );
    for omega in ["0.6.0", "0.6.1", "0.7.0"] {
        assert!(
            compat::requirements_unmet(&doc, omega).is_empty(),
            "Omega {omega}: {:?}",
            compat::requirements_unmet(&doc, omega)
        );
    }
    // An Omega older than the release's own minimum is still told so, in plain words.
    let old = compat::requirements_unmet(&doc, "0.5.2");
    assert_eq!(old.len(), 1, "{old:?}");
    assert!(old[0].contains("needs Tau Omega 0.6.0 or newer"), "{old:?}");
    const { assert!(compat::OMEGA_PERSIST_LAYOUT >= 2) };
}

#[test]
fn the_settings_file_and_the_user_meter_bin_are_listed_as_user_files_with_their_formats() {
    let doc = compat::parse_compat(&bytes()).unwrap();
    for pkg in &doc.packages {
        let settings = pkg
            .layout
            .iter()
            .find(|e| e.path.ends_with("/tau-meter-settings.bin"))
            .unwrap_or_else(|| panic!("{}: no settings file entry", pkg.core_id));
        assert_eq!(
            (settings.role, settings.slot, settings.required),
            (Role::User, Some(11), false),
            "{}",
            pkg.core_id
        );
        assert!(settings.path.starts_with("Saves/tau"), "{}", settings.path);
        let f = settings.format.as_ref().unwrap();
        assert_eq!(
            (f.name.as_str(), f.version, f.bytes),
            ("TMSF", 1, Some(8192))
        );
        let bin = pkg
            .layout
            .iter()
            .find(|e| e.path.ends_with("/common/tau-meters.bin"))
            .unwrap();
        assert_eq!((bin.role, bin.slot), (Role::User, Some(10)));
        let f = bin.format.as_ref().unwrap();
        assert_eq!(
            (f.name.as_str(), f.version, f.max_bytes),
            ("TPKB", 2, Some(4_456_448))
        );
    }
}
