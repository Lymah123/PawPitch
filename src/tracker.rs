use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fs;

const FILE: &str = "data/outreach.json";

pub const STATUSES: [&str; 5] = ["sent", "replied", "declined", "booked", "no-reply"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    pub brand: String,
    pub channel: String,
    pub sent_on: NaiveDate,
    pub status: String,
    #[serde(default)]
    pub followed_up_on: Option<NaiveDate>,
    #[serde(default)]
    pub note: String,
}

pub fn load() -> Result<Vec<Entry>, String> {
    match fs::read_to_string(FILE) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{FILE} is not valid: {e}")),
        Err(_) => Ok(Vec::new()),
    }
}

pub fn save(entries: &[Entry]) -> Result<(), String> {
    fs::create_dir_all("data").map_err(|e| e.to_string())?;
    let t = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(FILE, t).map_err(|e| e.to_string())
}

/// Messages still waiting for a reply, with no follow-up yet, and at least `days` old.
pub fn due(entries: &[Entry], today: NaiveDate, days: i64) -> Vec<&Entry> {
    entries
        .iter()
        .filter(|e| {
            e.status == "sent"
                && e.followed_up_on.is_none()
                && (today - e.sent_on).num_days() >= days
        })
        .collect()
}

pub fn find(entries: &[Entry], brand: &str) -> Option<usize> {
    let b = brand.trim().to_lowercase();
    entries.iter().rposition(|e| e.brand.trim().to_lowercase() == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(brand: &str, sent: &str, status: &str, followed: Option<&str>) -> Entry {
        Entry {
            brand: brand.to_string(),
            channel: "email".to_string(),
            sent_on: NaiveDate::parse_from_str(sent, "%Y-%m-%d").unwrap(),
            status: status.to_string(),
            followed_up_on: followed.map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap()),
            note: String::new(),
        }
    }

    #[test]
    fn only_unanswered_old_messages_are_due() {
        let today = NaiveDate::parse_from_str("2026-10-20", "%Y-%m-%d").unwrap();
        let entries = vec![
            entry("A", "2026-10-10", "sent", None),
            entry("B", "2026-10-18", "sent", None),
            entry("C", "2026-10-01", "replied", None),
            entry("D", "2026-10-02", "sent", Some("2026-10-09")),
        ];
        let d = due(&entries, today, 7);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].brand, "A");
    }

    #[test]
    fn finds_brands_ignoring_case() {
        let entries = vec![entry("Wagwell Treats", "2026-10-10", "sent", None)];
        assert_eq!(find(&entries, " wagwell treats "), Some(0));
        assert_eq!(find(&entries, "other"), None);
    }
}
