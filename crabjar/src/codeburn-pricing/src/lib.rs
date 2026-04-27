use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use std::collections::BTreeMap;

#[derive(Error, Debug)]
pub enum Error {
    #[error("pricing data not found")]
    PricingNotFound,
    #[error("pricing data stale: {0}")]
    PricingStale(String),
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("json error")]
    Json(#[from] serde_json::Error),
    #[error("network error")]
    Network(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct PricingMetrics {
    pub total_cost: f64,
    pub daily: Vec<(chrono::NaiveDate, f64)>,
    pub by_project: BTreeMap<String, f64>,
    pub by_model: BTreeMap<String, f64>,
    pub by_activity: BTreeMap<String, f64>,
    pub by_tool: BTreeMap<String, f64>,
    pub by_mcp: BTreeMap<String, f64>,
    pub by_shell: BTreeMap<String, f64>,
    pub top_sessions: Vec<(String, f64)>,
    pub efficiency: f64,
    pub style: String,
}

#[derive(Debug)]
pub struct PricingEngine {
    cache_path: std::path::PathBuf,
    #[allow(dead_code)]
    pricing_data: BTreeMap<String, (f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingEntry {
    pub model: String,
    pub input_price: f64,
    pub output_price: f64,
}

impl Default for PricingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PricingEngine {
    pub fn new() -> Self {
        Self {
            cache_path: std::path::PathBuf::from("~/.cache/codeburn/"),
            pricing_data: BTreeMap::new(),
        }
    }

    pub fn built_in_aliases() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    pub async fn calculate(
        &self,
        sessions: &[codeburn_provider::SessionData],
        _currency: Option<String>,
    ) -> Result<PricingMetrics, Error> {
        let pricing = self.fetch_or_cache().await?;

        let mut total_cost = 0.0;
        let mut daily = Vec::new();
        let mut by_project = BTreeMap::new();
        let mut by_model = BTreeMap::new();
        let by_activity = BTreeMap::new();
        let by_tool = BTreeMap::new();
        let by_mcp = BTreeMap::new();
        let by_shell = BTreeMap::new();
        let top_sessions = Vec::new();

        for session in sessions {
            let entry = pricing.1.get(&session.model);
            if let Some(p) = entry {
                let cost = session.input_tokens as f64 * p.input_price
                    + session.output_tokens as f64 * p.output_price;
                total_cost += cost;
                daily.push((session.date.date(), cost));
                by_project.insert(
                    session.project.clone().unwrap_or("unknown".to_string()),
                    cost,
                );
                by_model.insert(session.model.clone(), cost);
            }
        }

        Ok(PricingMetrics {
            total_cost,
            daily,
            by_project,
            by_model,
            by_activity,
            by_tool,
            by_mcp,
            by_shell,
            top_sessions,
            efficiency: 0.0,
            style: "unknown".to_string(),
        })
    }

    async fn fetch_or_cache(
        &self,
    ) -> Result<(chrono::NaiveDateTime, BTreeMap<String, PricingEntry>), Error> {
        if self.cache_path.exists() {
            let content = std::fs::read_to_string(&self.cache_path)?;
            let data: serde_json::Value = serde_json::from_str(&content)?;

            let cache_date = data
                .get("last_updated")
                .and_then(|u| u.as_str())
                .map(|s| s.parse::<chrono::NaiveDateTime>().ok())
                .unwrap_or(None);
            let pricing_map = data
                .get("pricing")
                .and_then(|p| p.as_object())
                .map(|o| {
                    o.iter()
                        .filter(|(k, _v)| *k != "last_updated")
                        .filter_map(|(k, v)| {
                            serde_json::from_value::<PricingEntry>(v.clone())
                                .ok()
                                .map(|p| (k.clone(), p))
                        })
                        .collect::<BTreeMap<String, PricingEntry>>()
                })
                .unwrap_or_default();

            if let Some(d) = cache_date
                && d.date() < chrono::Local::now().date_naive() - chrono::Duration::days(1)
            {
                return Err(Error::PricingStale(d.to_string()));
            }

            Ok((
                cache_date.unwrap_or(chrono::Local::now().date_naive().into()),
                pricing_map,
            ))
        } else {
            let pricing = reqwest::get("https://litellm.github.io/pricing.json")
                .await?
                .json::<serde_json::Value>()
                .await?;

            let cache_date: chrono::NaiveDateTime = chrono::Local::now().date_naive().into();
            let pricing_map = pricing
                .get("pricing")
                .and_then(|p| p.as_object())
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| {
                            serde_json::from_value::<PricingEntry>(v.clone())
                                .ok()
                                .map(|p| (k.clone(), p))
                        })
                        .collect::<BTreeMap<String, PricingEntry>>()
                })
                .unwrap_or(BTreeMap::new());

            let output = json!({
                "last_updated": cache_date.to_string(),
                "pricing": pricing_map,
            });
            std::fs::write(&self.cache_path, serde_json::to_string(&output)?)?;

            Ok((cache_date, pricing_map))
        }
    }
}
