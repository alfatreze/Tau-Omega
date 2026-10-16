//! Build series for developers (`docs/DEV_LOOP_SPEC.md`, the Builds page): which test builds belong together and which is newer. Every developer names
//! builds their own way, so the naming is a list of **schemes** the developer can edit. A scheme reads one field of a build (its core id, its shortname,
//! its version or its file name) with a small template:
//!
//! | token | matches |
//! |---|---|
//! | `{series}` | the text that names the series ("TAU DEV"); builds with the same series belong together |
//! | `{n}` | a build counter ("111") |
//! | `{major}` `{minor}` `{patch}` | SemVer numbers |
//! | `{pre}` | a SemVer pre-release ("alpha.3", "rc1") |
//! | `{any}` | anything, ignored |
//!
//! Everything else in the template is literal text (letters match in any case). The first scheme that matches a build decides its series and its place in
//! it; a build no scheme matches is simply not part of a series. No regular-expression library is involved, and a match gives up after a fixed number of attempts, so a template can never run away.

use std::{cmp::Ordering, collections::BTreeMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum Field {
    CoreId,
    Shortname,
    Version,
    /// The package's file name without `.zip`.
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scheme {
    pub name: String,
    pub field: Field,
    pub template: String,
}

/// The facts a scheme can read from one build (a package on disk or a core on the card).
#[derive(Debug, Clone, Default)]
pub struct Subject<'a> {
    pub core_id: &'a str,
    pub shortname: &'a str,
    pub version: &'a str,
    pub file: &'a str,
}

/// Where one build sits: its series, and its position in it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Placement {
    pub scheme: String,
    pub series: String,
    /// `[major, minor, patch, n]`; a number the template does not capture is 0.
    pub order: [u64; 4],
    pub pre: Option<String>,
}

/// The schemes every install has; a developer's own come first.
pub fn builtin() -> Vec<Scheme> {
    let s = |name: &str, field, template: &str| Scheme {
        name: name.into(),
        field,
        template: template.into(),
    };
    vec![
        s(
            "Name and number (TAU DEV 12)",
            Field::Shortname,
            "{series} {n}",
        ),
        s(
            "SemVer pre-release (0.6.0-alpha.3)",
            Field::Version,
            "{major}.{minor}.{patch}-{pre}",
        ),
        s("SemVer (0.6.0)", Field::Version, "{major}.{minor}.{patch}"),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Series,
    N,
    Major,
    Minor,
    Patch,
    Pre,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Lit(String),
    Var(Tok),
}

fn parse(template: &str) -> Result<Vec<Seg>, String> {
    let mut segs: Vec<Seg> = Vec::new();
    let mut lit = String::new();
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(x) => name.push(x),
                        None => return Err("A { has no closing }.".into()),
                    }
                }
                let tok = match name.as_str() {
                    "series" => Tok::Series,
                    "n" => Tok::N,
                    "major" => Tok::Major,
                    "minor" => Tok::Minor,
                    "patch" => Tok::Patch,
                    "pre" => Tok::Pre,
                    "any" => Tok::Any,
                    other => {
                        return Err(format!(
                            "{{{other}}} is not a known token. Use {{series}}, {{n}}, {{major}}, {{minor}}, {{patch}}, {{pre}} or {{any}}."
                        ));
                    }
                };
                if !lit.is_empty() {
                    segs.push(Seg::Lit(std::mem::take(&mut lit)));
                }
                segs.push(Seg::Var(tok));
            }
            '}' => return Err("A } has no opening {.".into()),
            other => lit.push(other),
        }
    }
    if !lit.is_empty() {
        segs.push(Seg::Lit(lit));
    }
    Ok(segs)
}

/// Checks a template a developer wrote: known tokens only, no two tokens touching (the split would be a guess), each at most once, and something to
/// order builds by.
pub fn validate(template: &str) -> Result<(), String> {
    if template.trim().is_empty() {
        return Err("The template is empty.".into());
    }
    let segs = parse(template)?;
    let mut seen: Vec<&Tok> = Vec::new();
    for w in segs.windows(2) {
        if matches!((&w[0], &w[1]), (Seg::Var(_), Seg::Var(_))) {
            return Err(
                "Two tokens touch. Put some text between them so Omega knows where one ends."
                    .into(),
            );
        }
    }
    for s in &segs {
        if let Seg::Var(t) = s {
            if *t != Tok::Any && seen.contains(&t) {
                return Err("A token is used twice. Each of {series}, {n}, {major}, {minor}, {patch} and {pre} may appear once.".into());
            }
            seen.push(t);
        }
    }
    if !seen
        .iter()
        .any(|t| matches!(t, Tok::N | Tok::Major | Tok::Minor | Tok::Patch | Tok::Pre))
    {
        return Err("Nothing here orders the builds. Add {n}, a SemVer token or {pre}.".into());
    }
    Ok(())
}

