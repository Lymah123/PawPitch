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

/// problems: reasons to reject the draft. warnings: things the human should double-check.
pub struct Report {
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
}

/// Claims a beginner must not be made to sound like they have, unless their own
/// profile (or the brand text) really says so.
const CLAIM_TERMS: &[&str] = &[
    "followers", "views", "engagement", "viral", "partnered", "collaborated", "worked with",
    "brand deals", "campaigns", "sales",
];

/// Equipment and experience claims. Fine if the profile lists them, rejected if invented.
const EQUIPMENT_TERMS: &[&str] = &[
    "professional", "studio", "camera", "dslr", "tripod", "lighting", "microphone", "gimbal",
    "drone", "software", "experienced", "years",
];

/// Family roles and identity claims the model likes to add. Allowed only if the profile says so.
const ROLE_TERMS: &[&str] = &["mom", "mama", "mommy", "dad", "daddy", "owner", "bestie", "furbaby"];

/// Soft personality words. Not rejected, but listed for a human to check.
const TRAIT_TERMS: &[&str] = &[
    "adventure", "adventures", "adventurous", "playful", "energetic", "loving", "sweet", "friendly",
    "adorable", "cuddly", "mischievous", "goofy", "silly", "fluffy", "lovable", "funny", "smart",
    "clever", "loyal",
];

/// Neutral pitch vocabulary that is not worth flagging as "not in your facts".
const COMMON_WORDS: &[&str] = &[
    "creating", "create", "created", "excited", "especially", "really", "always", "looking",
    "showing", "sharing", "partner", "partnering", "opportunity", "together", "interested",
    "reaching", "brand", "brands", "message", "should", "because", "through", "definitely",
    "absolutely", "wonderful", "working", "collaborate", "collaboration", "feedback", "regards",
    "sincerely", "thanks", "hello", "videos", "content", "treats", "products", "hoping",
];

/// The pet's gender is only used if the profile states it.
const PRONOUNS: &[&str] = &["she", "her", "hers", "he", "his", "him", "herself", "himself"];
const GENDER_EVIDENCE: &[&str] = &[
    "she", "her", "hers", "he", "his", "him", "female", "male", "girl", "boy", "daughter", "son",
];

/// "Biscuit loves <Brand>" says something already happened. A proposal does not.
const EXPERIENCE_VERBS: &[&str] = &[
    "loves", "loved", "enjoys", "enjoyed", "enjoying", "tried", "tested", "reviewed", "recommends",
    "recommended", "obsessed", "favourite", "favorite", "addicted", "devours", "devoured",
    "gobbles", "gobbled", "trying",
];
const PROPOSAL_MARKERS: &[&str] = &[
    "would", "could", "can", "love", "like", "want", "hope", "looking", "plan", "happy", "excited",
    "idea", "ready", "if",
];

/// Content formats must be exactly the ones the profile lists ("unboxing photos" is not "unboxing videos").
const MEDIA_NOUNS: &[&str] = &[
    "videos", "video", "photos", "photo", "reels", "reel", "content", "images", "image",
];
const ALLOWED_BEFORE_MEDIA: &[&str] = &[
    "to", "and", "or", "for", "with", "in", "on", "of", "the", "a", "an", "my", "your", "our",
    "their", "these", "those", "some", "many", "more", "new", "all", "any", "own", "i", "we",
    "you", "it", "that", "which", "using", "by", "from", "are", "is", "can", "will", "would",
    "could", "short", "fun", "honest", "natural", "authentic", "creative", "real", "relatable",
    "great", "cute", "engaging", "original", "quality", "best", "good", "simple", "quick", "make",
    "makes", "making", "create", "creates", "creating", "share",
];

