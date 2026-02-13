import {
  formatTokenCount,
  formatNumber,
  formatDuration,
  formatDate,
  formatShortDate,
  timeAgo,
  modelDisplayName,
  formatResetTime,
} from "./shared/formatters";
import { renderBarChart, renderHourlyHeatmap } from "./shared/chart";
import type { StatsCache, HistoryEntry, UsageLimits, LimitEntry, ProfileResponse } from "./shared/types";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;

let cachedStats: StatsCache | null = null;
let currentMetric: "tokens" | "messages" | "toolCalls" = "tokens";

async function loadProfile(): Promise<void> {
  try {
    const profile = await invoke<ProfileResponse>("get_profile");
    const el = document.getElementById("titlebar-profile")!;

    const parts: string[] = [];
    const name = profile.account.display_name || profile.account.full_name;
    if (name) parts.push(name);

    if (profile.organization.name) {
      const orgType = profile.organization.organization_type;
      const typeLabel = orgType.includes("team") ? "Team" : orgType.includes("enterprise") ? "Enterprise" : "";
      parts.push(typeLabel ? `${profile.organization.name} (${typeLabel})` : profile.organization.name);
    }

    if (profile.account.has_claude_max) {
      parts.push("Max");
    } else if (profile.account.has_claude_pro) {
      parts.push("Pro");
    }

    el.textContent = parts.join(" \u00B7 ");
  } catch (e) {
    console.error("Failed to load profile:", e);
  }
}

async function loadStats(): Promise<void> {
  try {
    cachedStats = await invoke<StatsCache>("get_stats_cache");
    renderTokenUsage(cachedStats);
    renderSessionStats(cachedStats);
    renderHeatmap(cachedStats);
    renderActivityChart(cachedStats, currentMetric);
  } catch (e) {
    console.error("Failed to load stats:", e);
  }
}

async function loadHistory(): Promise<void> {
  try {
    const entries = await invoke<HistoryEntry[]>("get_history", { limit: 50 });
    renderHistory(entries);
  } catch (e) {
    console.error("Failed to load history:", e);
  }
}

function renderTokenUsage(stats: StatsCache): void {
  const container = document.getElementById("token-usage-content")!;
  let html = "";

  const sortedModels = Object.entries(stats.modelUsage).sort((a, b) => {
    const totalA = a[1].inputTokens + a[1].outputTokens + a[1].cacheReadInputTokens;
    const totalB = b[1].inputTokens + b[1].outputTokens + b[1].cacheReadInputTokens;
    return totalB - totalA;
  });

  for (const [model, usage] of sortedModels) {
    const cls = model.includes("opus") ? "opus" : "sonnet";
    html += `
      <div class="model-section">
        <div class="model-name">
          <span class="dot ${cls}"></span>
          ${modelDisplayName(model)}
        </div>
        <div class="token-grid">
          <div class="token-item">
            <span class="token-item-value">${formatTokenCount(usage.inputTokens)}</span>
            <span class="token-item-label">Input</span>
          </div>
          <div class="token-item">
            <span class="token-item-value">${formatTokenCount(usage.outputTokens)}</span>
            <span class="token-item-label">Output</span>
          </div>
          <div class="token-item">
            <span class="token-item-value">${formatTokenCount(usage.cacheReadInputTokens)}</span>
            <span class="token-item-label">Cache Read</span>
          </div>
          <div class="token-item">
            <span class="token-item-value">${formatTokenCount(usage.cacheCreationInputTokens)}</span>
            <span class="token-item-label">Cache Create</span>
          </div>
        </div>
      </div>`;
  }

  container.innerHTML = html;
}

function renderSessionStats(stats: StatsCache): void {
  const container = document.getElementById("session-stats")!;

  const firstDate = stats.firstSessionDate
    ? formatDate(stats.firstSessionDate)
    : "N/A";

  const longestDuration = stats.longestSession
    ? formatDuration(stats.longestSession.duration)
    : "N/A";

  container.innerHTML = `
    <div class="session-stat">
      <div class="session-stat-value">${formatNumber(stats.totalSessions)}</div>
      <div class="session-stat-label">Total Sessions</div>
    </div>
    <div class="session-stat">
      <div class="session-stat-value">${formatNumber(stats.totalMessages)}</div>
      <div class="session-stat-label">Total Messages</div>
    </div>
    <div class="session-stat">
      <div class="session-stat-value">${firstDate}</div>
      <div class="session-stat-label">First Session</div>
    </div>
    <div class="session-stat">
      <div class="session-stat-value">${longestDuration}</div>
      <div class="session-stat-label">Longest Session</div>
    </div>`;
}

function renderHeatmap(stats: StatsCache): void {
  const canvas = document.getElementById("heatmap-canvas") as HTMLCanvasElement;
  renderHourlyHeatmap(canvas, stats.hourCounts);
}

