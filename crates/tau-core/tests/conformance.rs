use serde_json::Value;
use std::{collections::BTreeMap, fs, path::PathBuf};
use tau_core::{Entry, Playlist, ROOT_PREFIX, ascii_text, build_index, parse, scan_dir};

fn testdata(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(name)
}

fn entries(name: &str) -> Vec<Entry> {
    serde_json::from_slice::<Vec<Value>>(&fs::read(testdata(name)).unwrap())
        .unwrap()
        .into_iter()
        .map(|entry| {
            let tags = entry["tags"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().into()))
                .collect::<BTreeMap<_, _>>();
            Entry {
                rel: entry["rel"].as_str().unwrap().into(),
                dir: entry["dir"].as_str().unwrap().into(),
                file: entry["file"].as_str().unwrap().into(),
                tags,
                secs: entry["secs"].as_u64().unwrap() as u16,
                fmt: entry["fmt"].as_u64().unwrap() as u8,
            }
        })
        .collect()
}

#[test]
fn fixture_index_is_byte_identical_to_python() {
    let common = testdata("fixture/common");
    let scan = scan_dir(&common, true).unwrap();
    let mut warnings = Vec::new();
    let got = build_index(&scan.entries, &scan.playlists, ROOT_PREFIX, &mut warnings).unwrap();
    assert_eq!(got, fs::read(testdata("fixture.tdb")).unwrap());
}

#[test]
fn oracle_synths_are_byte_identical() {
    for tracks in [400, 7180, 16384] {
        let mut warnings = Vec::new();
        let got = build_index(
            &entries(&format!("synth-{tracks}.json")),
            &[],
            ROOT_PREFIX,
            &mut warnings,
        )
        .unwrap();
        assert_eq!(
            got,
            fs::read(testdata(&format!("synth-{tracks}.tdb"))).unwrap(),
            "{tracks} tracks"
        );
    }
}

#[test]
fn playlist_index_matches_oracle() {
    let source = entries("synth-400.json");
    let playlist = Playlist {
        name: "Golden list".into(),
        rel_ids: (0..16).collect(),
    };
    let mut warnings = Vec::new();
    assert_eq!(
        build_index(&source, &[playlist], ROOT_PREFIX, &mut warnings).unwrap(),
        fs::read(testdata("synth-400-playlists.tdb")).unwrap()
    );
}

#[test]
fn corruption_codes_match_oracle() {
    let matrix: Value =
        serde_json::from_slice(&fs::read(testdata("vectors.json")).unwrap()).unwrap();
    for (name, case) in matrix["corruption"].as_object().unwrap() {
        let blob = fs::read(testdata(case["file"].as_str().unwrap())).unwrap();
        assert_eq!(
            parse(blob)
                .err()
                .and_then(|error| error.code())
                .unwrap_or(0),
            case["code"].as_u64().unwrap() as u8,
            "{name}"
        );
    }
}

#[test]
fn ascii_vectors_match_python() {
    let vectors: Value =
        serde_json::from_slice(&fs::read(testdata("vectors.json")).unwrap()).unwrap();
    for (input, expected) in vectors["ascii"].as_object().unwrap() {
        assert_eq!(ascii_text(input, 63), expected.as_str().unwrap());
    }
}
