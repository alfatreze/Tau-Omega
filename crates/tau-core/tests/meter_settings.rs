//! `tau_core::meter_settings` against Tau Alpha's golden vectors for the per-meter settings file (`testdata/meter_settings/`, copied read-only
//! from Tau Alpha `docs/research/meter_settings/`: `expected.json` and the 18 files it names). Tau Alpha asked us (A029/R-1) to say where our
//! reader differs from `expected.json`: this test is that check.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tau_core::meter_settings::{self, FILE_SIZE, FileState, Status};

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/meter_settings")
        .join(name)
}
fn bin(name: &str) -> Vec<u8> {
    std::fs::read(fx(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn expected() -> Vec<serde_json::Value> {
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fx("expected.json")).unwrap()).unwrap();
    assert_eq!(v["file_size"].as_u64(), Some(FILE_SIZE as u64));
    v["files"].as_array().unwrap().clone()
}
fn hex(v: &serde_json::Value) -> u32 {
    u32::from_str_radix(v.as_str().unwrap(), 16).unwrap()
}
fn status(s: &str) -> Status {
    match s {
        "ok" => Status::Ok,
        "none" => Status::None,
        "locked" => Status::Locked,
        "size" => Status::Size,
        other => panic!("unknown status {other}"),
    }
}

#[test]
fn every_vector_is_the_file_tau_alpha_published() {
    let all = expected();
    assert_eq!(all.len(), 18);
    for v in &all {
        let name = v["file"].as_str().unwrap();
        let b = bin(name);
        assert_eq!(b.len() as u64, v["bytes"].as_u64().unwrap(), "{name}: size");
        assert_eq!(
            format!("{:x}", Sha256::digest(&b)),
            v["sha256"].as_str().unwrap(),
            "{name}: SHA-256 (the copy is not what Tau Alpha published)"
        );
    }
}

#[test]
fn every_vector_reads_as_tau_alpha_expects_status_bank_seq_records_generations_and_lookups() {
    let mut differ = Vec::new();
    for v in expected() {
        let name = v["file"].as_str().unwrap();
        let p = meter_settings::parse(&bin(name));
        let mut bad = |what: &str, got: String, want: String| {
            if got != want {
                differ.push(format!("{name}: {what}: got {got}, expected {want}"));
            }
        };
        bad(
            "status",
            format!("{:?}", p.status),
            format!("{:?}", status(v["status"].as_str().unwrap())),
        );
        bad(
            "active bank",
            format!("{:?}", p.active),
            format!("{:?}", v["active_bank"].as_u64().map(|b| b as usize)),
        );
        bad(
            "seq",
            format!("{:?}", (p.status == Status::Ok).then_some(p.seq)),
            format!("{:?}", v["seq"].as_u64().map(|s| s as u32)),
        );
        bad(
            "record count",
            p.records.len().to_string(),
            v["count"].as_u64().unwrap().to_string(),
        );
        // a file of the wrong size has no generations in expected.json: "a reader says `size` and nothing else"
        let want_gens: Vec<Option<u32>> = match v["generations"].as_array() {
            Some(gens) => gens
                .iter()
                .map(|g| {
                    g["valid"]
                        .as_bool()
                        .unwrap()
                        .then(|| g["seq"].as_u64().unwrap() as u32)
                })
                .collect(),
            None => vec![None, None],
        };
        bad(
            "generations",
            format!("{:?}", p.generations),
            format!("{want_gens:?}"),
        );
        let want_records = v["records"].as_array().unwrap();
        for (i, (got, want)) in p.records.iter().zip(want_records).enumerate() {
            let entries: Vec<(u8, u16)> = want["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| (e[0].as_u64().unwrap() as u8, e[1].as_u64().unwrap() as u16))
                .collect();
            bad(
                &format!("record {i}"),
                format!(
                    "{:08x} {} {} {:?}",
                    got.id_hash, got.params_rev, got.preset, got.entries
                ),
                format!(
                    "{:08x} {} {} {:?}",
                    hex(&want["id_hash"]),
                    want["params_rev"].as_u64().unwrap(),
                    want["preset"].as_u64().unwrap(),
                    entries
                ),
            );
        }
        for l in v["lookups"].as_array().unwrap() {
            let (h, rev) = (hex(&l["id_hash"]), l["params_rev"].as_u64().unwrap() as u16);
            let found = meter_settings::find(&p, h, rev);
            bad(
                &format!("lookup {h:08x} rev {rev}"),
                found.is_some().to_string(),
                l["found"].as_bool().unwrap().to_string(),
            );
            if let (Some(r), true) = (found, l["found"].as_bool().unwrap()) {
                let entries: Vec<(u8, u16)> = l["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| (e[0].as_u64().unwrap() as u8, e[1].as_u64().unwrap() as u16))
                    .collect();
                bad(
                    &format!("lookup {h:08x} content"),
                    format!("{} {:?}", r.preset, r.entries),
                    format!("{} {entries:?}", l["preset"].as_u64().unwrap()),
                );
            }
        }
    }
    assert!(
        differ.is_empty(),
        "files where this reader differs from expected.json:\n{}",
        differ.join("\n")
    );
}

#[test]
fn the_cases_that_matter_to_a_person_read_as_they_should() {
    let read = |n: &str| meter_settings::parse(&bin(n));
    // the newest valid generation wins, by serial arithmetic across the wrap
    assert_eq!(read("two_generations_b_wins.bin").active, Some(1));
    assert_eq!(
        (
            read("seq_wrap_b_wins.bin").active,
            read("seq_wrap_a_wins.bin").active
        ),
        (Some(1), Some(0))
    );
    assert_eq!(
        (
            read("seq_half_b_wins.bin").active,
            read("seq_half_a_wins.bin").active
        ),
        (Some(1), Some(0))
    );
    // a damaged newest generation falls back to the older one, and two damaged ones to the defaults
    assert_eq!(read("damaged_newest.bin").active, Some(1));
    assert_eq!(read("damaged_both.bin").status, Status::None);
    // a newer version ignores the whole file: no record of the other generation is used
    let newer = read("newer_version.bin");
    assert_eq!(
        (newer.status, newer.active, newer.records.len()),
        (Status::Locked, None, 0)
    );
    // a record saved for another params_rev is not used
    let three = read("three_records.bin");
    assert!(
        meter_settings::find(&three, 0x7471_cc21, 2).is_some()
            && meter_settings::find(&three, 0x7471_cc21, 1).is_none()
    );
    // a file too full for one generation keeps the newest meters (the oldest were evicted by the Pocket)
    assert_eq!(read("full_with_eviction.bin").records.len(), 31);
}

#[test]
fn a_card_with_every_vector_is_reported_by_what_the_pocket_will_do_with_it() {
    let card = std::env::temp_dir().join(format!("tau-mset-vectors-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&card);
    for v in expected() {
        let name = v["file"].as_str().unwrap();
        let dir = card.join("Saves/tau/alfatreze.TAU");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tau-meter-settings.bin"), bin(name)).unwrap();
        let found = meter_settings::scan_card(&card);
        assert_eq!(found.len(), 1, "{name}");
        let want = match v["status"].as_str().unwrap() {
            "ok" => FileState::Saved(v["count"].as_u64().unwrap() as usize),
            "none" => FileState::Defaults,
            "locked" => FileState::Newer,
            "size" => FileState::WrongSize,
            other => panic!("{other}"),
        };
        assert_eq!(found[0].state, want, "{name}");
        assert_eq!(found[0].bytes, v["bytes"].as_u64().unwrap(), "{name}");
    }
    // only a file of the wrong size is a problem: exactly the two the vectors name
    let _ = std::fs::remove_dir_all(&card);
}

/// A small deterministic generator: the same mutations every run.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as usize
    }
}

#[test]
fn no_input_makes_the_reader_panic_and_an_accepted_file_never_holds_more_than_it_can() {
    let mut rng = Lcg(0xC0FFEE);
    for name in [
        "three_records.bin",
        "full_with_eviction.bin",
        "two_generations_b_wins.bin",
        "seq_wrap_b_wins.bin",
    ] {
        let base = bin(name);
        for n in (0..FILE_SIZE + 3).step_by(37) {
            let _ = meter_settings::parse(&base[..n.min(base.len())]);
        }
        for _ in 0..3000 {
            let mut b = base.clone();
            for _ in 0..=rng.next() % 4 {
                let at = rng.next() % b.len();
                b[at] = (rng.next() & 255) as u8;
            }
            let p = meter_settings::parse(&b);
            assert!(
                p.records.len() <= 400 && p.records.iter().all(|r| r.entries.len() <= 64),
                "{name}"
            );
            assert_eq!(
                p.status == Status::Ok,
                p.active.is_some() && !p.records.is_empty()
                    || (p.status == Status::Ok && p.records.is_empty()),
                "{name}"
            );
        }
    }
}
