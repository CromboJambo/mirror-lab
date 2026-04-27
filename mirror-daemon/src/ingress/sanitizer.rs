use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;

/// A module for cleaning and normalizing incoming event data.
pub struct Sanitizer {
    /// Regex patterns used to identify and remove noise from text payloads.
    noise_patterns: Vec<Regex>,
}

impl Sanitizer {
    /// Creates a new `Sanitizer` with default noise reduction patterns.
    pub fn new() -> Result<Self> {
        let noise_patterns = vec![
            // Remove repetitive "OK", "Success", or heartbeat messages (case-insensitive)
            Regex::new(r"(?i)^\s*(ok|success|heartbeat)\s*$")?,
            // Collapse multiple newlines into a single newline
            Regex::new(r"\n{3,}")?,
            // Remove leading/trailing whitespace and common log prefixes like [timestamp] or <tag>
            Regex::new(r"^[\[<].*?[\]>] \s*")?,
            // Strip excessive punctuation (e.g., "....", "!!!!")
            Regex::new(r"([!?.]){3,}")?,
        ];

        Ok(Self { noise_patterns })
    }

    /// Sanitizes a text string by applying noise reduction patterns and normalizing whitespace.
    pub fn sanitize_text(&self, text: &str) -> String {
        let mut cleaned = text.to_string();

        for pattern in &self.noise_patterns {
            cleaned = pattern.replace_all(&cleaned, "").trim().to_string();
        }

        // Final pass: collapse all whitespace sequences (including tabs/newlines) into single spaces
        // This ensures payload canonicalization.
        let whitespace_regex = Regex::new(r"\s+").unwrap();
        whitespace_regex
            .replace_all(&cleaned, " ")
            .trim()
            .to_string()
    }

    /// Normalizes a timestamp string to a standard UTC ISO 8601 format.
    #[allow(dead_code)]
    pub fn normalize_timestamp(&self, ts: &str) -> Result<DateTime<Utc>> {
        // Attempt RFC3339 first as it's the project standard.
        let dt = DateTime::parse_from_rfc3339(ts)
            .map(|dt| dt.with_timezone(&Utc))
            .context(format!("Failed to parse timestamp '{}' as RFC3339", ts))?;

        Ok(dt)
    }
}
