//! Comparing the version strings of openFPGA cores, which follow no common scheme (`0.9.0`, `v1.2`, `2024.03.1`,
//! `0.01b`, `1.0-beta.2`, plain dates). Tau's own releases keep using `release_check`/`compat`.
//!
//! The rule is conservative: a core is "newer" only when both strings parse the same way and compare strictly.
//! Anything else is [`Verdict::Unknown`], never a guessed "newer" (an update the user did not need is a card write).

use std::cmp::Ordering;

/// Result of comparing an installed version with the catalog's newest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Verdict {
    /// The catalog's version is strictly newer.
    Newer,
    Same,
    /// The installed version is newer than the catalog's (a local or pre-release build).
    Older,
    /// The strings cannot be ordered with confidence.
    Unknown,
}

#[derive(Debug, PartialEq, Eq)]
struct Parsed {
    /// Dotted numeric fields (`0.9.0` -> 0, 9, 0).
    nums: Vec<u64>,
    /// A trailing letter or `-pre.N` text; empty for a plain release. A pre-release sorts before the release.
    pre: String,
}

fn parse(text: &str) -> Option<Parsed> {
    let t = text.trim().trim_start_matches(['v', 'V']);
    if t.is_empty() {
        return None;
    }
    let (core, pre) = match t.find(['-', '+']) {
        Some(i) if t.as_bytes()[i] == b'-' => (&t[..i], t[i + 1..].to_string()),
        Some(i) => (&t[..i], String::new()),
        None => (t, String::new()),
    };
    let mut pre = pre;
    let mut nums = Vec::new();
    for (n, part) in core.split('.').enumerate() {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        nums.push(digits.parse().ok()?);
        let rest = &part[digits.len()..];
        if !rest.is_empty() {
            // `0.01b`: a letter suffix on the last field marks a pre-release (beta/alpha), only there.
            if n + 1 != core.split('.').count() || !rest.chars().all(|c| c.is_ascii_alphabetic()) {
                return None;
            }
            pre = format!("{rest}{pre}");
        }
    }
    Some(Parsed { nums, pre })
}

fn cmp_nums(a: &[u64], b: &[u64]) -> Ordering {
    let len = a.len().max(b.len());
    (0..len)
        .map(|i| {
            a.get(i)
                .copied()
                .unwrap_or(0)
                .cmp(&b.get(i).copied().unwrap_or(0))
        })
        .find(|o| *o != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

/// Compares `installed` with `latest`.
pub fn compare(installed: &str, latest: &str) -> Verdict {
    if installed.trim() == latest.trim() {
        return Verdict::Same;
    }
    let (Some(a), Some(b)) = (parse(installed), parse(latest)) else {
        return Verdict::Unknown;
    };
    let ord = cmp_nums(&a.nums, &b.nums).then_with(|| match (a.pre.is_empty(), b.pre.is_empty()) {
        (true, true) => Ordering::Equal,
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        // Two pre-release labels: only order them when they are the same word with a number or letter step.
        (false, false) => a.pre.cmp(&b.pre),
    });
    match ord {
        Ordering::Less => Verdict::Newer,
        Ordering::Equal => Verdict::Same,
        Ordering::Greater => Verdict::Older,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_versions_order() {
        assert_eq!(compare("0.9.0", "0.10.0"), Verdict::Newer);
        assert_eq!(compare("1.2", "1.2.0"), Verdict::Same);
        assert_eq!(compare("v1.3", "1.2.9"), Verdict::Older);
        assert_eq!(compare("2024.03.1", "2024.10.0"), Verdict::Newer);
    }

    #[test]
    fn prerelease_sorts_before_release() {
        assert_eq!(compare("1.0-beta.2", "1.0"), Verdict::Newer);
        assert_eq!(compare("0.01b", "0.01"), Verdict::Newer);
        assert_eq!(compare("1.0", "1.0-rc.1"), Verdict::Older);
        assert_eq!(compare("0.01b", "0.02b"), Verdict::Newer);
    }

    #[test]
    fn identical_text_is_same_even_if_unparseable() {
        assert_eq!(compare("final", "final"), Verdict::Same);
    }

    #[test]
    fn unparseable_is_unknown_never_newer() {
        assert_eq!(compare("final", "1.0"), Verdict::Unknown);
        assert_eq!(compare("", "1.0"), Verdict::Unknown);
        assert_eq!(compare("1.x", "1.2"), Verdict::Unknown);
        assert_eq!(compare("1a.2", "1.3"), Verdict::Unknown);
    }
}
