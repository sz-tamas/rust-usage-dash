use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub id: String,
    pub provider_type: String,
    pub display_name: String,
    pub secret_ref: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub id: String,
    pub label: String,
    pub used: f64,
    pub limit: Option<f64>,
    pub unit: String,
}

#[derive(Debug, Clone)]
pub struct UsageSnapshot {
    pub provider_id: String,
    pub timestamp: String,
    pub status: String,
    pub cost: Option<f64>,
    pub currency: Option<String>,
    pub metrics: Vec<Metric>,
}

#[derive(Deserialize)]
pub struct NewProvider {
    pub provider_type: String,
    pub display_name: String,
    pub secret_ref: String,
}
