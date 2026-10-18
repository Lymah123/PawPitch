use crate::validate::Draft;
use std::time::Duration;

const MODEL: &str = "gemma3:4b";

/// The model drafts. It is told to cite the exact phrases it used, and the
/// code then checks those citations against the sources.
pub fn build_prompt(profile_text: &str, brand_text: &str, channel: &str, max_words: usize) -> String {
    let style = if channel == "email" {
        "a short email with a subject line"
    } else {
        "a short direct message (leave the subject empty)"
    };
    format!(
        "You write outreach messages for a new UGC (user-generated content) pet creator. Write {style}, at most {max_words} words in the body, in a warm, natural first-person voice. Use exactly four short sentences: (1) say hello and introduce yourself and your pet by name; (2) mention one thing from BRAND TEXT that you like about the brand; (3) propose one content idea in a format exactly as listed, starting with I would love to create; (4) end with the tagline if one is listed and a friendly call to action. Leave the subject field empty.\n\n\
STRICT FACT RULES:\n\
- Use only facts explicitly stated in CREATOR FACTS or BRAND TEXT.\n\
- Refer to the pet by name. Never use he, she, his or her unless the facts state the pet's gender.\n\
- Never say the pet or the creator has tried, loved, reviewed or endorsed the brand's products. Offer ideas for the future instead (for example: I would love to create...).\n\
- Name content formats exactly as listed. Do not turn photos into videos.\n\
- Do not invent personality traits, feelings, hobbies or family roles (such as mom or owner). Describe the pet only with the traits listed.\n\
- Do not mention equipment, follower counts, views, engagement, past collaborations, results, prices, turnaround times or any numbers unless they are listed.\n\
- If a useful fact is missing, leave it out instead of guessing.\n\n\
Reply with JSON only, in this form: {{\"subject\": string, \"body\": string, \"profile_facts_used\": [exact phrases copied from CREATOR FACTS that your message really uses], \"brand_facts_used\": [exact phrases copied from BRAND TEXT that your message really uses]}}\n\nCREATOR FACTS:\n{profile}\n\nBRAND TEXT:\n{brand}",
        style = style,
        max_words = max_words,
        profile = profile_text,
        brand = brand_text
    )
}

fn generate(prompt: &str, seed: i64) -> Result<String, String> {
    let body = serde_json::json!({
        "model": MODEL,
        "prompt": prompt,
        "stream": false,
        "format": "json",
        "keep_alive": "30m",
        "options": { "temperature": 0.3, "seed": seed }
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post("http://localhost:11434/api/generate")
        .json(&body)
        .send()
        .map_err(|e| format!("could not reach Ollama at localhost:11434 (is it running?): {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ollama returned status {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
    v["response"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Ollama reply had no 'response' text".to_string())
}

pub fn parse_draft(text: &str) -> Result<Draft, String> {
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<Draft>(cleaned).map_err(|e| format!("not a usable draft ({e}). Raw reply: {text}"))
}

pub fn draft(prompt: &str, seed: i64) -> Result<Draft, String> {
    let text = generate(prompt, seed)?;
    parse_draft(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_json() {
        let t = "```json\n{\"subject\": \"Hi\", \"body\": \"Hello\", \"profile_facts_used\": [\"a\"], \"brand_facts_used\": [\"b\"]}\n```";
        let d = parse_draft(t).unwrap();
        assert_eq!(d.subject, "Hi");
        assert_eq!(d.profile_facts_used, vec!["a".to_string()]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_draft("sure! here is a pitch").is_err());
    }

    #[test]
    fn prompt_contains_both_sources_and_the_rule() {
        let p = build_prompt("pet name: Biscuit", "Wagwell makes treats", "email", 140);
        assert!(p.contains("pet name: Biscuit") && p.contains("Wagwell makes treats"));
        assert!(p.contains("STRICT FACT RULES"));
    }
}