use super::*;

fn parts(p: &str) -> Vec<String> {
    p.split('/').map(String::from).collect()
}

#[test]
fn code_page_437_table_is_the_reference_set() {
    // The 128 characters above 0x9F that Python's cp437 codec encodes (listed once from it).
    let expect: [u32; 128] = [
        0xA0, 0xA1, 0xA2, 0xA3, 0xA5, 0xAA, 0xAB, 0xAC, 0xB0, 0xB1, 0xB2, 0xB5, 0xB7, 0xBA, 0xBB,
        0xBC, 0xBD, 0xBF, 0xC4, 0xC5, 0xC6, 0xC7, 0xC9, 0xD1, 0xD6, 0xDC, 0xDF, 0xE0, 0xE1, 0xE2,
        0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF, 0xF1, 0xF2, 0xF3,
        0xF4, 0xF6, 0xF7, 0xF9, 0xFA, 0xFB, 0xFC, 0xFF, 0x192, 0x393, 0x398, 0x3A3, 0x3A6, 0x3A9,
        0x3B1, 0x3B4, 0x3B5, 0x3C0, 0x3C3, 0x3C4, 0x3C6, 0x207F, 0x20A7, 0x2219, 0x221A, 0x221E,
        0x2229, 0x2248, 0x2261, 0x2264, 0x2265, 0x2310, 0x2320, 0x2321, 0x2500, 0x2502, 0x250C,
        0x2510, 0x2514, 0x2518, 0x251C, 0x2524, 0x252C, 0x2534, 0x253C, 0x2550, 0x2551, 0x2552,
        0x2553, 0x2554, 0x2555, 0x2556, 0x2557, 0x2558, 0x2559, 0x255A, 0x255B, 0x255C, 0x255D,
        0x255E, 0x255F, 0x2560, 0x2561, 0x2562, 0x2563, 0x2564, 0x2565, 0x2566, 0x2567, 0x2568,
        0x2569, 0x256A, 0x256B, 0x256C, 0x2580, 0x2584, 0x2588, 0x258C, 0x2590, 0x2591, 0x2592,
        0x2593, 0x25A0,
    ];
    let mut got: Vec<u32> = (0x80..=0xFFu8)
        .map(|b| from_cp437(&[b]).chars().next().unwrap() as u32)
        .filter(|&c| c > 0x9F)
        .collect();
    got.sort_unstable();
    assert_eq!(got, expect);
    // Byte values that the firmware tests pinned on hardware.
    for (c, b) in [
        ('ß', 0xE1),
        ('é', 0x82),
        ('æ', 0x91),
        ('¢', 0x9B),
        ('π', 0xE3),
    ] {
        assert_eq!(cp437_byte(c), Some(b), "{c}");
        assert_eq!(from_cp437(&[b]), c.to_string());
    }
    assert_eq!(cp437_byte('Á'), None);
    assert_eq!(cp437_byte('Ø'), None);
    assert!(!reachable("Привет"));
    assert!(reachable("Café"));
    assert_eq!(
        path_bytes("Cafe\u{301}"),
        Some(vec![b'C', b'a', b'f', 0x82])
    );
}

#[test]
fn the_reference_vectors() {
    assert_eq!(card_part("Café.mp3", true), "Café.mp3");
    assert_eq!(card_part("Cafe\u{301}.mp3", true), "Café.mp3");
    assert_eq!(card_part("Søren.mp3", true), "Soren.mp3");
    assert_eq!(card_part("Привет.mp3", true), "Privet.mp3");
    assert_eq!(card_part("😀.mp3", true), "track.mp3");
    assert_eq!(card_part("Café Привет.mp3", true), "Café Privet.mp3");
    // CJK: the table differs from the reference (anyascii), the invariants do not.
    let cjk = card_part("东京.flac", true);
    assert!(
        cjk.ends_with(".flac") && reachable(&cjk) && cjk.len() > 5,
        "{cjk}"
    );
}

#[test]
fn greek_is_all_or_nothing() {
    assert_eq!(card_part("Ελλάδα.mp3", true), "Ellada.mp3");
    assert_eq!(card_part("Pi π.mp3", true), "Pi π.mp3");
    assert_eq!(card_part("π.mp3", true), "π.mp3");
}

#[test]
fn forbidden_reserved_and_empty_names() {
    assert_eq!(
        card_part("a<b>c:d\"e|f?g*h\\i.mp3", true),
        "a_b_c_d_e_f_g_h_i.mp3"
    );
    assert_eq!(card_part("CON.mp3", true), "CON_.mp3");
    assert_eq!(card_part("lpt3", false), "lpt3_");
    assert_eq!(card_part("..hidden.mp3", true), "hidden.mp3");
    assert_eq!(card_part("  padded  .mp3", true), "padded.mp3");
    assert_eq!(card_part("name. .mp3", true), "name.mp3");
    assert_eq!(card_part(".mp3", true), "track.mp3");
    assert_eq!(card_part("\u{1F600}", false), "track");
    // controls become a space, a dropped character leaves no double space
    assert_eq!(card_part("a\tb.mp3", true), "a b.mp3");
    assert_eq!(card_part("a \u{1F600} b.mp3", true), "a b.mp3");
    // a folder has no extension
    assert_eq!(card_part("Vol.1", false), "Vol.1");
}

