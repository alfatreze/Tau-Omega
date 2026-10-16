//! A real Tau Alpha release zip (read from the sibling repository, read-only) passes the marketplace vetting; skipped when absent.
use std::path::Path;
use tau_core::core_package::{Limits, vet_zip};

#[test]
fn real_tau_alpha_release_zips_pass_the_marketplace_vetting() {
    let dir = Path::new("../../../tau-alpha/release");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut checked = 0;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".zip") || !name.starts_with("alfatreze.TAU_Preview_") {
            continue;
        }
        let stem = name.split("_0.").next().unwrap().replace('_', " ");
        vet_zip(&e.path(), &stem, Limits::default())
            .unwrap_or_else(|err| panic!("{name}: {}", err.message));
        checked += 1;
    }
    eprintln!("vetted {checked} real Tau zips");
}
