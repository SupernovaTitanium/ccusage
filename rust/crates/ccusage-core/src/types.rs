use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::TimestampMs;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageEntry {
    pub session_id: Option<String>,
    pub timestamp: String,
    pub version: Option<String>,
    pub message: UsageMessage,
    #[serde(rename = "costUSD")]
    pub cost_usd: Option<f64>,
    pub request_id: Option<String>,
    pub is_api_error_message: Option<bool>,
    pub is_sidechain: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsageMessage {
    pub usage: TokenUsageRaw,
    pub model: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct TokenUsageRaw {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    pub speed: Option<Speed>,
    #[serde(default)]
    pub cache_creation: Option<CacheCreationRaw>,
}

impl TokenUsageRaw {
    pub fn cache_creation_buckets(&self) -> (u64, u64, u64) {
        if let Some(b) = &self.cache_creation {
            (b.ephemeral_5m_input_tokens, b.ephemeral_1h_input_tokens, 0)
        } else {
            (0, 0, self.cache_creation_input_tokens)
        }
    }

    pub fn cache_creation_token_count(&self) -> u64 {
        let (five_minute, one_hour, unbucketed) = self.cache_creation_buckets();
        five_minute
            .saturating_add(one_hour)
            .saturating_add(unbucketed)
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct CacheCreationRaw {
    #[serde(default)]
    pub(crate) ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    pub(crate) ephemeral_1h_input_tokens: u64,
}

impl CacheCreationRaw {
    pub fn from_ttl_buckets(
        ephemeral_5m_input_tokens: u64,
        ephemeral_1h_input_tokens: u64,
    ) -> Self {
        Self {
            ephemeral_5m_input_tokens,
            ephemeral_1h_input_tokens,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speed {
    Standard,
    Fast,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCounts {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_creation_5m_tokens: u64,
    pub cache_creation_1h_tokens: u64,
    pub cache_creation_unbucketed_tokens: u64,
    pub cache_read_tokens: u64,
    pub(crate) extra_total_tokens: u64,
}

impl TokenCounts {
    pub fn add_usage(&mut self, usage: TokenUsageRaw) {
        self.input_tokens = self.input_tokens.saturating_add(usage.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(usage.output_tokens);
        let (five_minute, one_hour, unbucketed) = usage.cache_creation_buckets();
        self.cache_creation_5m_tokens = self.cache_creation_5m_tokens.saturating_add(five_minute);
        self.cache_creation_1h_tokens = self.cache_creation_1h_tokens.saturating_add(one_hour);
        self.cache_creation_unbucketed_tokens = self
            .cache_creation_unbucketed_tokens
            .saturating_add(unbucketed);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(five_minute)
            .saturating_add(one_hour)
            .saturating_add(unbucketed);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(usage.cache_read_input_tokens);
    }

    pub fn total(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_creation_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.extra_total_tokens)
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBreakdown {
    pub model_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_creation_5m_tokens: u64,
    pub cache_creation_1h_tokens: u64,
    pub cache_creation_unbucketed_tokens: u64,
    pub cache_read_tokens: u64,
    #[serde(skip_serializing)]
    pub extra_total_tokens: u64,
    pub cost: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub missing_pricing: bool,
}

impl ModelBreakdown {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_creation_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.extra_total_tokens)
    }
}

#[derive(Debug, Clone)]
pub struct LoadedEntry {
    pub data: UsageEntry,
    pub timestamp: TimestampMs,
    pub date: String,
    pub project: Arc<str>,
    pub session_id: Arc<str>,
    pub project_path: Arc<str>,
    pub cost: f64,
    pub extra_total_tokens: u64,
    pub credits: Option<f64>,
    pub message_count: Option<u64>,
    pub model: Option<String>,
    pub usage_limit_reset_time: Option<TimestampMs>,
    pub missing_pricing_model: Option<String>,
}

#[derive(Debug)]
pub struct LoadedFile {
    pub timestamp: Option<TimestampMs>,
    pub entries: Vec<LoadedEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub week: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_activity: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_creation_5m_tokens: u64,
    pub cache_creation_1h_tokens: u64,
    pub cache_creation_unbucketed_tokens: u64,
    pub cache_read_tokens: u64,
    #[serde(skip_serializing)]
    pub extra_total_tokens: u64,
    pub total_cost: f64,
    pub credits: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_count: Option<u64>,
    pub models_used: Vec<String>,
    pub model_breakdowns: Vec<ModelBreakdown>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<String>>,
}

impl UsageSummary {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_creation_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.extra_total_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturates_token_count_accumulation_and_total() {
        let mut counts = TokenCounts {
            input_tokens: u64::MAX,
            output_tokens: u64::MAX,
            cache_creation_tokens: u64::MAX,
            cache_creation_5m_tokens: u64::MAX,
            cache_creation_1h_tokens: u64::MAX,
            cache_creation_unbucketed_tokens: u64::MAX,
            cache_read_tokens: u64::MAX,
            extra_total_tokens: u64::MAX,
        };
        counts.add_usage(TokenUsageRaw {
            input_tokens: 1,
            output_tokens: 1,
            cache_creation_input_tokens: 1,
            cache_read_input_tokens: 1,
            ..TokenUsageRaw::default()
        });

        assert_eq!(counts.input_tokens, u64::MAX);
        assert_eq!(counts.output_tokens, u64::MAX);
        assert_eq!(counts.cache_creation_tokens, u64::MAX);
        assert_eq!(counts.cache_read_tokens, u64::MAX);
        assert_eq!(counts.total(), u64::MAX);
    }
}