fn pre_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

/// How many attempts one match may make before it gives up (a hostile template and name cannot run away).
const BUDGET: u32 = 100_000;

fn matches_at(
    segs: &[Seg],
    s: &str,
    pos: usize,
    caps: &mut BTreeMap<&'static str, String>,
    budget: &mut u32,
) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let Some((first, rest)) = segs.split_first() else {
        return pos == s.len();
    };
    match first {
        Seg::Lit(l) => {
            s.get(pos..pos + l.len())
                .is_some_and(|p| p.eq_ignore_ascii_case(l))
                && matches_at(rest, s, pos + l.len(), caps, budget)
        }
        Seg::Var(tok) => {
            let name = match tok {
                Tok::Series => "series",
                Tok::N => "n",
                Tok::Major => "major",
                Tok::Minor => "minor",
                Tok::Patch => "patch",
                Tok::Pre => "pre",
                Tok::Any => "any",
            };
            let tail = &s[pos..];
            // candidate lengths in bytes, in the order to try them
            let mut cands: Vec<usize> = Vec::new();
            match tok {
                Tok::N | Tok::Major | Tok::Minor | Tok::Patch => {
                    let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
                    cands.extend((1..=digits).rev());
                }
                Tok::Pre => {
                    let ok = tail
                        .char_indices()
                        .take_while(|(_, c)| pre_char(*c))
                        .map(|(i, c)| i + c.len_utf8())
                        .collect::<Vec<_>>();
                    cands.extend(ok.into_iter().rev());
                }
                Tok::Series | Tok::Any => {
                    cands.extend(tail.char_indices().map(|(i, c)| i + c.len_utf8()));
                }
            }
            for len in cands {
                if *tok != Tok::Any {
                    caps.insert(name, tail[..len].to_string());
                }
                if matches_at(rest, s, pos + len, caps, budget) {
                    return true;
                }
                caps.remove(name);
            }
            false
        }
    }
}

fn number(caps: &BTreeMap<&'static str, String>, k: &str) -> u64 {
    caps.get(k).and_then(|v| v.parse().ok()).unwrap_or(0)
}

/// Places one build with one scheme, or `None` when the scheme does not match. A scheme without `{series}` puts every build of the same core id in one
/// series (`fallback_series`).
pub fn place(scheme: &Scheme, subject: &Subject, fallback_series: &str) -> Option<Placement> {
    let segs = parse(&scheme.template).ok()?;
    let text = match scheme.field {
        Field::CoreId => subject.core_id,
        Field::Shortname => subject.shortname,
        Field::Version => subject.version,
        Field::File => subject.file,
    };
    let mut caps = BTreeMap::new();
    if !matches_at(&segs, text.trim(), 0, &mut caps, &mut BUDGET.clone()) {
        return None;
    }
    let series = caps
        .get("series")
        .map_or_else(|| fallback_series.to_string(), |s| s.trim().to_string())
        .to_lowercase();
    Some(Placement {
        scheme: scheme.name.clone(),
        series,
        order: [
            number(&caps, "major"),
            number(&caps, "minor"),
            number(&caps, "patch"),
            number(&caps, "n"),
        ],
        pre: caps.get("pre").cloned(),
    })
}

fn captures(scheme: &Scheme, text: &str) -> Option<(Vec<Seg>, BTreeMap<&'static str, String>)> {
    let segs = parse(&scheme.template).ok()?;
    let mut caps = BTreeMap::new();
    matches_at(&segs, text.trim(), 0, &mut caps, &mut BUDGET.clone()).then_some((segs, caps))
}

