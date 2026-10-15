//! Card roles for developers (`docs/DEV_LOOP_SPEC.md` section 5). A card is **Real** unless it carries a small marker file, `.omega-role`, at its
//! root saying it is a **Test** card. Real is the default on purpose: Omega never assumes a card is safe to experiment on. The marker travels with the
//! card, so the role follows it to another computer; it is a hidden file the Pocket ignores, and only this module writes or removes it.

use crate::{ErrorCode, TauError};
use std::{fs, path::Path};

pub const ROLE_FILE: &str = ".omega-role";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Role {
    Real,
    Test,
}

fn is_ours(bytes: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .is_some_and(|v| v.get("role").and_then(serde_json::Value::as_str).is_some())
}

/// The card's role: `Test` only when a valid marker says so; anything else (no file, unreadable, unknown content) is `Real`.
pub fn read(card: &Path) -> Role {
    fs::read(card.join(ROLE_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| {
            v.get("role")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .map_or(Role::Real, |r| {
            if r == "test" { Role::Test } else { Role::Real }
        })
}

/// Marks the card as a test card (writes the marker). `name` is the card's volume name, kept in the file so a reader can see which card it was set on.
pub fn set_test(card: &Path, name: &str, now: u64) -> Result<(), TauError> {
    if !card.join("Cores").is_dir() {
        return Err(TauError::e(
            ErrorCode::InstallRefused,
            "This folder is not a Pocket card (no Cores folder).",
        ));
    }
    let body = serde_json::json!({ "role": "test", "card": name, "set": now });
    let tmp = card.join(format!("{ROLE_FILE}.part"));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(&body)
            .map_err(|e| TauError::e(ErrorCode::Json, e.to_string()))?,
    )?;
    fs::rename(&tmp, card.join(ROLE_FILE))?;
    let _ = fs::remove_file(card.join(format!("._{ROLE_FILE}")));
    Ok(())
}

/// Makes the card Real again by removing the marker. Only a file with the marker's shape is removed; anything else at that name is left and reported.
pub fn clear(card: &Path) -> Result<bool, TauError> {
    let path = card.join(ROLE_FILE);
    let Ok(bytes) = fs::read(&path) else {
        return Ok(false);
    };
    if !is_ours(&bytes) {
        return Err(TauError::e(
            ErrorCode::InstallRefused,
            format!("{ROLE_FILE} on this card is not an Omega marker, so it was left alone."),
        ));
    }
    fs::remove_file(&path)?;
    let _ = fs::remove_file(card.join(format!("._{ROLE_FILE}")));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corehub::testcard::card;

    #[test]
    fn a_card_is_real_until_marked_and_real_again_when_cleared() {
        let c = card("role1");
        assert_eq!(read(&c), Role::Real);
        set_test(&c, "CARDWRITE", 1).unwrap();
        assert_eq!(read(&c), Role::Test);
        assert!(clear(&c).unwrap());
        assert_eq!(read(&c), Role::Real);
        assert!(!clear(&c).unwrap());
    }

    #[test]
    fn garbage_means_real_and_a_foreign_file_is_never_removed() {
        let c = card("role2");
        fs::write(c.join(ROLE_FILE), b"not json").unwrap();
        assert_eq!(read(&c), Role::Real);
        assert!(clear(&c).is_err());
        assert!(c.join(ROLE_FILE).exists());
        fs::write(c.join(ROLE_FILE), br#"{"role":"admin"}"#).unwrap();
        assert_eq!(read(&c), Role::Real);
    }

    #[test]
    fn a_folder_that_is_not_a_card_is_refused() {
        let c = std::env::temp_dir().join("omega-role-not-a-card");
        let _ = fs::create_dir_all(&c);
        assert!(set_test(&c, "x", 1).is_err());
    }
}
