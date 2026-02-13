use crate::models::{Credentials, HistoryEntry, ProfileResponse, StatsCache, TodaySummary, UpdateResult, UsageLimits};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tauri::{AppHandle, LogicalSize, Manager, Size};
use tauri_plugin_updater::UpdaterExt;
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
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    // Try stats cache first — if it has today's data, use it
    if let Ok(stats) = get_stats_cache() {
        let activity = stats.daily_activity.iter().find(|a| a.date == today);
        let tokens_entry = stats.daily_model_tokens.iter().find(|t| t.date == today);

        if activity.is_some() || tokens_entry.is_some() {
            let tokens_by_model = tokens_entry
                .map(|t| t.tokens_by_model.clone())
                .unwrap_or_default();
            let total_tokens: u64 = tokens_by_model.values().sum();

            return Ok(TodaySummary {
                date: today,
                total_tokens,
                tokens_by_model,
                messages: activity.map(|a| a.message_count).unwrap_or(0),
                sessions: activity.map(|a| a.session_count).unwrap_or(0),
                tool_calls: activity.map(|a| a.tool_call_count).unwrap_or(0),
            });
        }
    }

    // Cache doesn't have today's data — compute from session files
    compute_today_from_sessions()
}

/// Compute today's usage stats directly from session JSONL files.
/// Used as a fallback when stats-cache.json hasn't been recomputed yet today.
fn compute_today_from_sessions() -> Result<TodaySummary, String> {
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let today_naive = now.date_naive();

    let claude = claude_dir();

    // Step 1: Read history.jsonl to find session IDs with activity today
    let history_path = claude.join("history.jsonl");
    let mut today_session_ids: HashSet<String> = HashSet::new();

    let today_start_ms = today_naive
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .earliest()
        .unwrap()
        .timestamp_millis();

    if let Ok(file) = fs::File::open(&history_path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(&line) {
                let ts = entry.get("timestamp").and_then(|t| t.as_i64()).unwrap_or(0);
                if ts >= today_start_ms {
                    if let Some(sid) = entry.get("sessionId").and_then(|s| s.as_str()) {
                        today_session_ids.insert(sid.to_string());
                    }
                }
            }
        }
    }

    if today_session_ids.is_empty() {
        return Ok(TodaySummary {
            date: today,
            total_tokens: 0,
            tokens_by_model: HashMap::new(),
            messages: 0,
            sessions: 0,
            tool_calls: 0,
        });
    }

    // Step 2: Find session JSONL files in project directories
    let projects_dir = claude.join("projects");
    let mut session_paths: Vec<PathBuf> = Vec::new();

    if let Ok(projects) = fs::read_dir(&projects_dir) {
        for project_entry in projects.flatten() {
            if project_entry.path().is_dir() {
                if let Ok(files) = fs::read_dir(project_entry.path()) {
                    for file_entry in files.flatten() {
                        let name = file_entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".jsonl") {
                            let session_id = name.trim_end_matches(".jsonl");
                            if today_session_ids.contains(session_id) {
                                session_paths.push(file_entry.path());
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 3: Parse session JSONL files for today's entries
    struct AssistantMsg {
        model: String,
        output_tokens: u64,
        tool_calls: u64,
    }

    let mut user_uuids: HashSet<String> = HashSet::new();
    let mut assistant_msgs: HashMap<String, AssistantMsg> = HashMap::new();

    for path in &session_paths {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let reader = BufReader::new(file);

        for line in reader.lines().flatten() {
            let val: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Filter to entries from today (session timestamps are ISO 8601 / UTC)
            let is_today = val
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Local).date_naive() == today_naive)
                .unwrap_or(false);

            if !is_today {
                continue;
            }

            match val.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                "user" => {
                    if let Some(uuid) = val.get("uuid").and_then(|u| u.as_str()) {
                        user_uuids.insert(uuid.to_string());
                    }
                }
                "assistant" => {
                    let msg_id = val
                        .get("message")
                        .and_then(|m| m.get("id"))
                        .and_then(|id| id.as_str())
                        .unwrap_or("")
                        .to_string();

                    if msg_id.is_empty() {
                        continue;
                    }

                    let model = val
                        .get("message")
                        .and_then(|m| m.get("model"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("")
                        .to_string();

                    let output_tokens = val
                        .get("message")
                        .and_then(|m| m.get("usage"))
                        .and_then(|u| u.get("output_tokens"))
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0);

                    // Count tool_use blocks in this streaming chunk
                    let mut tc: u64 = 0;
                    if let Some(content) = val
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_array())
                    {
                        for block in content {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                tc += 1;
                            }
                        }
                    }

                    let entry =
                        assistant_msgs
                            .entry(msg_id)
                            .or_insert_with(|| AssistantMsg {
                                model: String::new(),
                                output_tokens: 0,
                                tool_calls: 0,
                            });

                    if !model.is_empty() {
                        entry.model = model;
                    }
                    // output_tokens is cumulative across streaming chunks — keep the latest
                    entry.output_tokens = output_tokens;
                    // tool_use blocks appear in different chunks — accumulate
                    entry.tool_calls += tc;
                }
                _ => {}
            }
        }
    }

    // Step 4: Aggregate
    let mut tokens_by_model: HashMap<String, u64> = HashMap::new();
    let mut tool_calls: u64 = 0;

    for msg in assistant_msgs.values() {
        tool_calls += msg.tool_calls;
        if !msg.model.is_empty() {
            *tokens_by_model.entry(msg.model.clone()).or_insert(0) += msg.output_tokens;
        }
    }

    let total_tokens: u64 = tokens_by_model.values().sum();

    Ok(TodaySummary {
        date: today,
        total_tokens,
        tokens_by_model,
        messages: user_uuids.len() as u64 + assistant_msgs.len() as u64,
        sessions: today_session_ids.len() as u64,
        tool_calls,
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

#[tauri::command]
pub fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateResult, String> {
    let updater = app.updater().map_err(|e| format!("Updater init error: {e}"))?;

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            return Ok(UpdateResult {
                update_available: false,
                version: None,
            });
        }
        Err(e) => return Err(format!("Update check failed: {e}")),
    };

    let version = update.version.clone();

    // Trigger the install dialog on a background task (non-blocking)
    tauri::async_runtime::spawn(async move {
        crate::prompt_install(update, app).await;
    });

    Ok(UpdateResult {
        update_available: true,
        version: Some(version),
    })
}
