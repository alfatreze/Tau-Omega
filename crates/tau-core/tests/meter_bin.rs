//! `tau_core::meter_bin` against Tau Alpha's golden vectors for the meter bin v2 (`testdata/meter_bin/`, copied read-only from Tau Alpha
//! `docs/research/meter_bin/`, `expected.json` and the files it names) and the output of their reference reader (`tools/meter_bin.py info`,
//! kept in `*.reference.json`). Tau Alpha asked us (A027/R-1) to say where our reader differs from `expected.json`: this test is that check.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tau_core::meter_bin::{self, Availability, Refusal};

const FW_ABI: u16 = 0x0102; // expected.json: "firmware at ABI 1.2 (0x0102) and TAU 0.8 (0x0008)"
const FW_TAU: u16 = 0x0008;

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/meter_bin")
        .join(name)
}
fn bin(name: &str) -> Vec<u8> {
    std::fs::read(fx(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn json(name: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(fx(name)).unwrap()).unwrap()
}
fn expected() -> Vec<serde_json::Value> {
    json("expected.json")["vectors"].as_array().unwrap().clone()
}
fn code_of<T>(r: &Result<T, meter_bin::BinError>) -> u64 {
    r.as_ref().err().map_or(0, |e| u64::from(e.refusal.code()))
}
fn row_of<T>(r: &Result<T, meter_bin::BinError>) -> Option<u64> {
    r.as_ref().err().and_then(|e| e.row).map(|r| r as u64)
}

#[test]
fn every_vector_is_the_file_tau_alpha_published() {
    let all = expected();
    assert_eq!(all.len(), 32);
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
fn the_full_check_gives_every_vector_the_code_and_row_tau_alpha_expects() {
    let mut differ = Vec::new();
    for v in expected() {
        let name = v["file"].as_str().unwrap();
        let b = bin(name);
        let got = meter_bin::parse(&b);
        let (want_code, want_row) = (v["code"].as_u64().unwrap(), v["bad_row"].as_u64());
        if code_of(&got) != want_code || row_of(&got) != want_row {
            differ.push(format!(
                "{name}: got {} row {:?}, expected {want_code} row {want_row:?}",
                code_of(&got),
                row_of(&got)
            ));
        }
        if want_code == 0 {
            assert_eq!(
                got.unwrap().rows.len() as u64,
                v["count"].as_u64().unwrap(),
                "{name}: row count"
            );
        }
    }
    assert!(
        differ.is_empty(),
        "vectors where this reader differs from expected.json:\n{}",
        differ.join("\n")
    );
}

#[test]
fn the_boot_check_gives_the_code_the_pocket_gives_and_needs_no_byte_past_the_previews() {
    for v in expected() {
        let Some(want) = v["boot_code"].as_u64() else {
            continue;
        }; // v1 files: the firmware does not read them
        let name = v["file"].as_str().unwrap();
        let b = bin(name);
        assert_eq!(
            code_of(&meter_bin::open_toc(&b, b.len())),
            want,
            "{name}: boot code"
        );
        if want == 0 {
            // the Pocket reads only bytes 0..packs_off at boot: the same answer from just those bytes
            let packs_off = u32::from_le_bytes(b[24..28].try_into().unwrap()) as usize;
            let part = meter_bin::open_toc(&b[..packs_off], b.len())
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(part, meter_bin::open_toc(&b, b.len()).unwrap(), "{name}");
        }
    }
}

#[test]
fn the_rows_match_tau_alphas_reference_reader_field_for_field() {
    for name in [
        "v2_seven",
        "v2_reordered_hidden",
        "v2_unavailable_rows",
        "v2_empty",
        "v2_tuning_collision",
        "v1_seven",
    ] {
        let want = json(&format!("{name}.reference.json"));
        let b = bin(&format!("{name}.bin"));
        let got = meter_bin::parse(&b).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            u64::from(got.version),
            want["version"].as_u64().unwrap(),
            "{name}: version"
        );
        assert_eq!(
            got.bytes as u64,
            want["bytes"].as_u64().unwrap(),
            "{name}: bytes"
        );
        let rows = want["rows"].as_array().unwrap();
        assert_eq!(got.rows.len(), rows.len(), "{name}: rows");
        for (i, (g, w)) in got.rows.iter().zip(rows).enumerate() {
            let at = format!("{name} row {i}");
            assert_eq!(g.id, w["id"].as_str().unwrap(), "{at}: id");
            assert_eq!(g.name, w["name"].as_str().unwrap(), "{at}: name");
            let ver: Vec<u64> = w["ver"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap())
                .collect();
            assert_eq!(
                [
                    u64::from(g.version.0),
                    u64::from(g.version.1),
                    u64::from(g.version.2)
                ]
                .to_vec(),
                ver,
                "{at}: version"
            );
            for (key, have) in [
                ("id_hash", u64::from(g.id_hash)),
                ("abi", u64::from(g.abi)),
                ("tau_min", u64::from(g.tau_min)),
                ("flags", u64::from(g.flags)),
                ("params_rev", u64::from(g.params_rev)),
                ("nparams", u64::from(g.nparams)),
                ("cmds", u64::from(g.cmds)),
                ("max_cycles", u64::from(g.max_cycles)),
                ("pack_off", u64::from(g.pack_off)),
                ("pack_size", u64::from(g.pack_size)),
                ("pack_crc", u64::from(g.pack_crc)),
                ("pstate", u64::from(g.pstate)),
                ("bss", u64::from(g.bss)),
                ("preview_off", u64::from(g.preview_off)),
                ("preview_len", u64::from(g.preview_len)),
            ] {
                assert_eq!(have, w[key].as_u64().unwrap(), "{at}: {key}");
            }
            let verdict = match meter_bin::availability(g, FW_ABI, FW_TAU) {
                Availability::Available => "yes",
                Availability::NeedsOtherAbi => "needs another ABI",
                Availability::NeedsNewerTau => "needs a newer TAU",
            };
            assert_eq!(
                verdict,
                w["available"].as_str().unwrap(),
                "{at}: availability"
            );
        }
    }
}

#[test]
fn the_seven_shipped_meters_come_in_order_with_winamp_bars_as_the_fallback_and_previews_in_range() {
    let b = bin("v2_seven.bin");
    let seven = meter_bin::parse(&b).unwrap();
    assert_eq!(seven.version, 2);
    assert_eq!(seven.rows[0].name, "WINAMP BARS");
    assert!(seven.rows[0].visible());
    assert!(
        seven
            .rows
            .iter()
            .all(|r| r.visible() && r.binary_only() && !r.from_v1()),
        "all seven are visible, binary-only, real v2 rows"
    );
    assert!(seven.rows.iter().all(|r| {
        meter_bin::tuning_collisions(&seven.rows).iter().all(|c| !c)
            && r.preview(&b)
                .is_some_and(|p| p.len() > 64 && p.len() <= 1024)
    }));
    for (i, r) in seven.rows.iter().enumerate() {
        assert!(
            meter_bin::pack_image_ok(&b, r),
            "row {i}: the pack image matches its CRC"
        );
    }
    let rev = meter_bin::parse(&bin("v2_reordered_hidden.bin")).unwrap();
    assert_eq!(
        rev.rows
            .iter()
            .filter(|r| !r.visible())
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        ["MASTER VU"],
        "exactly Master VU is hidden"
    );
    assert_eq!(
        rev.rows.last().unwrap().name,
        "WINAMP BARS",
        "the table of contents is the order: reversed"
    );
    assert_eq!(
        meter_bin::parse(&bin("v2_empty.bin")).unwrap().rows.len(),
        0
    );
}

/// A tripwire, not a wish: this records what the published files say TODAY. Their README says Winamp Bars is "flagged as the fallback", and
/// message 027 says a bin without a fallback row is refused by newer firmware (code 20), but no row of any v2 vector has flag bit 1 set, and
/// `expected.json`'s `fallback_id` (102d4d21...) is not Winamp Bars's id (7dddfe63..., from `packages/winamp_bars/meter.json`), which would
/// explain it: the vectors were composed with a stale id. Reported to Tau Alpha (our message 025). If they republish the vectors with the flag
/// set, this fails on purpose: update it and the note in `testdata/meter_bin/README.md`.
#[test]
fn the_published_vectors_carry_no_fallback_flag_on_any_row() {
    for name in [
        "v2_seven.bin",
        "v2_reordered_hidden.bin",
        "v2_unavailable_rows.bin",
    ] {
        let bin = meter_bin::parse(&bin(name)).unwrap();
        assert!(
            bin.fallback().is_none(),
            "{name}: a row now carries the fallback flag: the vectors changed, review this test"
        );
    }
    assert_eq!(
        json("expected.json")["fallback_id"],
        "102d4d21726303d2f1c571f55e7e71c0"
    );
    let winamp_bars = meter_bin::parse(&bin("v2_seven.bin")).unwrap().rows[0]
        .id
        .clone();
    assert_eq!(winamp_bars, "7dddfe63121e904843088c075cc28ce9");
    assert_ne!(
        winamp_bars,
        json("expected.json")["fallback_id"].as_str().unwrap(),
        "expected.json names another id as the fallback"
    );
}

#[test]
fn rows_the_firmware_cannot_run_are_unavailable_and_colliding_ids_are_flagged() {
    let u = meter_bin::parse(&bin("v2_unavailable_rows.bin")).unwrap();
    let verdicts: Vec<_> = u
        .rows
        .iter()
        .map(|r| meter_bin::availability(r, FW_ABI, FW_TAU))
        .collect();
    assert_eq!(
        verdicts,
        [
            Availability::Available,
            Availability::NeedsOtherAbi,
            Availability::NeedsNewerTau
        ],
        "the bin is sound; rows 1 and 2 are greyed"
    );
    let c = meter_bin::parse(&bin("v2_tuning_collision.bin")).unwrap();
    assert_eq!(
        meter_bin::tuning_collisions(&c.rows),
        [true, true, false],
        "rows 0 and 1 share the low 16 bits of id_hash"
    );
}

#[test]
fn what_only_the_loader_sees_is_found_when_a_meter_is_selected_not_at_boot() {
    // A damaged pack image: boot is fine, the whole-file CRC and the loader's pack CRC catch it.
    let b = bin("flipped_pack_byte.bin");
    let boot = meter_bin::open_toc(&b, b.len()).unwrap();
    assert_eq!(meter_bin::parse(&b).unwrap_err().refusal, Refusal::FileCrc);
    assert_eq!(
        boot.rows
            .iter()
            .filter(|r| !meter_bin::pack_image_ok(&b, r))
            .count(),
        1,
        "exactly one pack image is damaged"
    );
    // A row whose pack_crc is not its pack header's: found by the pack check, row 1.
    let b = bin("pack_crc_mismatch.bin");
    let rows = meter_bin::open_toc(&b, b.len()).unwrap().rows;
    let found: Vec<_> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| meter_bin::check_pack(&b, r, i).err())
        .collect();
    assert_eq!(
        (found.len(), found[0].refusal, found[0].row),
        (1, Refusal::Pack, Some(1))
    );
    // A pack linked for a slot, not for the window: row 0.
    let b = bin("pack_origin_slot.bin");
    let rows = meter_bin::open_toc(&b, b.len()).unwrap().rows;
    assert_eq!(
        meter_bin::check_pack(&b, &rows[0], 0).unwrap_err().refusal,
        Refusal::Origin
    );
}

#[test]
fn the_size_limit_is_four_point_two_five_mebibytes() {
    let mut big = vec![0u8; meter_bin::MAX_BIN + 1];
    big[..6].copy_from_slice(b"TPKB\x02\x00");
    assert_eq!(meter_bin::parse(&big).unwrap_err().refusal, Refusal::Size);
    assert_eq!(
        meter_bin::open_toc(&big, big.len()).unwrap_err().refusal,
        Refusal::Size
    );
    assert_eq!(meter_bin::MAX_BIN, 4_456_448);
}

#[test]
fn a_refusal_is_explained_in_plain_words_with_the_meter_counted_from_one() {
    let e = meter_bin::parse(&bin("duplicate_hash.bin")).unwrap_err();
    assert_eq!((e.refusal.code(), e.row), (9, Some(1)));
    assert_eq!(
        e.plain(),
        "Two meters in the file share the same short id (meter 2.)"
    );
    let e = meter_bin::parse(&bin("bad_file_crc.bin")).unwrap_err();
    assert!(e.plain().contains("checksum"), "{}", e.plain());
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
fn no_input_makes_the_reader_panic_and_every_accepted_row_points_inside_the_file() {
    let mut rng = Lcg(0x5EED);
    for name in [
        "v2_seven.bin",
        "v2_unavailable_rows.bin",
        "v2_tuning_collision.bin",
        "v1_seven.bin",
        "v2_empty.bin",
    ] {
        let base = bin(name);
        // every cut of the first 2,000 bytes and a spread of longer ones
        for n in (0..2000.min(base.len())).chain((0..40).map(|k| base.len() * k / 40)) {
            let _ = meter_bin::parse(&base[..n]);
            let _ = meter_bin::open_toc(&base[..n], base.len());
        }
        for _ in 0..1500 {
            let mut b = base.clone();
            for _ in 0..=rng.next() % 3 {
                let at = if rng.next().is_multiple_of(2) {
                    rng.next() % 600.min(b.len())
                } else {
                    rng.next() % b.len()
                }; // header and rows are where the checks are
                b[at] = (rng.next() & 255) as u8;
            }
            if rng.next().is_multiple_of(5) {
                b.truncate(rng.next() % (b.len() + 1));
            }
            if rng.next().is_multiple_of(9) {
                b.extend((0..rng.next() % 9).map(|_| (rng.next() & 255) as u8));
            }
            if let Ok(bin) = meter_bin::parse(&b) {
                for r in &bin.rows {
                    assert!(
                        r.pack_off as usize + r.pack_size as usize <= b.len(),
                        "{name}: a pack outside the file"
                    );
                    assert!(
                        r.preview(&b).is_some(),
                        "{name}: a preview outside the file"
                    );
                }
            }
            let _ = meter_bin::open_toc(&b, b.len());
        }
    }
}