function renderActivityChart(
  stats: StatsCache,
  metric: "tokens" | "messages" | "toolCalls"
): void {
  const canvas = document.getElementById("activity-chart") as HTMLCanvasElement;
  const labels = stats.dailyActivity.map((d) => formatShortDate(d.date));

  let values: number[];
  if (metric === "tokens") {
    values = stats.dailyModelTokens.map((d) =>
      Object.values(d.tokensByModel).reduce((a, b) => a + b, 0)
    );
  } else if (metric === "messages") {
    values = stats.dailyActivity.map((d) => d.messageCount);
  } else {
    values = stats.dailyActivity.map((d) => d.toolCallCount);
  }

  renderBarChart(canvas, labels, values);
}

function renderHistory(entries: HistoryEntry[]): void {
  const container = document.getElementById("history-list")!;
  if (entries.length === 0) {
    container.innerHTML = '<div style="color:#6b6b8a;padding:20px;text-align:center">No history entries</div>';
    return;
  }

  container.innerHTML = entries
    .map((entry) => {
      const project = entry.project
        ? entry.project.split("\\").pop() || entry.project.split("/").pop() || ""
        : "";
      const message = escapeHtml(entry.display).slice(0, 120);
      return `
        <div class="history-item">
          <span class="history-time">${timeAgo(entry.timestamp)}</span>
          <span class="history-message">${message}</span>
          <span class="history-project">${escapeHtml(project)}</span>
        </div>`;
    })
    .join("");
}

function escapeHtml(text: string): string {
  const el = document.createElement("span");
  el.textContent = text;
  return el.innerHTML;
}

function renderDashboardLimitBar(label: string, entry: LimitEntry): string {
  const pct = Math.round(entry.utilization);
  const fillClass = pct >= 90 ? "critical" : pct >= 70 ? "high" : "";
  const resetText = entry.resets_at ? formatResetTime(entry.resets_at) : "";

  return `
    <div class="dashboard-limit-item">
      <div class="dashboard-limit-header">
        <span class="dashboard-limit-label">${label}</span>
        <span class="dashboard-limit-pct">${pct}%</span>
      </div>
      <div class="dashboard-limit-bar-track">
        <div class="dashboard-limit-bar-fill ${fillClass}" style="width: ${Math.min(pct, 100)}%"></div>
      </div>
      ${resetText ? `<span class="dashboard-limit-reset">${resetText}</span>` : ""}
    </div>`;
}

async function loadLimits(): Promise<void> {
  const container = document.getElementById("dashboard-limits")!;

  try {
    const limits = await invoke<UsageLimits>("get_usage_limits");
    let html = "";

    if (limits.five_hour) {
      html += renderDashboardLimitBar("Current Session", limits.five_hour);
    }
    if (limits.seven_day) {
      html += renderDashboardLimitBar("All Models (Weekly)", limits.seven_day);
    }
    if (limits.seven_day_sonnet) {
      html += renderDashboardLimitBar("Sonnet Only", limits.seven_day_sonnet);
    }
    if (limits.seven_day_opus) {
      html += renderDashboardLimitBar("Opus (Weekly)", limits.seven_day_opus);
    }
    html += renderDashboardLimitBar("Cowork (Weekly)", limits.seven_day_cowork ?? { utilization: 0, resets_at: null });
    html += renderDashboardLimitBar("OAuth Apps (Weekly)", limits.seven_day_oauth_apps ?? { utilization: 0, resets_at: null });
    if (limits.extra_usage) {
      if (limits.extra_usage.is_enabled) {
        html += renderDashboardLimitBar("Extra Usage", {
          utilization: limits.extra_usage.utilization ?? 0,
          resets_at: null,
        });
      } else {
        html += `<div class="dashboard-limit-item">
          <div class="dashboard-limit-header">
            <span class="dashboard-limit-label">Extra Usage</span>
            <span class="dashboard-limit-pct disabled">Off</span>
          </div>
          <div class="dashboard-limit-bar-track"><div class="dashboard-limit-bar-fill" style="width:0%"></div></div>
        </div>`;
      }
    }

    if (!html) {
      html = '<div class="limits-loading">No usage limits available</div>';
    }

    container.innerHTML = html;
  } catch (e) {
    container.innerHTML = '<div class="limits-error">Failed to load limits. Check your OAuth credentials.</div>';
    console.error("Failed to load limits:", e);
  }
}

// Titlebar controls
document.getElementById("btn-minimize")!.addEventListener("click", async () => {
  const win = getCurrentWebviewWindow();
  await win.minimize();
});

document.getElementById("btn-close")!.addEventListener("click", async () => {
  const win = getCurrentWebviewWindow();
  await win.close();
});

// Refresh buttons
document.getElementById("dashboard-refresh-limits")!.addEventListener("click", () => {
  const btn = document.getElementById("dashboard-refresh-limits")!;
  btn.classList.add("spinning");
  loadLimits().finally(() => btn.classList.remove("spinning"));
});


// Chart toggle buttons
document.querySelectorAll(".toggle-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".toggle-btn").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    currentMetric = (btn as HTMLElement).dataset.metric as typeof currentMetric;
    if (cachedStats) {
      renderActivityChart(cachedStats, currentMetric);
    }
  });
});

// Live updates
listen("stats-updated", () => loadStats());
listen("history-updated", () => loadHistory());

// Initial load
loadProfile();
loadStats();
loadHistory();
loadLimits();
