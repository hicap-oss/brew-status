use crate::models::{
    Credentials, DailyActivity, DailyModelTokens, HistoryEntry, LongestSession, ModelUsage,
    ProfileResponse, StatsCache, TodaySummary, UpdateResult, UsageLimits,
};
use chrono::{DateTime, Local, Timelike};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;
use tauri::{AppHandle, LogicalSize, Manager, Size};
use tauri_plugin_positioner::{Position, WindowExt};
use tauri_plugin_updater::UpdaterExt;

fn claude_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".claude")
}

#[tauri::command]
pub fn get_stats_cache() -> Result<StatsCache, String> {
    let path = claude_dir().join("stats-cache.json");
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(stats) = serde_json::from_str::<StatsCache>(&data) {
            return Ok(stats);
        }
    }

    compute_stats_cache_from_sessions()
}

#[tauri::command]
pub fn get_today_summary() -> Result<TodaySummary, String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let stats = get_stats_cache()?;

    let activity = stats.daily_activity.iter().find(|a| a.date == today);
    let tokens_entry = stats.daily_model_tokens.iter().find(|t| t.date == today);
    let tokens_by_model = tokens_entry
        .map(|t| t.tokens_by_model.clone())
        .unwrap_or_default();

    Ok(TodaySummary {
        date: today,
        total_tokens: tokens_by_model.values().sum(),
        tokens_by_model,
        messages: activity.map(|a| a.message_count).unwrap_or(0),
        sessions: activity.map(|a| a.session_count).unwrap_or(0),
        tool_calls: activity.map(|a| a.tool_call_count).unwrap_or(0),
    })
}

#[derive(Default)]
struct SessionAgg {
    first_ts_ms: Option<i64>,
    first_ts_iso: Option<String>,
    last_ts_ms: Option<i64>,
    message_count: u64,
}

#[derive(Default)]
struct AssistantMsgAgg {
    session_id: String,
    date: String,
    hour: u32,
    timestamp_ms: i64,
    timestamp_iso: String,
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_input_tokens: u64,
    cache_creation_input_tokens: u64,
    web_search_requests: u64,
    tool_uses: HashSet<String>,
}

fn session_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let projects_dir = claude_dir().join("projects");
    if let Ok(projects) = fs::read_dir(projects_dir) {
        for project_entry in projects.flatten() {
            if !project_entry.path().is_dir() {
                continue;
            }
            if let Ok(files) = fs::read_dir(project_entry.path()) {
                for file_entry in files.flatten() {
                    let path = file_entry.path();
                    let is_jsonl = path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"));
                    if is_jsonl {
                        paths.push(path);
                    }
                }
            }
        }
    }
    paths
}

fn parse_local_timestamp(value: &serde_json::Value) -> Option<DateTime<Local>> {
    value
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Local))
}

fn update_session_agg(
    sessions: &mut HashMap<String, SessionAgg>,
    session_id: &str,
    timestamp_ms: i64,
    timestamp_iso: &str,
) {
    let session = sessions.entry(session_id.to_string()).or_default();

    if session.first_ts_ms.is_none_or(|v| timestamp_ms < v) {
        session.first_ts_ms = Some(timestamp_ms);
        session.first_ts_iso = Some(timestamp_iso.to_string());
    }
    if session.last_ts_ms.is_none_or(|v| timestamp_ms > v) {
        session.last_ts_ms = Some(timestamp_ms);
    }
    session.message_count += 1;
}

