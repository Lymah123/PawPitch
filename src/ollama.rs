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
        "You write outreach messages for a new UGC (user-generated content) pet creator. Write {style}, at most {max_words} words in the body, in a warm, natural first-person voice. \
Use ONLY the facts in CREATOR FACTS and BRAND TEXT. Never invent follower counts, views, results, past brand partnerships, prices, turnaround times or numbers. If a detail is not in the facts, leave it out. \
Reply with JSON only, in this form: {{\"subject\": string, \"body\": string, \"profile_facts_used\": [exact phrases copied from CREATOR FACTS], \"brand_facts_used\": [exact phrases copied from BRAND TEXT]}}\n\nCREATOR FACTS:\n{profile}\n\nBRAND TEXT:\n{brand}",
        style = style,
        max_words = max_words,
        profile = profile_text,
        brand = brand_text
    )
}

fn generate(prompt: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "model": MODEL,
        "prompt": prompt,
        "stream": false,
        "format": "json",
        "keep_alive": "30m",
        "options": { "temperature": 0.3, "seed": 42 }
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

pub fn draft(prompt: &str) -> Result<Draft, String> {
    let text = generate(prompt)?;
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
        assert!(p.contains("Never invent"));
    }
}