/// Lowercase, collapse whitespace, trim quotes and sentence punctuation at the ends.
pub fn norm(s: &str) -> String {
    let joined = s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    joined
        .trim_matches(|c: char| {
            c == '"' || c == '\'' || c == '.' || c == ',' || c == ';' || c == '\u{201c}' || c == '\u{201d}'
        })
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

pub fn word_list(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// " word word word " so whole words and phrases can be matched safely
/// ("reviews" must not match "views").
fn padded_words(text: &str) -> String {
    format!(" {} ", word_list(text).join(" "))
}

fn sentences(text: &str) -> Vec<String> {
    text.split(|c| c == '.' || c == '!' || c == '?' || c == '\n')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn pet_name(profile_text: &str) -> Option<String> {
    profile_text.lines().find_map(|l| {
        if l.to_lowercase().starts_with("pet name:") {
            l.split_once(':').map(|(_, v)| v.trim().to_string())
        } else {
            None
        }
    })
}

/// First distinctive word of the brand name ("Wagwell Treats" -> "wagwell").
fn brand_token(brand_name: &str) -> Option<String> {
    word_list(brand_name)
        .into_iter()
        .find(|w| w.len() >= 3 && w != "the" && w != "and")
}

/// Does a cited fact really show up in the message? Every meaningful word of the fact must be
/// there (singular and plural count as the same word), not just one shared word.
pub fn fact_in_text(draft_text: &str, fact: &str) -> bool {
    let value = match fact.split_once(':') {
        Some((_, v)) => v,
        None => fact,
    };
    let stem = |w: &str| w.trim_end_matches('s').to_string();
    let draft_stems: Vec<String> = word_list(draft_text).iter().map(|w| stem(w)).collect();
    let needed: Vec<String> = word_list(value)
        .iter()
        .filter(|w| w.len() >= 4)
        .map(|w| stem(w))
        .collect();
    !needed.is_empty() && needed.iter().all(|w| draft_stems.contains(w))
}

/// Words in the message that appear nowhere in the facts or the brand text.
/// Invented details hide here, so a human should look at them.
fn novel_words(draft_text: &str, sources: &str) -> Vec<String> {
    let src: HashSet<String> = word_list(sources).into_iter().collect();
    let mut out: Vec<String> = Vec::new();
    for w in word_list(draft_text) {
        if w.len() >= 6 && !src.contains(&w) && !COMMON_WORDS.contains(&w.as_str()) && !out.contains(&w) {
            out.push(w);
        }
    }
    out.truncate(12);
    out
}

/// Everything that makes a draft unsafe to send (problems), plus things to double-check (warnings).
pub fn validate(
    d: &Draft,
    profile_text: &str,
    brand_text: &str,
    brand_name: &str,
    max_words: usize,
) -> Report {
    let mut problems = Vec::new();
    let mut warnings = Vec::new();
    let b = norm(brand_text);
    let profile_words_vec = word_list(profile_text);

    if d.body.trim().is_empty() {
        problems.push("the body is empty".to_string());
    }
    if d.profile_facts_used.is_empty() {
        problems.push("it cites no facts from your profile, so it is generic".to_string());
    }
    if d.brand_facts_used.is_empty() {
        problems.push("it cites nothing from the brand text, so it is not tailored".to_string());
    }
    // A 4B model paraphrases instead of copying, so profile citations are advisory:
    // a citation that shares no words with the profile at all is worth a warning.
    for f in &d.profile_facts_used {
        let value = match f.split_once(':') {
            Some((_, v)) => v.to_string(),
            None => f.clone(),
        };
        let overlaps = word_list(&value)
            .iter()
            .filter(|w| w.len() >= 4)
            .any(|w| profile_words_vec.contains(w));
        if !overlaps {
            warnings.push(format!("it cites something not in your profile: \"{}\"", f));
        }
    }
    // Brand sentences are easy to copy, so these must match word for word.
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
            problems.push(format!("it mentions \"{}\", which is not in your profile or the brand text", term));
        }
    }
    for term in EQUIPMENT_TERMS {
        let t = format!(" {} ", term);
        if draft_words.contains(&t) && !src_words.contains(&t) {
            problems.push(format!(
                "it mentions \"{}\" (equipment or experience), which your profile does not list",
                term
            ));
        }
    }
    for term in ROLE_TERMS {
        let t = format!(" {} ", term);
        if draft_words.contains(&t) && !src_words.contains(&t) {
            problems.push(format!(
                "it calls you or the pet \"{}\", which your profile does not say",
                term
            ));
        }
    }
    for term in TRAIT_TERMS {
        let t = format!(" {} ", term);
        if draft_words.contains(&t) && !src_words.contains(&t) {
            warnings.push(format!(
                "it says \"{}\", which your profile does not say about the pet or you; keep it only if it is true",
                term
            ));
        }
    }

    // The pet's gender must come from the profile, not from the model.
    let profile_words = padded_words(profile_text);
    let has_gender_evidence = GENDER_EVIDENCE
        .iter()
        .any(|g| profile_words.contains(&format!(" {} ", g)));
    if !has_gender_evidence {
        let used: Vec<&str> = PRONOUNS
            .iter()
            .copied()
            .filter(|w| draft_words.contains(&format!(" {} ", w)))
            .collect();
        if !used.is_empty() {
            problems.push(format!(
                "it uses \"{}\" for the pet, but the profile never says the pet's gender; use the pet's name",
                used.join("/")
            ));
        }
    }

    // A content format must be one the profile (or the brand text) actually names.
    let words = word_list(&draft_text);
    let mut format_problems: Vec<String> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        if i == 0 || !MEDIA_NOUNS.contains(&w.as_str()) {
            continue;
        }
        let prev = words[i - 1].as_str();
        if ALLOWED_BEFORE_MEDIA.contains(&prev) {
            continue;
        }
        // "slice-of-life video" and "slice-of-life videos" are the same format.
        let singular = w.trim_end_matches('s').to_string();
        let plural = format!("{}s", singular);
        let found = [w.clone(), singular, plural]
            .iter()
            .any(|c| src_words.contains(&format!(" {} {} ", prev, c)));
        if !found {
            format_problems.push(format!("{} {}", prev, w));
        }
    }
    format_problems.sort();
    format_problems.dedup();
    for fp in format_problems {
        problems.push(format!(
            "\"{}\" is not a content format your profile or the brand text names",
            fp
        ));
    }

    // "Biscuit loves <Brand>" claims something already happened. A proposal ("I'd love to...") is fine.
    let mut pet_tokens: Vec<String> = pet_name(profile_text)
        .map(|n| word_list(&n))
        .unwrap_or_default();
    pet_tokens.extend(PRONOUNS.iter().map(|s| s.to_string()));
    let brand_tok = brand_token(brand_name);
    for s in sentences(&d.body) {
        let w = word_list(&s);
        let mentions_pet = w.iter().any(|x| pet_tokens.contains(x));
        let mentions_brand = w.iter().any(|x| x == "your" || Some(x.clone()) == brand_tok);
        let has_verb = w.iter().any(|x| EXPERIENCE_VERBS.contains(&x.as_str()));
        let proposal = w.iter().any(|x| PROPOSAL_MARKERS.contains(&x.as_str()));
        if mentions_pet && mentions_brand && has_verb && !proposal {
            problems.push(format!(
                "this sentence claims an experience with the brand that your profile does not document: \"{}\"",
                s
            ));
        }
    }

    let wc = d.body.split_whitespace().count();
    if wc > max_words {
        problems.push(format!("the body is {} words, over the {} word limit", wc, max_words));
    }

    let novel = novel_words(&draft_text, &sources);
    if !novel.is_empty() {
        warnings.push(format!(
            "words that are not in your profile or the brand text, so check each one is true: {}",
            novel.join(", ")
        ));
    }
    Report { problems, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "pet name: Biscuit\npet breed: beagle\ncontent: product demo videos\ncontent: slice-of-life videos\ncontent: unboxing photos\nequipment: smartphone\nequipment: natural light\ntagline: Nose first, always.";
    const BRAND: &str = "Wagwell Treats makes small-batch dog treats from real ingredients. We are looking for pet parents who share honest videos of their dog trying our 3 flavours.";
    const BRAND_NAME: &str = "Wagwell Treats";

    fn draft(body: &str) -> Draft {
        Draft {
            subject: "Hello from Biscuit".to_string(),
            body: body.to_string(),
            profile_facts_used: vec!["product demo videos".to_string()],
            brand_facts_used: vec!["honest videos".to_string()],
        }
    }

    fn report(body: &str) -> Report {
        validate(&draft(body), PROFILE, BRAND, BRAND_NAME, 140)
    }

    fn check(body: &str) -> Vec<String> {
        report(body).problems
    }

    fn has(problems: &[String], needle: &str) -> bool {
        problems.iter().any(|m| m.contains(needle))
    }

    #[test]
    fn grounded_draft_passes() {
        let p = check("Hi! I make product demo videos with my beagle Biscuit. I liked your honest videos approach.");
        assert!(p.is_empty(), "{:?}", p);
    }

    #[test]
    fn a_citation_unrelated_to_the_profile_is_a_warning() {
        let mut d = draft("Hi!");
        d.profile_facts_used = vec!["50k followers".to_string()];
        let r = validate(&d, PROFILE, BRAND, BRAND_NAME, 140);
        assert!(has(&r.warnings, "cites something not in your profile"));
    }

    #[test]
    fn a_paraphrased_true_fact_is_not_rejected() {
        let mut d = draft("Hi! Biscuit is a curious beagle.");
        d.profile_facts_used = vec!["Biscuit the beagle".to_string(), "pet breed: beagle".to_string()];
        let r = validate(&d, PROFILE, BRAND, BRAND_NAME, 140);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert!(r.warnings.iter().all(|m| !m.contains("cites something")));
    }

    #[test]
    fn invented_number_is_caught() {
        assert!(has(&check("I can deliver 12 videos in a week."), "number 12"));
    }

    #[test]
    fn number_from_the_brand_text_is_allowed() {
        let p = check("I would love to try all 3 flavours on video.");
        assert!(p.is_empty(), "{:?}", p);
    }

    #[test]
    fn claims_are_caught_but_reviews_is_not_views() {
        assert!(check("I write honest reviews.").is_empty());
        assert!(has(&check("My videos get great views."), "views"));
    }

    #[test]
    fn generic_draft_is_rejected() {
        let d = Draft {
            subject: "Hi".to_string(),
            body: "Hi, I am a creator.".to_string(),
            profile_facts_used: vec![],
            brand_facts_used: vec![],
        };
        assert_eq!(validate(&d, PROFILE, BRAND, BRAND_NAME, 140).problems.len(), 2);
    }

    #[test]
    fn too_long_is_caught() {
        assert!(has(&check(&"word ".repeat(200)), "word limit"));
    }

    // ---- regression tests for what the real runs let through ----

    #[test]
    fn pet_experience_with_the_brand_is_rejected() {
        assert!(has(&check("Biscuit loves Wagwell Treats."), "claims an experience"));
        assert!(has(&check("We make videos to show how much Biscuit loves Wagwell Treats."), "claims an experience"));
    }

    #[test]
    fn a_proposal_is_allowed() {
        let p = check("I'd love to create a video trying your treats.");
        assert!(p.is_empty(), "{:?}", p);
    }

    #[test]
    fn singular_and_plural_of_a_listed_format_are_the_same_format() {
        let p = check("I would love to create a slice-of-life video and an unboxing photo.");
        assert!(p.is_empty(), "{:?}", p);
        // but a format the profile only lists as photos is still not a video format
        assert!(has(&check("I would love to create an unboxing video."), "unboxing video"));
    }

    #[test]
    fn the_draft_rejected_for_singular_formats_now_passes() {
        let body = "Hi, I\u{2019}m Alex and this is Biscuit, our curious beagle! I really appreciate how Wagwell Treats uses real ingredients \u{2013} it\u{2019}s fantastic to see dogs enjoying them. I would love to create a short slice-of-life video of Biscuit excitedly trying a crunchy peanut butter bite. I would also love to create unboxing photos showing Biscuit\u{2019}s reaction to a new bag of treats. Nose first, always. Let\u{2019}s chat!";
        let p = check(body);
        assert!(p.is_empty(), "{:?}", p);
    }

    #[test]
    fn unlisted_content_format_is_rejected() {
        assert!(has(&check("I make unboxing videos."), "unboxing videos"));
    }

    #[test]
    fn listed_content_formats_are_allowed() {
        let p = check("I create slice-of-life videos and unboxing photos.");
        assert!(p.is_empty(), "{:?}", p);
    }

    #[test]
    fn unlisted_equipment_is_rejected() {
        let p = check("I use professional lighting.");
        assert!(has(&p, "professional") && has(&p, "lighting"));
    }

    #[test]
    fn listed_equipment_is_allowed() {
        let p = check("I shoot with my smartphone in natural light.");
        assert!(p.is_empty(), "{:?}", p);
    }

    #[test]
    fn invented_pet_gender_is_rejected() {
        assert!(has(&check("She is super curious."), "never says the pet's gender"));
        assert!(check("Biscuit is super curious.").iter().all(|m| !m.contains("gender")));
    }

    #[test]
    fn pet_gender_is_allowed_when_the_profile_states_it() {
        let profile = format!("{}\npet gender: female", PROFILE);
        let r = validate(&draft("She makes product demo videos."), &profile, BRAND, BRAND_NAME, 140);
        assert!(!has(&r.problems, "gender"), "{:?}", r.problems);
    }

    #[test]
    fn family_roles_are_rejected_and_traits_are_only_warnings() {
        let r = report("Biscuit is always ready for a new adventure.");
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert!(has(&r.warnings, "adventure"), "{:?}", r.warnings);
        assert!(has(&check("I am a beagle mom."), "mom"));
        assert!(check("Biscuit is curious.").is_empty());
    }

    #[test]
    fn unknown_words_are_listed_for_a_human_to_check() {
        let r = report("Biscuit loves exploring the neighbourhood.");
        assert!(has(&r.warnings, "exploring"), "{:?}", r.warnings);
    }

    #[test]
    fn the_first_real_draft_that_slipped_through_is_now_rejected() {
        let body = "Hey there! I'm Alex, and I create fun videos of my beagle, Biscuit. She\u{2019}s super curious and *really* loves treats \u{2013} especially Wagwell Treats! I love making slice-of-life and unboxing videos using just my smartphone and natural light. I\u{2019}m looking to share honest, natural-looking videos of Biscuit enjoying your treats in real homes. Nose first, always! Let\u{2019}s chat.";
        let p = check(body);
        assert!(has(&p, "never says the pet's gender"), "{:?}", p);
        assert!(has(&p, "unboxing videos"), "{:?}", p);
    }

    #[test]
    fn the_second_real_draft_that_passed_is_now_rejected() {
        let body = "Hi there! I\u{2019}m Alex, and I\u{2019}m a beagle mom \u{2013} Biscuit is incredibly curious and always ready for a new adventure. I\u{2019}d love to create product demo videos showing Biscuit enjoying Wagwell Treats! We both love the idea of sharing slice-of-life videos of Biscuit exploring. I'm especially keen to capture some unboxing photos of the treats. Nose first, always!";
        let p = check(body);
        assert!(has(&p, "mom"), "{:?}", p);
        assert!(has(&report(body).warnings, "adventure"));
    }

    #[test]
    fn the_decent_draft_that_was_wrongly_rejected_now_passes_with_warnings() {
        let mut d = draft("Hi there! I\u{2019}m Alex, and I\u{2019}m always looking for fun ways to share Biscuit\u{2019}s adventures. I\u{2019}d love to create some product demo videos showing Biscuit enjoying Wagwell Treats. Biscuit is a curious beagle, and especially food-motivated! I think slice-of-life videos of Biscuit would be great. I\u{2019}m excited about the prospect of sharing unboxing photos too. Nose first, always!");
        d.subject = "Biscuit x Wagwell Treats: a UGC idea".to_string();
        d.brand_facts_used = vec!["Wagwell Treats makes small-batch dog treats from real ingredients.".to_string()];
        let r = validate(&d, PROFILE, BRAND, BRAND_NAME, 140);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert!(has(&r.warnings, "adventures"), "{:?}", r.warnings);
    }

    #[test]
    fn the_pet_name_keeps_its_capitalisation() {
        assert_eq!(pet_name(PROFILE), Some("Biscuit".to_string()));
        assert_eq!(pet_name("pet breed: beagle"), None);
    }

    #[test]
    fn a_fact_with_only_one_shared_word_is_not_counted_as_used() {
        assert!(!fact_in_text(
            "Biscuit loves a Wagwell Treat",
            "pet personality: dramatic when the treat jar is empty"
        ));
        assert!(fact_in_text("I would love a slice-of-life video", "content: slice-of-life videos"));
        assert!(fact_in_text("our curious beagle", "pet personality: curious"));
    }

    #[test]
    fn cited_facts_not_visible_in_the_message_are_detected() {
        assert!(fact_in_text("I make product demo videos", "content: product demo videos"));
        assert!(!fact_in_text("I make product demo videos", "platforms: TikTok"));
    }
}