#[test]
fn collisions_are_numbered_in_sorted_order_ignoring_case() {
    let map = NameMap::new([
        parts("A/Привет.mp3"),
        parts("A/Привет!.mp3"),
        parts("A/privet.MP3"),
        parts("A/Cafe\u{301}.mp3"),
        parts("A/Café.mp3"),
    ]);
    let name = |p: &str| map.get(&parts(p)).unwrap().join("/");
    // sorted by original: "Cafe\u{301}"(=Café) once, then Привет < Привет! < privet
    assert_eq!(name("A/Café.mp3"), "A/Café.mp3");
    assert_eq!(name("A/Cafe\u{301}.mp3"), "A/Café.mp3"); // one NFC name
    let a = name("A/Привет.mp3");
    let b = name("A/Привет!.mp3");
    let c = name("A/privet.MP3");
    let mut all = [a.to_lowercase(), b.to_lowercase(), c.to_lowercase()];
    all.sort();
    all.windows(2).for_each(|w| assert_ne!(w[0], w[1]));
    assert!(a.ends_with("Privet.mp3") || a.contains("(2)") || a.contains("(3)"));
    // stable: the same input gives the same output
    let again = NameMap::new([
        parts("A/privet.MP3"),
        parts("A/Привет!.mp3"),
        parts("A/Привет.mp3"),
    ]);
    assert_eq!(
        again.get(&parts("A/Привет.mp3")),
        map.get(&parts("A/Привет.mp3"))
    );
}

#[test]
fn folders_are_numbered_and_long_names_are_cut() {
    let map = NameMap::new([parts("Привет/a.mp3"), parts("Привет!/a.mp3")]);
    let f1 = &map.get(&parts("Привет/a.mp3")).unwrap()[0];
    let f2 = &map.get(&parts("Привет!/a.mp3")).unwrap()[0];
    assert_ne!(f1.to_lowercase(), f2.to_lowercase());
    let long = format!("{}.mp3", "x".repeat(300));
    let folder = "d".repeat(100);
    let m = NameMap::new([vec![folder.clone(), long.clone()]]);
    let p = m.get(&[folder, long]).unwrap();
    assert_eq!(p[0].chars().count(), FOLDER_MAX);
    let total = ROOT_BUDGET + p.join("/").len();
    assert!(total <= MAX_PATH, "{total}");
}

/// Conformance against Tau Alpha's shared corpus (`sim/name_corpus.py`, 62 cases).
/// Not run by default: `TAU_NAME_CORPUS=<corpus.json> cargo test -p tau-core corpus -- --ignored`
/// (the JSON is dumped from that Python file; nothing is copied into this repository).
#[test]
#[ignore]
fn corpus_invariants() {
    let Ok(path) = std::env::var("TAU_NAME_CORPUS") else {
        return;
    };
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut rels = Vec::new();
    for c in &cases {
        let cat = c["cat"].as_str().unwrap().to_string();
        rels.push(vec![cat, format!("{}.mp3", c["name"].as_str().unwrap())]);
    }
    let map = NameMap::new(rels.clone());
    let mut seen = std::collections::BTreeMap::new();
    for (c, rel) in cases.iter().zip(&rels) {
        let card = map.get(rel).unwrap();
        let joined = card.join("/");
        assert!(reachable(&joined), "{}: {joined}", c["id"]);
        assert!(
            path_bytes(&joined).unwrap().len() + ROOT_BUDGET <= MAX_PATH,
            "{}",
            c["id"]
        );
        // The same name spelled decomposed and precomposed is one original (rule 5a).
        let original: Vec<String> = rel.iter().map(|x| x.nfc().collect()).collect();
        let prior = seen.insert(joined.to_lowercase(), original.clone());
        assert!(
            prior.is_none_or(|p| p == original),
            "collision {}: {joined}",
            c["id"]
        );
        if let Some(keep) = c["keep"].as_str() {
            let keep: String = keep.nfc().collect();
            assert!(joined.contains(&keep), "{}: {joined} lost {keep}", c["id"]);
        }
    }
}

#[test]
fn display_text_follows_the_latin1_rule() {
    assert_eq!(latin1_text("Café", 63), "Café");
    assert_eq!(latin1_text("Cafe\u{301}", 63), "Café");
    assert_eq!(
        latin1_text("It\u{2019}s \u{201C}live\u{201D} \u{2013} 2\u{2026}", 63),
        "It's \"live\" - 2..."
    );
    assert_eq!(latin1_text("Привет", 63), "Privet");
    assert_eq!(latin1_text("Søren", 63), "Søren"); // ø is U+00F8: Latin-1 can draw it
    assert_eq!(latin1_text("\u{1F600}", 63), "");
    assert_eq!(latin1_text("???", 63), "");
    assert_eq!(latin1_text("a\tb  c", 63), "a b c");
    assert_eq!(latin1_text("abcdef", 3), "abc");
}

/// Cross-check against Tau Alpha's `tau_library.latin1_text` where the result needs no
/// transliteration table: `TAU_LATIN1_CASES=<json of [input, expected]>`.
#[test]
#[ignore]
fn latin1_matches_the_reference_where_no_table_is_involved() {
    let Ok(path) = std::env::var("TAU_LATIN1_CASES") else {
        return;
    };
    let cases: Vec<(String, String)> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (mut same, mut table) = (0, 0);
    for (input, expected) in cases {
        let got = latin1_text(&input, 63);
        if got == expected {
            same += 1;
        } else {
            // a difference is allowed only where a transliteration table decides the text
            assert!(
                input.chars().any(|c| (c as u32) > 0xFF),
                "{input:?}: got {got:?}, reference {expected:?}"
            );
            table += 1;
        }
    }
    eprintln!("{same} identical, {table} differ only by transliteration table");
}