/// The name that would come next after `text` under one scheme: the counter plus one (keeping leading zeros), else the number at the end of the
/// pre-release plus one (`alpha.3` becomes `alpha.4`), else the patch number plus one. `None` when the scheme does not match, has `{any}` (which is not
/// kept, so the name cannot be rebuilt), or has nothing to count.
pub fn next_text(scheme: &Scheme, text: &str) -> Option<String> {
    let (segs, mut caps) = captures(scheme, text)?;
    if segs.iter().any(|s| matches!(s, Seg::Var(Tok::Any))) {
        return None;
    }
    if let Some(n) = caps.get("n").cloned() {
        let next = n.parse::<u64>().ok()?.checked_add(1)?;
        caps.insert("n", format!("{next:0width$}", width = n.len()));
    } else if let Some(pre) = caps.get("pre").cloned() {
        let digits_end = pre.trim_end_matches(|c: char| !c.is_ascii_digit()).len();
        let start = pre[..digits_end]
            .rfind(|c: char| !c.is_ascii_digit())
            .map_or(0, |i| i + 1);
        let n = pre[start..digits_end].parse::<u64>().ok()?;
        caps.insert(
            "pre",
            format!(
                "{}{}{}",
                &pre[..start],
                n.checked_add(1)?,
                &pre[digits_end..]
            ),
        );
    } else {
        let p = caps.get("patch").cloned()?;
        caps.insert("patch", p.parse::<u64>().ok()?.checked_add(1)?.to_string());
    }
    Some(
        segs.iter()
            .map(|s| match s {
                Seg::Lit(l) => l.clone(),
                Seg::Var(t) => {
                    let k = match t {
                        Tok::Series => "series",
                        Tok::N => "n",
                        Tok::Major => "major",
                        Tok::Minor => "minor",
                        Tok::Patch => "patch",
                        Tok::Pre => "pre",
                        Tok::Any => "any",
                    };
                    caps.get(k).cloned().unwrap_or_default()
                }
            })
            .collect(),
    )
}

/// The next name for `text` (a value of `field`), by the first scheme reading that field that matches; returns that scheme's name too.
pub fn next_name(schemes: &[Scheme], field: Field, text: &str) -> Option<(String, String)> {
    schemes
        .iter()
        .filter(|s| s.field == field)
        .find_map(|s| next_text(s, text).map(|n| (s.name.clone(), n)))
}

/// The first scheme that matches.
pub fn classify(schemes: &[Scheme], subject: &Subject) -> Option<Placement> {
    let fallback = subject.core_id.to_lowercase();
    schemes.iter().find_map(|s| place(s, subject, &fallback))
}

fn pre_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.split('.'), b.split('.'));
    loop {
        match (x.next(), y.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) => {
                let o = match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(m), Ok(n)) => m.cmp(&n),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => p.cmp(q),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
        }
    }
}

/// Older first. A release is newer than its own pre-releases (SemVer).
pub fn compare(a: &Placement, b: &Placement) -> Ordering {
    a.order.cmp(&b.order).then_with(|| match (&a.pre, &b.pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(p), Some(q)) => pre_cmp(p, q),
    })
}

