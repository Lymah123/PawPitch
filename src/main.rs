mod ollama;
mod profile;
mod tracker;
mod validate;

use chrono::{Local, NaiveDate};
use std::fs;
use std::process::exit;
use tracker::Entry;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("pitch") => cmd_pitch(&args[1..]),
        Some("check") => cmd_check(&args[1..]),
        Some("log") => cmd_log(&args[1..]),
        Some("status") => cmd_status(&args[1..]),
        Some("followed-up") => cmd_followed_up(&args[1..]),
        Some("due") => cmd_due(&args[1..]),
        Some("list") => cmd_list(),
        _ => {
            print_usage();
            return;
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn print_usage() {
    println!("pawpitch: fact-checked outreach drafts and a follow-up tracker for new UGC pet creators\n");
    println!("  pitch --brand \"<name>\" --brand-file <file> [--profile data/profile.json] [--channel email|dm] [--tries 3] [--seed 42]");
    println!("  check --brand \"<name>\" --brand-file <file> --draft-file <file> [--profile data/profile.json]");
    println!("      fact-check any pitch (written by you, by another chatbot, anywhere)");
    println!("  log --brand <name> [--channel email|dm] [--date YYYY-MM-DD] [--note <text>]");
    println!("  status --brand <name> --set sent|replied|declined|booked|no-reply");
    println!("  followed-up --brand <name>");
    println!("  due [--days 7]");
    println!("  list");
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn today() -> NaiveDate {
    Local::now().date_naive()
}

// ---------- pitch ----------

fn cmd_pitch(args: &[String]) -> Result<(), String> {
    let brand_name = flag(args, "--brand")
        .ok_or("--brand is required, e.g. --brand \"Wagwell Treats\"")?;
    let brand_path = flag(args, "--brand-file")
        .ok_or("--brand-file is required, e.g. --brand-file data/brands/acme.txt")?;
    let profile_path = flag(args, "--profile").unwrap_or_else(|| "data/profile.json".to_string());
    let channel = flag(args, "--channel").unwrap_or_else(|| "email".to_string());
    if channel != "email" && channel != "dm" {
        return Err("--channel must be email or dm".to_string());
    }
    let base_seed: i64 = match flag(args, "--seed") {
        Some(v) => v.parse().map_err(|_| "--seed must be a whole number".to_string())?,
        None => 42,
    };
    let tries: i64 = match flag(args, "--tries") {
        Some(v) => v.parse().map_err(|_| "--tries must be a whole number".to_string())?,
        None => 3,
    };

    let profile_raw = fs::read_to_string(&profile_path)
        .map_err(|e| format!("cannot read {profile_path}: {e}"))?;
    let profile_json: serde_json::Value = serde_json::from_str(&profile_raw)
        .map_err(|e| format!("{profile_path} is not valid JSON: {e}"))?;
    let profile_text = profile::flatten(&profile_json);
    if profile_text.trim().is_empty() {
        return Err("your profile has no facts in it yet".to_string());
    }
    let brand_text = fs::read_to_string(&brand_path)
        .map_err(|e| format!("cannot read {brand_path}: {e}"))?;
    if brand_text.trim().len() < 40 {
        return Err("the brand text is very short; paste the brand's description or the gig post".to_string());
    }

    let max_words = if channel == "email" { 140 } else { 60 };
    let prompt = ollama::build_prompt(&profile_text, &brand_text, &channel, max_words);

    let pet = validate::pet_name(&profile_text);

    // Each attempt uses a different seed, so a rejected draft is simply tried again.
    let mut last: Option<(validate::Draft, Vec<String>)> = None;
    for attempt in 0..tries.max(1) {
        let seed = base_seed + attempt;
        println!("Asking Gemma (attempt {}/{}, seed {seed}; slow on CPU)...", attempt + 1, tries.max(1));
        let mut draft = match ollama::draft(&prompt, seed) {
            Ok(d) => d,
            Err(e) => {
                println!("  this attempt failed: {e}");
                continue;
            }
        };
        // The subject line is written by the code, not the model.
        draft.subject = if channel == "email" {
            match &pet {
                Some(p) => format!("{} x {}: a UGC idea", p, brand_name),
                None => format!("A UGC idea for {}", brand_name),
            }
        } else {
            String::new()
        };
        let report = validate::validate(&draft, &profile_text, &brand_text, &brand_name, max_words);
        if report.problems.is_empty() {
            show_draft(&draft, &report, &channel);
            return Ok(());
        }
        println!("  rejected:");
        for p in &report.problems {
            println!("    - {p}");
        }
        last = Some((draft, report.problems));
    }

    match last {
        Some((draft, problems)) => {
            println!("\nREJECTED after {} attempt(s). Do not send any of these. Problems with the last one:", tries.max(1));
            for p in &problems {
                println!("  - {p}");
            }
            println!("\nWhat Gemma wrote, for debugging only:");
            println!("Subject: {}\n\n{}", draft.subject, draft.body);
            Err("no draft passed the fact check; try --tries 5 or add more facts to your profile".to_string())
        }
        None => Err("Gemma did not return a usable draft".to_string()),
    }
}

fn show_draft(draft: &validate::Draft, report: &validate::Report, channel: &str) {
    println!("\n=== DRAFT ({channel}) ===");
    if channel == "email" {
        println!("Subject: {}\n", draft.subject);
    }
    println!("{}\n", draft.body.trim());
    println!("--- passed the fact check (it does not prove every word is true)");
    let shown: Vec<&String> = draft
        .profile_facts_used
        .iter()
        .filter(|f| validate::fact_in_text(&draft.body, f))
        .collect();
    println!(
        "profile facts in the message: {}",
        shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" | ")
    );
    println!("brand sentences the model cited: {}", draft.brand_facts_used.join(" | "));
    if !report.warnings.is_empty() {
        println!("\nDouble-check before sending:");
        for w in &report.warnings {
            println!("  - {w}");
        }
    }
    println!("\nRead it, change anything that does not sound like you, and send it yourself.");
    println!("Then log it: cargo run -- log --brand \"<name>\" --channel {channel}");
}

// ---------- check ----------

fn load_profile_text(profile_path: &str) -> Result<String, String> {
    let raw = fs::read_to_string(profile_path)
        .map_err(|e| format!("cannot read {profile_path}: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("{profile_path} is not valid JSON: {e}"))?;
    let text = profile::flatten(&json);
    if text.trim().is_empty() {
        return Err("your profile has no facts in it yet".to_string());
    }
    Ok(text)
}

/// Fact-check a pitch that PawPitch did not write: one from you, from another chatbot, from anywhere.
fn cmd_check(args: &[String]) -> Result<(), String> {
    let brand_name = flag(args, "--brand").ok_or("--brand is required, e.g. --brand \"Wagwell Treats\"")?;
    let brand_path = flag(args, "--brand-file").ok_or("--brand-file is required")?;
    let draft_path = flag(args, "--draft-file")
        .ok_or("--draft-file is required, e.g. --draft-file examples/bad-draft.txt")?;
    let profile_path = flag(args, "--profile").unwrap_or_else(|| "data/profile.json".to_string());

    let profile_text = load_profile_text(&profile_path)?;
    let brand_text = fs::read_to_string(&brand_path)
        .map_err(|e| format!("cannot read {brand_path}: {e}"))?;
    let body = fs::read_to_string(&draft_path)
        .map_err(|e| format!("cannot read {draft_path}: {e}"))?;

    let draft = validate::Draft {
        subject: String::new(),
        body,
        profile_facts_used: vec![],
        brand_facts_used: vec![],
    };
    let report = validate::validate(&draft, &profile_text, &brand_text, &brand_name, 200);
    // A pitch written elsewhere cannot cite its sources, so only the content checks apply.
    let problems: Vec<&String> = report.problems.iter().filter(|p| !p.starts_with("it cites")).collect();

    if problems.is_empty() {
        println!("PASSED the fact check (it does not prove every word is true).");
    } else {
        println!("REJECTED. {} problem(s):", problems.len());
        for p in problems {
            println!("  - {p}");
        }
    }
    if !report.warnings.is_empty() {
        println!("\nDouble-check before sending:");
        for w in &report.warnings {
            println!("  - {w}");
        }
    }
    Ok(())
}

// ---------- tracker ----------

fn cmd_log(args: &[String]) -> Result<(), String> {
    let brand = flag(args, "--brand").ok_or("--brand is required")?;
    let channel = flag(args, "--channel").unwrap_or_else(|| "email".to_string());
    let sent_on = match flag(args, "--date") {
        Some(d) => NaiveDate::parse_from_str(&d, "%Y-%m-%d")
            .map_err(|_| "dates look like 2026-10-16".to_string())?,
        None => today(),
    };
    let note = flag(args, "--note").unwrap_or_default();
    let mut entries = tracker::load()?;
    entries.push(Entry {
        brand: brand.clone(),
        channel,
        sent_on,
        status: "sent".to_string(),
        followed_up_on: None,
        note,
    });
    tracker::save(&entries)?;
    println!(
        "Logged {brand}. A follow-up will be due on {}.",
        sent_on + chrono::Duration::days(7)
    );
    Ok(())
}

fn cmd_status(args: &[String]) -> Result<(), String> {
    let brand = flag(args, "--brand").ok_or("--brand is required")?;
    let new_status = flag(args, "--set").ok_or("--set is required, e.g. --set replied")?;
    if !tracker::STATUSES.contains(&new_status.as_str()) {
        return Err(format!("status must be one of: {}", tracker::STATUSES.join(", ")));
    }
    let mut entries = tracker::load()?;
    let i = tracker::find(&entries, &brand).ok_or_else(|| format!("no outreach logged for {brand}"))?;
    entries[i].status = new_status.clone();
    tracker::save(&entries)?;
    println!("{} is now: {new_status}", entries[i].brand);
    Ok(())
}

fn cmd_followed_up(args: &[String]) -> Result<(), String> {
    let brand = flag(args, "--brand").ok_or("--brand is required")?;
    let mut entries = tracker::load()?;
    let i = tracker::find(&entries, &brand).ok_or_else(|| format!("no outreach logged for {brand}"))?;
    entries[i].followed_up_on = Some(today());
    tracker::save(&entries)?;
    println!("Recorded a follow-up to {} today.", entries[i].brand);
    Ok(())
}

fn cmd_due(args: &[String]) -> Result<(), String> {
    let days: i64 = match flag(args, "--days") {
        Some(d) => d.parse().map_err(|_| "--days must be a whole number".to_string())?,
        None => 7,
    };
    let entries = tracker::load()?;
    let due = tracker::due(&entries, today(), days);
    if due.is_empty() {
        println!("Nothing is due for a follow-up.");
    } else {
        println!("Due for a follow-up (no reply after {days}+ days):");
        for e in due {
            println!("  {:<28} {:<6} sent {}", e.brand, e.channel, e.sent_on);
        }
    }
    Ok(())
}

fn cmd_list() -> Result<(), String> {
    let entries = tracker::load()?;
    if entries.is_empty() {
        println!("No outreach logged yet.");
        return Ok(());
    }
    for e in &entries {
        let followed = match e.followed_up_on {
            Some(d) => format!("followed up {d}"),
            None => String::new(),
        };
        println!(
            "{:<28} {:<6} sent {}  {:<9} {}",
            e.brand, e.channel, e.sent_on, e.status, followed
        );
    }
    Ok(())
}