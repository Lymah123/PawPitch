use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct Draft {
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub profile_facts_used: Vec<String>,
    #[serde(default)]
    pub brand_facts_used: Vec<String>,
}

/// Claims a beginner must not be made to sound like they have, unless their own
/// profile (or the brand text) really says so.
const CLAIM_TERMS: [&str; 10] = [
    "followers",
    "views",
    "engagement",
    "viral",
    "partnered",
    "collaborated",
    "worked with",
    "brand deals",
    "campaigns",
    "sales",
];

/// Lowercase, collapse whitespace, trim quotes and sentence punctuation at the ends.
pub fn norm(s: &str) -> String {
    let joined = s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    joined
        .trim_matches(|c: char| c == '"' || c == '\'' || c == '.' || c == ',' || c == ';' || c == '\u{201c}' || c == '\u{201d}')
        .to_string()
}

fn contains_verbatim(haystack_norm: &str, phrase: &str) -> bool {
    let p = norm(phrase);
    !p.is_empty() && haystack_norm.contains(&p)
}

/// Number-like tokens ("500", "2.5", "10,000") in a text.
fn numbers_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let t = cur.trim_end_matches(|c| c == ',' || c == '.').to_string();
        if !t.is_empty() {
            out.push(t);
        }
        cur.clear();
    };
    for c in text.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if (c == ',' || c == '.') && !cur.is_empty() {
            cur.push(c);
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// " word word word " so whole words and phrases can be matched safely
/// ("reviews" must not match "views").
fn padded_words(text: &str) -> String {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect();
    format!(" {} ", words.join(" "))
}

/// Everything that makes a draft unsafe to send. An empty list means it passed.
pub fn validate(d: &Draft, profile_text: &str, brand_text: &str, max_words: usize) -> Vec<String> {
    let mut problems = Vec::new();
    let p = norm(profile_text);
    let b = norm(brand_text);

    if d.body.trim().is_empty() {
        problems.push("the body is empty".to_string());
    }
    if d.profile_facts_used.is_empty() {
        problems.push("it cites no facts from your profile, so it is generic".to_string());
    }
    if d.brand_facts_used.is_empty() {
        problems.push("it cites nothing from the brand text, so it is not tailored".to_string());
    }
    for f in &d.profile_facts_used {
        if !contains_verbatim(&p, f) {
            problems.push(format!("profile fact not found word for word: \"{}\"", f));
        }
    }
    for f in &d.brand_facts_used {
        if !contains_verbatim(&b, f) {
            problems.push(format!("brand fact not found word for word: \"{}\"", f));
        }
    }

    let sources = format!("{}\n{}", profile_text, brand_text);
    let known: HashSet<String> = numbers_in(&sources).into_iter().collect();
    let draft_text = format!("{} {}", d.subject, d.body);
    for n in numbers_in(&draft_text) {
        if !known.contains(&n) {
            problems.push(format!("the number {} is not in your profile or the brand text", n));
        }
    }

    let src_words = padded_words(&sources);
    let draft_words = padded_words(&draft_text);
    for term in CLAIM_TERMS {
        let t = format!(" {} ", term);
        if draft_words.contains(&t) && !src_words.contains(&t) {
            problems.push(format!(
                "it mentions \"{}\", which is not in your profile or the brand text",
                term
            ));
        }
    }

    let wc = d.body.split_whitespace().count();
    if wc > max_words {
        problems.push(format!("the body is {} words, over the {} word limit", wc, max_words));
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "pet name: Biscuit\npet breed: beagle\ncontent: product demo videos\ncontent: slice-of-life videos\ntagline: Nose first, always.";
    const BRAND: &str = "Wagwell Treats makes small-batch dog treats from real ingredients. We are looking for pet parents who share honest videos of their dog trying our 3 flavours.";

    fn draft(body: &str, pf: &[&str], bf: &[&str]) -> Draft {
        Draft {
            subject: "Hello from Biscuit".to_string(),
            body: body.to_string(),
            profile_facts_used: pf.iter().map(|s| s.to_string()).collect(),
            brand_facts_used: bf.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn grounded_draft_passes() {
        let d = draft(
            "Hi! I make product demo videos with my beagle Biscuit and loved your honest videos approach.",
            &["product demo videos", "pet breed: beagle"],
            &["honest videos"],
        );
        assert!(validate(&d, PROFILE, BRAND, 140).is_empty());
    }

    #[test]
    fn invented_fact_is_caught() {
        let d = draft("Hi!", &["50k followers"], &["honest videos"]);
        let p = validate(&d, PROFILE, BRAND, 140);
        assert!(p.iter().any(|m| m.contains("profile fact not found")));
    }

    #[test]
    fn invented_number_is_caught() {
        let d = draft("I can deliver 12 videos in a week.", &["product demo videos"], &["honest videos"]);
        let p = validate(&d, PROFILE, BRAND, 140);
        assert!(p.iter().any(|m| m.contains("number 12")));
    }

    #[test]
    fn number_from_the_brand_text_is_allowed() {
        let d = draft("I would love to try all 3 flavours on camera.", &["product demo videos"], &["3 flavours"]);
        assert!(validate(&d, PROFILE, BRAND, 140).is_empty());
    }

    #[test]
    fn claims_are_caught_but_reviews_is_not_views() {
        let ok = draft("I write honest reviews.", &["product demo videos"], &["honest videos"]);
        assert!(validate(&ok, PROFILE, BRAND, 140).is_empty());
        let bad = draft("My videos get great views.", &["product demo videos"], &["honest videos"]);
        assert!(validate(&bad, PROFILE, BRAND, 140).iter().any(|m| m.contains("views")));
    }

    #[test]
    fn generic_draft_is_rejected() {
        let d = draft("Hi, I am a creator.", &[], &[]);
        assert_eq!(validate(&d, PROFILE, BRAND, 140).len(), 2);
    }

    #[test]
    fn too_long_is_caught() {
        let long = "word ".repeat(200);
        let d = draft(&long, &["product demo videos"], &["honest videos"]);
        assert!(validate(&d, PROFILE, BRAND, 140).iter().any(|m| m.contains("word limit")));
    }
}