fn compute_stats_cache_from_sessions() -> Result<StatsCache, String> {
    let mut user_ids_seen: HashSet<String> = HashSet::new();
    let mut assistant_messages: HashMap<String, AssistantMsgAgg> = HashMap::new();

    let mut daily_user_messages: HashMap<String, u64> = HashMap::new();
    let mut daily_assistant_messages: HashMap<String, u64> = HashMap::new();
    let mut daily_tool_calls: HashMap<String, u64> = HashMap::new();
    let mut daily_sessions: HashMap<String, HashSet<String>> = HashMap::new();

    let mut daily_model_tokens: HashMap<String, HashMap<String, u64>> = HashMap::new();
    let mut model_usage: HashMap<String, ModelUsage> = HashMap::new();

    let mut hour_counts: HashMap<String, u64> = HashMap::new();
    let mut sessions: HashMap<String, SessionAgg> = HashMap::new();

    for path in session_paths() {
        let session_id_from_path = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();

        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(_) => continue,
        };
        let reader = BufReader::new(file);

        for (idx, line) in reader.lines().enumerate() {
            let Ok(line) = line else { continue };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let Some(timestamp) = parse_local_timestamp(&value) else {
                continue;
            };
            let timestamp_ms = timestamp.timestamp_millis();
            let timestamp_iso = timestamp.to_rfc3339();
            let date = timestamp.format("%Y-%m-%d").to_string();
            let hour = timestamp.hour();
            let hour_key = hour.to_string();
            let session_id = value
                .get("sessionId")
                .and_then(|s| s.as_str())
                .unwrap_or(&session_id_from_path)
                .to_string();
            let kind = value.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match kind {
                "user" => {
                    let message_id = value
                        .get("uuid")
                        .and_then(|u| u.as_str())
                        .map(ToString::to_string)
                        .unwrap_or_else(|| format!("{}:{}:user", session_id, idx));

                    if !user_ids_seen.insert(message_id) {
                        continue;
                    }

                    *daily_user_messages.entry(date.clone()).or_insert(0) += 1;
                    daily_sessions
                        .entry(date)
                        .or_default()
                        .insert(session_id.clone());
                    *hour_counts.entry(hour_key).or_insert(0) += 1;
                    update_session_agg(&mut sessions, &session_id, timestamp_ms, &timestamp_iso);
                }
                "assistant" => {
                    let message_id = value
                        .get("message")
                        .and_then(|m| m.get("id"))
                        .and_then(|id| id.as_str())
                        .map(ToString::to_string)
                        .or_else(|| {
                            value
                                .get("uuid")
                                .and_then(|u| u.as_str())
                                .map(ToString::to_string)
                        })
                        .unwrap_or_else(|| format!("{}:{}:assistant", session_id, idx));

                    let message = value.get("message").unwrap_or(&serde_json::Value::Null);
                    let usage = message.get("usage").unwrap_or(&serde_json::Value::Null);

                    let entry = assistant_messages.entry(message_id).or_default();
                    if entry.session_id.is_empty() {
                        entry.session_id = session_id.clone();
                    }
                    if entry.date.is_empty() {
                        entry.date = date.clone();
                    }
                    if entry.timestamp_ms == 0 {
                        entry.timestamp_ms = timestamp_ms;
                        entry.timestamp_iso = timestamp_iso.clone();
                        entry.hour = hour;
                    }

                    if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
                        if !model.is_empty() {
                            entry.model = model.to_string();
                        }
                    }

                    entry.input_tokens = entry.input_tokens.max(
                        usage
                            .get("input_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0),
                    );
                    entry.output_tokens = entry.output_tokens.max(
                        usage
                            .get("output_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0),
                    );
                    entry.cache_read_input_tokens = entry.cache_read_input_tokens.max(
                        usage
                            .get("cache_read_input_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0),
                    );
                    entry.cache_creation_input_tokens = entry.cache_creation_input_tokens.max(
                        usage
                            .get("cache_creation_input_tokens")
                            .and_then(|t| t.as_u64())
                            .unwrap_or(0),
                    );

                    let direct_ws = usage
                        .get("web_search_requests")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0);
                    let server_ws = usage
                        .get("server_tool_use")
                        .and_then(|s| s.get("web_search_requests"))
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0);
                    entry.web_search_requests = entry
                        .web_search_requests
                        .max(std::cmp::max(direct_ws, server_ws));

                    if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                        for block in content {
                            if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
                                continue;
                            }
                            let signature = block
                                .get("id")
                                .and_then(|id| id.as_str())
                                .map(|id| format!("id:{id}"))
                                .unwrap_or_else(|| format!("anon:{}", block));
                            entry.tool_uses.insert(signature);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    for msg in assistant_messages.values() {
        *daily_assistant_messages
            .entry(msg.date.clone())
            .or_insert(0) += 1;
        daily_sessions
            .entry(msg.date.clone())
            .or_default()
            .insert(msg.session_id.clone());
        *daily_tool_calls.entry(msg.date.clone()).or_insert(0) += msg.tool_uses.len() as u64;
        *hour_counts.entry(msg.hour.to_string()).or_insert(0) += 1;
        update_session_agg(
            &mut sessions,
            &msg.session_id,
            msg.timestamp_ms,
            &msg.timestamp_iso,
        );

        if !msg.model.is_empty() {
            let model_day = daily_model_tokens.entry(msg.date.clone()).or_default();
            *model_day.entry(msg.model.clone()).or_insert(0) += msg.output_tokens;

            let usage = model_usage.entry(msg.model.clone()).or_insert(ModelUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
                web_search_requests: 0,
                cost_usd: 0.0,
                context_window: 0,
                max_output_tokens: 0,
            });
            usage.input_tokens += msg.input_tokens;
            usage.output_tokens += msg.output_tokens;
            usage.cache_read_input_tokens += msg.cache_read_input_tokens;
            usage.cache_creation_input_tokens += msg.cache_creation_input_tokens;
            usage.web_search_requests += msg.web_search_requests;
        }
    }

    let mut all_dates: HashSet<String> = HashSet::new();
    all_dates.extend(daily_user_messages.keys().cloned());
    all_dates.extend(daily_assistant_messages.keys().cloned());
    all_dates.extend(daily_model_tokens.keys().cloned());

    let mut sorted_dates: Vec<String> = all_dates.into_iter().collect();
    sorted_dates.sort();

    let daily_activity: Vec<DailyActivity> = sorted_dates
        .iter()
        .map(|date| DailyActivity {
            date: date.clone(),
            message_count: daily_user_messages.get(date).copied().unwrap_or(0)
                + daily_assistant_messages.get(date).copied().unwrap_or(0),
            session_count: daily_sessions.get(date).map_or(0, |s| s.len() as u64),
            tool_call_count: daily_tool_calls.get(date).copied().unwrap_or(0),
        })
        .collect();

    let daily_model_tokens: Vec<DailyModelTokens> = sorted_dates
        .into_iter()
        .map(|date| DailyModelTokens {
            date: date.clone(),
            tokens_by_model: daily_model_tokens.remove(&date).unwrap_or_default(),
        })
        .collect();

    let mut longest_session: Option<LongestSession> = None;
    let mut first_session_date: Option<String> = None;

    for (session_id, session) in sessions.iter() {
        let Some(first_ms) = session.first_ts_ms else {
            continue;
        };
        let Some(last_ms) = session.last_ts_ms else {
            continue;
        };

        let duration_ms = (last_ms.saturating_sub(first_ms)) as u64;
        let first_local = DateTime::<Local>::from(
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(first_ms as u64),
        );
        let first_date = first_local.format("%Y-%m-%d").to_string();

        if first_session_date
            .as_ref()
            .is_none_or(|current| first_date < *current)
        {
            first_session_date = Some(first_date);
        }

        if longest_session
            .as_ref()
            .is_none_or(|current| duration_ms > current.duration)
        {
            longest_session = Some(LongestSession {
                session_id: session_id.clone(),
                duration: duration_ms,
                message_count: session.message_count,
                timestamp: session.first_ts_iso.clone().unwrap_or_default(),
            });
        }
    }

    Ok(StatsCache {
        version: 1,
        last_computed_date: Local::now().format("%Y-%m-%d").to_string(),
        daily_activity,
        daily_model_tokens,
        model_usage,
        total_sessions: sessions.len() as u64,
        total_messages: (user_ids_seen.len() + assistant_messages.len()) as u64,
        longest_session,
        first_session_date,
        hour_counts,
        total_speculation_time_saved_ms: 0,
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

        // Snap popup flush against the taskbar
        #[cfg(target_os = "windows")]
        {
            if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
                if let Some(work_bottom) = crate::tray::get_work_area_bottom(pos.x, pos.y) {
                    let y = work_bottom - size.height as i32;
                    let _ = window.set_position(tauri::PhysicalPosition::new(pos.x, y));
                }
            }
        }
    }

    Ok(())
}

fn get_oauth_token() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        return get_macos_oauth_token();
    }

    #[cfg(not(target_os = "macos"))]
    {
        return get_legacy_oauth_token();
    }
}

