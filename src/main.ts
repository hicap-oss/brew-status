import {
  formatTokenCount,
  formatNumber,
  formatDuration,
  formatDate,
  formatShortDate,
  timeAgo,
  modelDisplayName,
} from "./shared/formatters";
import { renderBarChart, renderHourlyHeatmap } from "./shared/chart";
import type { StatsCache, HistoryEntry } from "./shared/types";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWebviewWindow } = window.__TAURI__.webviewWindow;

let cachedStats: StatsCache | null = null;
let currentMetric: "tokens" | "messages" | "toolCalls" = "tokens";

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

// Titlebar controls
document.getElementById("btn-minimize")!.addEventListener("click", async () => {
  const win = getCurrentWebviewWindow();
  await win.minimize();
});

document.getElementById("btn-close")!.addEventListener("click", async () => {
  const win = getCurrentWebviewWindow();
  await win.close();
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
loadStats();
loadHistory();
