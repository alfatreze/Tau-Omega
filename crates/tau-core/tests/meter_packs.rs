//! `tau_core::meter_packs` against a REAL bundle (`testdata/meter_packs/tau-packs_dev184.bin`, the `tau-packs.bin` TAU DEV 184 ran on a Pocket, copied from
//! Tau Alpha `docs/research/p2_packs/`) and Tau Alpha's reference reader (`tau_assets.py parse_packs`, its output kept in `*.reference.json`).

use std::path::PathBuf;
use tau_core::meter_packs::{check_pack, compose_bundle, parse_bundle, split_bundle};

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/meter_packs")
        .join(name)
}
fn real() -> Vec<u8> {
    std::fs::read(fx("tau-packs_dev184.bin")).unwrap()
}

#[test]
fn the_real_bundle_reads_exactly_as_tau_alphas_reference_reader_does() {
    let reference: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fx("tau-packs_dev184.reference.json")).unwrap())
            .unwrap();
    let ours = parse_bundle(&real()).unwrap();
    let theirs = reference.as_array().unwrap();
    assert_eq!(ours.len(), theirs.len());
    for (o, t) in ours.iter().zip(theirs) {
        assert_eq!(u64::from(o.meter_id), t["meter"].as_u64().unwrap());
        assert_eq!(
            u64::from(o.abi_major << 8 | o.abi_minor),
            t["abi"].as_u64().unwrap()
        );
        assert_eq!(u64::from(o.image_size), t["size"].as_u64().unwrap());
        assert_eq!(u64::from(o.entry_offset), t["entry"].as_u64().unwrap());
    }
    assert_eq!(
        ours.iter().map(|p| p.meter_id).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6]
    );
}

#[test]
fn composing_all_the_packs_again_gives_the_identical_file_and_any_subset_is_valid() {
    let d = real();
    let packs = split_bundle(&d).unwrap();
    assert_eq!(
        compose_bundle(&packs).unwrap(),
        d,
        "the same packs make the same bundle, byte for byte"
    );
    // The order given does not matter: slot order is fixed.
    let mut shuffled = packs.clone();
    shuffled.reverse();
    assert_eq!(compose_bundle(&shuffled).unwrap(), d);
    // A chosen subset (Winamp Scope, Waterfall, SOUND MAP-like ids) is a valid bundle with those packs only.
    let pick = vec![packs[1].clone(), packs[5].clone(), packs[3].clone()];
    let bundle = compose_bundle(&pick).unwrap();
    assert_eq!(
        parse_bundle(&bundle)
            .unwrap()
            .iter()
            .map(|p| p.meter_id)
            .collect::<Vec<_>>(),
        [1, 3, 5]
    );
    assert_eq!(bundle.len(), 8 + pick.iter().map(Vec::len).sum::<usize>());
    // No packs at all: a valid empty bundle (8 bytes).
    let empty = compose_bundle(&[]).unwrap();
    assert_eq!(empty, [b'T', b'P', b'K', b'B', 1, 0, 0, 0]);
    assert!(parse_bundle(&empty).unwrap().is_empty());
}

#[test]
fn everything_the_firmware_would_refuse_is_refused() {
    let d = real();
    let packs = split_bundle(&d).unwrap();
    // a pack for another slot: origin that does not match the meter id
    let mut wrong_slot = packs[2].clone();
    wrong_slot[6] = 3; // meter id 3 at slot 2's origin
    assert!(
        check_pack(&wrong_slot)
            .unwrap_err()
            .message
            .contains("must be linked at")
    );
    // another ABI major, a minor above 2
    let mut abi2 = packs[0].clone();
    abi2[5] = 2;
    assert!(check_pack(&abi2).unwrap_err().message.contains("ABI"));
    let mut minor3 = packs[0].clone();
    minor3[4] = 3;
    assert!(check_pack(&minor3).unwrap_err().message.contains("ABI"));
    // a flipped image byte (CRC), a cut pack, a meter id outside the slots
    let mut flipped = packs[1].clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 1;
    assert!(check_pack(&flipped).unwrap_err().message.contains("CRC"));
    assert!(check_pack(&packs[1][..packs[1].len() - 1]).is_err());
    let mut high = packs[0].clone();
    high[6] = 8;
    assert!(check_pack(&high).unwrap_err().message.contains("outside"));
    // composing: a duplicate id, more than 8 packs
    assert!(
        compose_bundle(&[packs[0].clone(), packs[0].clone()])
            .unwrap_err()
            .message
            .contains("same meter")
    );
    assert!(
        compose_bundle(&vec![packs[0].clone(); 9])
            .unwrap_err()
            .message
            .contains("at most 8")
    );
    // reading a bundle: magic, version, count, trailing bytes, order
    assert!(parse_bundle(b"XPKB\x01\x00\x00\x00").is_err());
    let mut v2 = d.clone();
    v2[4] = 2;
    assert!(parse_bundle(&v2).unwrap_err().message.contains("version"));
    let mut trailing = d.clone();
    trailing.push(0);
    assert!(
        parse_bundle(&trailing)
            .unwrap_err()
            .message
            .contains("after the last")
    );
    let mut count = d.clone();
    count[6] = 9;
    assert!(
        parse_bundle(&count)
            .unwrap_err()
            .message
            .contains("at most 8")
    );
    let out_of_order = [&d[..8], &packs[1][..], &packs[0][..]].concat();
    let mut fixed = out_of_order.clone();
    fixed[6] = 2;
    assert!(
        parse_bundle(&fixed)
            .unwrap_err()
            .message
            .contains("increasing")
    );
    assert!(parse_bundle(&d[..d.len() - 5]).is_err());
}