#[cfg(not(target_os = "macos"))]
fn get_legacy_oauth_token() -> Result<String, String> {
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

#[cfg(target_os = "macos")]
fn get_macos_oauth_token() -> Result<String, String> {
    if let Ok(token) = get_macos_claude_code_oauth_token() {
        return Ok(token);
    }

    // Fallback to desktop cache for environments where Claude Code credentials
    // are unavailable. This token may not include profile scopes.
    get_macos_desktop_oauth_token()
}

#[cfg(target_os = "macos")]
fn get_macos_claude_code_oauth_token() -> Result<String, String> {
    let account = std::env::var("USER").unwrap_or_default();
    let keychain_json = get_keychain_secret("Claude Code-credentials", Some(&account))
        .or_else(|_| get_keychain_secret("Claude Code-credentials", None))?;

    let creds: Credentials = serde_json::from_str(&keychain_json)
        .map_err(|e| format!("Failed to parse Claude Code keychain credentials JSON: {e}"))?;
    let oauth = creds
        .claude_ai_oauth
        .ok_or_else(|| "claudeAiOauth missing from Claude Code keychain credentials".to_string())?;

    Ok(oauth.access_token)
}

#[cfg(target_os = "macos")]
fn get_macos_desktop_oauth_token() -> Result<String, String> {
    let config_path = dirs::home_dir()
        .ok_or_else(|| "Could not find home directory".to_string())?
        .join("Library")
        .join("Application Support")
        .join("Claude")
        .join("config.json");

    let config_data = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read Claude config.json: {e}"))?;
    let config: serde_json::Value = serde_json::from_str(&config_data)
        .map_err(|e| format!("Failed to parse Claude config.json: {e}"))?;

    let token_cache_encrypted = config
        .get("oauth:tokenCache")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Claude oauth:tokenCache not found in config.json".to_string())?;

    let keychain_secret = get_keychain_secret("Claude Safe Storage", Some("Claude"))
        .or_else(|_| get_keychain_secret("Claude Safe Storage", None))?;
    let decrypted = decrypt_chromium_v10(token_cache_encrypted, &keychain_secret)?;

    let token_cache: serde_json::Value = serde_json::from_str(&decrypted)
        .map_err(|e| format!("Failed to parse decrypted OAuth token cache: {e}"))?;

    let token = token_cache
        .as_object()
        .and_then(|entries| {
            let mut selected: Option<(i64, String)> = None;
            for entry in entries.values() {
                let Some(token) = entry.get("token").and_then(|t| t.as_str()) else {
                    continue;
                };
                let expires_at = entry
                    .get("expiresAt")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(i64::MIN);
                if selected
                    .as_ref()
                    .is_none_or(|(best_exp, _)| expires_at > *best_exp)
                {
                    selected = Some((expires_at, token.to_string()));
                }
            }
            selected.map(|(_, token)| token)
        })
        .ok_or_else(|| "No OAuth token found in decrypted Claude token cache".to_string())?;

    Ok(token)
}

#[cfg(target_os = "macos")]
fn get_keychain_secret(service: &str, account: Option<&str>) -> Result<String, String> {
    let mut args = vec!["find-generic-password", "-s", service];
    if let Some(account) = account {
        args.extend(["-a", account]);
    }
    args.push("-w");

    let output = Command::new("security")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run security CLI: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let scope = account
            .map(|a| format!("account '{a}'"))
            .unwrap_or_else(|| "any account".to_string());
        return Err(format!(
            "Failed to read macOS Keychain item '{service}' ({scope}): {}",
            stderr.trim()
        ));
    }

    let secret = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to parse keychain secret as UTF-8: {e}"))?;

    Ok(secret.trim().to_string())
}

