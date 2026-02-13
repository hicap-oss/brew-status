use crate::models::{Credentials, HistoryEntry, ProfileResponse, StatsCache, TodaySummary, UsageLimits};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tauri::{AppHandle, LogicalSize, Manager, Size};
use tauri_plugin_positioner::{Position, WindowExt};

fn claude_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".claude")
}

#[tauri::command]
pub fn get_stats_cache() -> Result<StatsCache, String> {
    let path = claude_dir().join("stats-cache.json");
    let data =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read stats-cache.json: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse stats-cache.json: {}", e))
}

#[tauri::command]
pub fn get_today_summary() -> Result<TodaySummary, String> {
    let stats = get_stats_cache()?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let activity = stats.daily_activity.iter().find(|a| a.date == today);

    let tokens_entry = stats.daily_model_tokens.iter().find(|t| t.date == today);

    let tokens_by_model = tokens_entry
        .map(|t| t.tokens_by_model.clone())
        .unwrap_or_default();

    let total_tokens: u64 = tokens_by_model.values().sum();

    Ok(TodaySummary {
        date: today,
        total_tokens,
        tokens_by_model,
        messages: activity.map(|a| a.message_count).unwrap_or(0),
        sessions: activity.map(|a| a.session_count).unwrap_or(0),
        tool_calls: activity.map(|a| a.tool_call_count).unwrap_or(0),
    })
}

#[tauri::command]
pub fn get_history(limit: usize) -> Result<Vec<HistoryEntry>, String> {
    let path = claude_dir().join("history.jsonl");
    let file = fs::File::open(&path).map_err(|e| format!("Failed to open history.jsonl: {}", e))?;
    let reader = BufReader::new(file);

    let mut entries: Vec<HistoryEntry> = reader
        .lines()
        .filter_map(|line| line.ok())
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect();

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries.truncate(limit);
    Ok(entries)
}

#[tauri::command]
pub fn get_daily_token_totals() -> Result<Vec<HashMap<String, serde_json::Value>>, String> {
    let stats = get_stats_cache()?;
    let result: Vec<HashMap<String, serde_json::Value>> = stats
        .daily_model_tokens
        .iter()
        .map(|day| {
            let total: u64 = day.tokens_by_model.values().sum();
            let mut map = HashMap::new();
            map.insert(
                "date".to_string(),
                serde_json::Value::String(day.date.clone()),
            );
            map.insert("total".to_string(), serde_json::json!(total));
            map.insert(
                "byModel".to_string(),
                serde_json::json!(day.tokens_by_model),
            );
            map
        })
        .collect();
    Ok(result)
}

#[tauri::command]
pub fn resize_popup(app: AppHandle, height: f64) -> Result<(), String> {
    const POPUP_WIDTH: f64 = 320.0;
    const POPUP_MIN_HEIGHT: f64 = 420.0;
    const POPUP_MAX_HEIGHT: f64 = 700.0;

    let clamped_height = height.clamp(POPUP_MIN_HEIGHT, POPUP_MAX_HEIGHT);

    if let Some(window) = app.get_webview_window("popup") {
        window
            .set_size(Size::Logical(LogicalSize::new(POPUP_WIDTH, clamped_height)))
            .map_err(|e| format!("Failed to resize popup: {}", e))?;

        if window
            .move_window_constrained(Position::TrayCenter)
            .is_err()
        {
            let _ = window.move_window(Position::BottomRight);
        }
    }

    Ok(())
}

fn get_oauth_token() -> Result<String, String> {
    let creds_path = claude_dir().join(".credentials.json");
    let creds_data = fs::read_to_string(&creds_path)
        .map_err(|e| format!("Failed to read credentials: {}", e))?;
    let creds: Credentials = serde_json::from_str(&creds_data)
        .map_err(|e| format!("Failed to parse credentials: {}", e))?;

    let oauth = creds
        .claude_ai_oauth
        .ok_or_else(|| "No OAuth token found in credentials".to_string())?;

    Ok(oauth.access_token)
}

#[tauri::command]
pub async fn get_usage_limits() -> Result<UsageLimits, String> {
    let token = get_oauth_token()?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {}", token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch usage limits: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API returned status {}", resp.status()));
    }

    let limits: UsageLimits = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse usage limits: {}", e))?;

    Ok(limits)
}

#[tauri::command]
pub async fn get_profile() -> Result<ProfileResponse, String> {
    let token = get_oauth_token()?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/api/oauth/profile")
        .header("Authorization", format!("Bearer {}", token))
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch profile: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("API returned status {}", resp.status()));
    }

    let profile: ProfileResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse profile: {}", e))?;

    Ok(profile)
}
