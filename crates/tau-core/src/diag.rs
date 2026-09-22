//! Read-only firmware diagnostics and persisted settings.

use crate::{ErrorCode, TauError};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PersistedSetting {
    pub id: u64,
    pub kind: String,
    pub value: Value,
}

/// Reads an Interact persisted-settings file without mutating its source.
pub fn read_persisted_settings(path: impl AsRef<Path>) -> Result<Vec<PersistedSetting>, TauError> {
    let bytes = fs::read(path)?;
    let json: Value = serde_json::from_slice(&bytes).map_err(|error| {
        TauError::e(
            ErrorCode::Json,
            format!("invalid persisted settings: {error}"),
        )
    })?;
    Ok(json
        .get("variables")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some(PersistedSetting {
                id: entry.get("id")?.as_u64()?,
                kind: entry
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                value: entry.get("val").cloned().unwrap_or(Value::Null),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn reads_known_variables_without_mutating_source() {
        let path = std::env::temp_dir().join(format!(
            "tau-settings-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            br#"{"variables":[{"id":10,"type":"u32","val":7},{"id":24,"type":"u32","val":123}]}"#,
        )
        .unwrap();
        let settings = read_persisted_settings(&path).unwrap();
        assert_eq!(settings.len(), 2);
        assert_eq!(settings[0].id, 10);
        assert_eq!(settings[0].kind, "u32");
        assert_eq!(settings[0].value, serde_json::json!(7));
        assert!(path.is_file());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_invalid_json() {
        let path = std::env::temp_dir().join("tau-settings-invalid.json");
        fs::write(&path, b"not json").unwrap();
        assert!(read_persisted_settings(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