/// The built-ins followed by nothing, or a developer's own first.
pub fn with_custom(mut custom: Vec<Scheme>) -> Vec<Scheme> {
    custom.retain(|s| validate(&s.template).is_ok());
    custom.extend(builtin());
    custom
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject<'a>(id: &'a str, short: &'a str, ver: &'a str, file: &'a str) -> Subject<'a> {
        Subject {
            core_id: id,
            shortname: short,
            version: ver,
            file,
        }
    }

    #[test]
    fn name_and_number_builds_share_a_series_and_order_by_the_number() {
        let s = builtin();
        let a = classify(
            &s,
            &subject("alfatreze.TAU DEV 9", "TAU DEV 9", "0.6.0", ""),
        )
        .unwrap();
        let b = classify(
            &s,
            &subject("alfatreze.TAU DEV 111", "TAU DEV 111", "0.6.0", ""),
        )
        .unwrap();
        assert_eq!(a.series, "tau dev");
        assert_eq!(a.series, b.series);
        assert_eq!(
            compare(&a, &b),
            Ordering::Less,
            "9 is older than 111, not 'greater' as text"
        );
    }

    #[test]
    fn semver_builds_of_one_core_order_with_pre_releases_below_the_release() {
        let s = builtin();
        let p = |v: &str| classify(&s, &subject("a.Core", "Core", v, "")).unwrap();
        let (a3, a10, rc, rel) = (
            p("0.6.0-alpha.3"),
            p("0.6.0-alpha.10"),
            p("0.6.0-rc.1"),
            p("0.6.0"),
        );
        assert_eq!(a3.series, "a.core");
        assert_eq!(compare(&a3, &a10), Ordering::Less);
        assert_eq!(compare(&a10, &rc), Ordering::Less);
        assert_eq!(compare(&rc, &rel), Ordering::Less);
        assert_eq!(compare(&p("0.7.0-alpha.1"), &rel), Ordering::Greater);
    }

    #[test]
    fn a_developers_own_scheme_wins_and_the_file_name_can_be_read() {
        let mine = Scheme {
            name: "nightly".into(),
            field: Field::File,
            template: "{series}-nightly-{n}".into(),
        };
        let s = with_custom(vec![mine]);
        let p = classify(&s, &subject("x.Y", "Y", "1.0.0", "Pocket-nightly-0042")).unwrap();
        assert_eq!(
            (p.scheme.as_str(), p.series.as_str(), p.order[3]),
            ("nightly", "pocket", 42)
        );
        // a build the custom scheme does not match falls through to the built-ins
        assert_eq!(
            classify(&s, &subject("x.Y", "Y", "1.0.0", "other"))
                .unwrap()
                .scheme,
            "SemVer (0.6.0)"
        );
    }

    #[test]
    fn matching_is_case_blind_on_text_and_never_guesses_across_digits() {
        let s = Scheme {
            name: "t".into(),
            field: Field::Shortname,
            template: "{series} dev {n}".into(),
        };
        assert_eq!(
            place(&s, &subject("", "TAU DEV 12", "", ""), "x")
                .unwrap()
                .order[3],
            12
        );
        assert!(place(&s, &subject("", "TAU 12", "", ""), "x").is_none());
        assert!(place(&s, &subject("", "TAU DEV 12b", "", ""), "x").is_none());
        let rc = Scheme {
            name: "r".into(),
            field: Field::Version,
            template: "{major}.{minor}rc{n}".into(),
        };
        let p = place(&rc, &subject("", "", "1.20rc3", ""), "x").unwrap();
        assert_eq!((p.order[0], p.order[1], p.order[3]), (1, 20, 3));
    }

    #[test]
    fn bad_templates_are_refused_with_a_reason() {
        assert!(validate("{series} {n}").is_ok());
        assert!(validate("").unwrap_err().contains("empty"));
        assert!(
            validate("{nope} {n}")
                .unwrap_err()
                .contains("not a known token")
        );
        assert!(validate("{series}{n}").unwrap_err().contains("touch"));
        assert!(validate("{n}-{n}").unwrap_err().contains("twice"));
        assert!(validate("{series} build").unwrap_err().contains("orders"));
        assert!(validate("{series").unwrap_err().contains("closing"));
        assert!(validate("a } b").unwrap_err().contains("opening"));
        // an unparsable custom scheme is dropped rather than matching anything
        let bad = Scheme {
            name: "bad".into(),
            field: Field::File,
            template: "{n".into(),
        };
        assert_eq!(with_custom(vec![bad]).len(), builtin().len());
    }

    #[test]
    fn a_long_hostile_name_cannot_make_matching_run_away() {
        let s = Scheme {
            name: "t".into(),
            field: Field::File,
            template: "{any}-{any}-{any}-{series} {n}".into(),
        };
        let long = "a-".repeat(400);
        let t = std::time::Instant::now();
        let _ = place(&s, &subject("", "", "", &long), "x");
        assert!(t.elapsed().as_secs() < 5);
    }

    #[test]
    fn the_next_name_counts_on_keeping_the_shape() {
        let s = builtin();
        assert_eq!(
            next_name(&s, Field::Shortname, "TAU DEV 112").unwrap().1,
            "TAU DEV 113"
        );
        assert_eq!(
            next_name(&s, Field::Shortname, "Pocket 0099").unwrap().1,
            "Pocket 0100",
            "leading zeros stay"
        );
        assert_eq!(
            next_name(&s, Field::Version, "0.6.0-alpha.3").unwrap().1,
            "0.6.0-alpha.4"
        );
        assert_eq!(
            next_name(&s, Field::Version, "0.6.0-rc9").unwrap().1,
            "0.6.0-rc10"
        );
        assert_eq!(next_name(&s, Field::Version, "0.6.0").unwrap().1, "0.6.1");
        assert!(next_name(&s, Field::Version, "weird").is_none());
        assert!(
            next_name(&s, Field::File, "TAU DEV 1").is_none(),
            "no scheme reads the file name"
        );
        let skip = Scheme {
            name: "a".into(),
            field: Field::Shortname,
            template: "{any}-{n}".into(),
        };
        assert!(
            next_text(&skip, "x-4").is_none(),
            "an ignored part cannot be rebuilt"
        );
        let nightly = Scheme {
            name: "n".into(),
            field: Field::File,
            template: "{series}-nightly-{n}".into(),
        };
        assert_eq!(
            next_text(&nightly, "Pocket-nightly-0042").unwrap(),
            "Pocket-nightly-0043"
        );
    }
}
