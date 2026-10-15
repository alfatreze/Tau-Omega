//! Where a platform sits in the openFPGA Library's grouping: **category > manufacturer > platform** (for example
//! Console > Nintendo > SNES). The grouping is the Library's own (`api/v3/platforms.json`); a snapshot of its
//! `id -> (category, manufacturer)` pairs is built in, so the grouping is the same offline and on every card. A platform the
//! Library does not know (Tau, a core's own platform) uses what its `Platforms/<id>.json` on the card says; with nothing, it
//! lands in "Other". The card's own file never overrides the Library, because several of them put a manufacturer where the
//! category belongs.

use std::{collections::HashMap, sync::OnceLock};

const SNAPSHOT: &str = include_str!("../data/library_platforms.json");

/// Shown when neither the Library nor the card gives a category.
pub const OTHER: &str = "Other";
/// Shown when no manufacturer is known.
pub const UNKNOWN_MAKER: &str = "Unknown";

fn library() -> &'static HashMap<String, (String, String)> {
    static MAP: OnceLock<HashMap<String, (String, String)>> = OnceLock::new();
    MAP.get_or_init(|| {
        serde_json::from_str::<Vec<(String, String, String)>>(SNAPSHOT)
            .unwrap_or_default()
            .into_iter()
            .map(|(id, cat, maker)| (id, (cat, maker)))
            .collect()
    })
}

/// `(category, manufacturer)` for a platform id; `card_*` are the card's own platform file values (may be empty).
pub fn resolve(id: &str, card_category: &str, card_manufacturer: &str) -> (String, String) {
    let or = |s: &str, fallback: &str| {
        if s.trim().is_empty() {
            fallback.to_string()
        } else {
            s.trim().to_string()
        }
    };
    match library().get(id) {
        Some((c, m)) => (or(c, OTHER), or(m, UNKNOWN_MAKER)),
        None => (
            or(card_category, OTHER),
            or(card_manufacturer, UNKNOWN_MAKER),
        ),
    }
}

/// Display order of categories: the Library's main ones first, then alphabetical, "Other" last.
pub fn category_rank(name: &str) -> (u8, String) {
    let rank = match name {
        "Console" => 0,
        "Handheld" => 1,
        "Computer" => 2,
        "Arcade" => 3,
        "Arcade Multi" => 4,
        "Media Players" => 5,
        OTHER => 9,
        _ => 6,
    };
    (rank, name.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_decides_and_the_card_fills_the_gaps() {
        assert_eq!(
            resolve("snes", "", ""),
            ("Console".into(), "Nintendo".into())
        );
        // a card file that puts the manufacturer in the category does not win over the Library
        assert_eq!(
            resolve("gba", "Nintendo", "Nintendo"),
            ("Handheld".into(), "Nintendo".into())
        );
        // unknown to the Library: the card's file
        assert_eq!(
            resolve("tau", "Media Players", "alfatreze"),
            ("Media Players".into(), "alfatreze".into())
        );
        assert_eq!(
            resolve("nothing", "", ""),
            (OTHER.into(), UNKNOWN_MAKER.into())
        );
    }

    #[test]
    fn the_snapshot_is_whole_and_ordering_is_stable() {
        assert!(library().len() > 300);
        let mut cats = vec![
            "Other",
            "Arcade",
            "Media Players",
            "Zeta",
            "Console",
            "Handheld",
            "Computer",
            "Alpha",
        ];
        cats.sort_by_key(|c| category_rank(c));
        assert_eq!(
            cats,
            [
                "Console",
                "Handheld",
                "Computer",
                "Arcade",
                "Media Players",
                "Alpha",
                "Zeta",
                "Other"
            ]
        );
    }
}