#[cfg(target_os = "macos")]
fn decrypt_chromium_v10(ciphertext_b64: &str, password: &str) -> Result<String, String> {
    use aes::Aes128;
    use base64::Engine;
    use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    use pbkdf2::pbkdf2_hmac;
    use sha1::Sha1;

    type Aes128CbcDec = cbc::Decryptor<Aes128>;

    let encrypted = base64::engine::general_purpose::STANDARD
        .decode(ciphertext_b64)
        .map_err(|e| format!("Failed to decode OAuth token cache payload: {e}"))?;

    if encrypted.len() < 4 || &encrypted[..3] != b"v10" {
        return Err("Unsupported OAuth token cache format (expected v10)".to_string());
    }

    let mut key = [0u8; 16];
    pbkdf2_hmac::<Sha1>(password.as_bytes(), b"saltysalt", 1003, &mut key);
    let iv = [b' '; 16];

    let mut payload = encrypted[3..].to_vec();
    let decrypted = Aes128CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut payload)
        .map_err(|e| format!("Failed to decrypt OAuth token cache: {e}"))?;

    String::from_utf8(decrypted.to_vec())
        .map_err(|e| format!("Decrypted OAuth token cache is not valid UTF-8: {e}"))
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
    let updater = app
        .updater()
        .map_err(|e| format!("Updater init error: {e}"))?;

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
