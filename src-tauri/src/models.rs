use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// OAuth credentials from ~/.claude/.credentials.json
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credentials {
    pub claude_ai_oauth: Option<OAuthToken>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct OAuthToken {
    pub access_token: String,
    pub expires_at: i64,
}

// Profile from Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileResponse {
    pub account: AccountInfo,
    pub organization: OrganizationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountInfo {
    pub uuid: String,
    pub full_name: String,
    pub display_name: String,
    pub email: String,
    pub has_claude_max: bool,
    pub has_claude_pro: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationInfo {
    pub uuid: String,
    pub name: String,
    pub organization_type: String,
    pub billing_type: String,
    pub rate_limit_tier: String,
    pub has_extra_usage_enabled: bool,
    pub subscription_status: String,
    pub subscription_created_at: String,
}

// Usage limits from Anthropic API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLimits {
    pub five_hour: Option<LimitEntry>,
    pub seven_day: Option<LimitEntry>,
    pub seven_day_opus: Option<LimitEntry>,
    pub seven_day_sonnet: Option<LimitEntry>,
    pub seven_day_cowork: Option<LimitEntry>,
    pub seven_day_oauth_apps: Option<LimitEntry>,
    pub extra_usage: Option<ExtraUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitEntry {
    pub utilization: f64,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraUsage {
    pub is_enabled: bool,
    pub monthly_limit: Option<f64>,
    pub used_credits: Option<f64>,
    pub utilization: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsCache {
    pub version: u32,
    pub last_computed_date: String,
    pub daily_activity: Vec<DailyActivity>,
    pub daily_model_tokens: Vec<DailyModelTokens>,
    pub model_usage: HashMap<String, ModelUsage>,
    pub total_sessions: u64,
    pub total_messages: u64,
    pub longest_session: Option<LongestSession>,
    pub first_session_date: Option<String>,
    pub hour_counts: HashMap<String, u64>,
    pub total_speculation_time_saved_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    pub date: String,
    pub message_count: u64,
    pub session_count: u64,
    pub tool_call_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyModelTokens {
    pub date: String,
    pub tokens_by_model: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub web_search_requests: u64,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub context_window: u64,
    #[serde(default)]
    pub max_output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LongestSession {
    pub session_id: String,
    pub duration: u64,
    pub message_count: u64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub display: String,
    pub timestamp: u64,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodaySummary {
    pub date: String,
    pub total_tokens: u64,
    pub tokens_by_model: HashMap<String, u64>,
    pub messages: u64,
    pub sessions: u64,
    pub tool_calls: u64,
}
