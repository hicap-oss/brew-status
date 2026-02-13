import { formatTokenCount, formatNumber, modelDisplayName, formatResetTime } from "./shared/formatters";
import type { TodaySummary, StatsCache, UsageLimits, LimitEntry, ProfileResponse } from "./shared/types";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let resizeTimer: number | null = null;
let lastRequestedHeight: number | null = null;

function schedulePopupResize(): void {
  if (resizeTimer !== null) {
    window.clearTimeout(resizeTimer);
  }

  resizeTimer = window.setTimeout(() => {
    resizeTimer = null;
    void resizePopupToContent();
  }, 16);
}

async function resizePopupToContent(): Promise<void> {
  const container = document.querySelector(".popup-container") as HTMLElement | null;
  if (!container) return;

  // Temporarily allow overflow so we get the true content height
  const prevOverflow = document.body.style.overflow;
  document.body.style.overflow = "visible";
  const contentHeight = Math.ceil(
    Math.max(
      container.offsetHeight,
      container.scrollHeight,
      document.body.scrollHeight
    ) + 32
  );
  document.body.style.overflow = prevOverflow;
  if (lastRequestedHeight !== null && Math.abs(contentHeight - lastRequestedHeight) < 2) {
    return;
  }

  try {
    await invoke("resize_popup", { height: contentHeight });
    lastRequestedHeight = contentHeight;
  } catch (e) {
    console.error("Failed to resize popup:", e);
  }
}

async function loadProfile(): Promise<void> {
  try {
    const profile = await invoke<ProfileResponse>("get_profile");

    const nameEl = document.getElementById("popup-user-name")!;
    nameEl.textContent = profile.account.display_name || profile.account.full_name;

    const badgesEl = document.getElementById("popup-badges")!;
    let badges = "";

    if (profile.account.has_claude_max) {
      badges += '<span class="popup-badge plan-max">Max</span>';
    } else if (profile.account.has_claude_pro) {
      badges += '<span class="popup-badge plan-pro">Pro</span>';
    }

    if (profile.organization.name) {
      badges += `<span class="popup-badge org">${profile.organization.name}</span>`;
    }

    badgesEl.innerHTML = badges;
  } catch (e) {
    console.error("Failed to load profile:", e);
  } finally {
    schedulePopupResize();
  }
}

async function loadData(): Promise<void> {
  try {
    const [summary, stats] = await Promise.all([
      invoke<TodaySummary>("get_today_summary"),
      invoke<StatsCache>("get_stats_cache"),
    ]);

    // Date
    const dateEl = document.getElementById("today-date")!;
    const d = new Date(summary.date);
    dateEl.textContent = d.toLocaleDateString("en-US", {
      weekday: "short",
      month: "short",
      day: "numeric",
    });

    // Hero
    document.getElementById("total-tokens")!.textContent = formatTokenCount(summary.totalTokens);

    // Model breakdown
    const breakdownEl = document.getElementById("model-breakdown")!;
    breakdownEl.innerHTML = "";
    for (const [model, tokens] of Object.entries(summary.tokensByModel)) {
      const cls = model.includes("opus") ? "opus" : "sonnet";
      breakdownEl.innerHTML += `
        <span class="model-tag">
          <span class="model-dot ${cls}"></span>
          ${modelDisplayName(model)}: ${formatTokenCount(tokens)}
        </span>`;
    }

    // Stats
    document.getElementById("sessions-today")!.textContent = formatNumber(summary.sessions);
    document.getElementById("messages-today")!.textContent = formatNumber(summary.messages);
    document.getElementById("tool-calls-today")!.textContent = formatNumber(summary.toolCalls);
    document.getElementById("total-sessions")!.textContent = formatNumber(stats.totalSessions);
  } catch (e) {
    console.error("Failed to load data:", e);
  } finally {
    schedulePopupResize();
  }
}

function renderLimitBar(label: string, entry: LimitEntry): string {
  const pct = Math.round(entry.utilization);
  const fillClass = pct >= 90 ? "critical" : pct >= 70 ? "high" : "";
  const resetText = entry.resets_at ? formatResetTime(entry.resets_at) : "";

  return `
    <div class="limit-item">
      <div class="limit-header">
        <span class="limit-label">${label}</span>
        <span class="limit-pct">${pct}%</span>
      </div>
      <div class="limit-bar-track">
        <div class="limit-bar-fill ${fillClass}" style="width: ${Math.min(pct, 100)}%"></div>
      </div>
      ${resetText ? `<span class="limit-reset">${resetText}</span>` : ""}
    </div>`;
}

async function loadLimits(): Promise<void> {
  const container = document.getElementById("limits-content")!;
  container.innerHTML = '<div class="limits-loading">Loading limits...</div>';

  try {
    const limits = await invoke<UsageLimits>("get_usage_limits");
    let html = "";

    if (limits.five_hour) {
      html += renderLimitBar("Current Session", limits.five_hour);
    }
    if (limits.seven_day) {
      html += renderLimitBar("All Models (Weekly)", limits.seven_day);
    }
    if (limits.seven_day_sonnet) {
      html += renderLimitBar("Sonnet Only", limits.seven_day_sonnet);
    }
    if (limits.seven_day_opus) {
      html += renderLimitBar("Opus (Weekly)", limits.seven_day_opus);
    }
    if (limits.seven_day_cowork) {
      html += renderLimitBar("Cowork (Weekly)", limits.seven_day_cowork);
    }
    if (limits.seven_day_oauth_apps) {
      html += renderLimitBar("OAuth Apps (Weekly)", limits.seven_day_oauth_apps);
    }
    if (limits.extra_usage?.is_enabled && limits.extra_usage.utilization !== null) {
      html += renderLimitBar("Extra Usage", {
        utilization: limits.extra_usage.utilization,
        resets_at: null,
      });
    }

    if (!html) {
      html = '<div class="limits-loading">No usage limits available</div>';
    }

    container.innerHTML = html;
  } catch (e) {
    container.innerHTML = `<div class="limits-error">Failed to load limits. Check your OAuth credentials.</div>`;
    console.error("Failed to load limits:", e);
  } finally {
    schedulePopupResize();
  }
}

// Tab switching
document.querySelectorAll(".tab-btn").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".tab-btn").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");

    const tab = (btn as HTMLElement).dataset.tab!;
    document.querySelectorAll(".tab-panel").forEach((p) => p.classList.add("hidden"));
    document.getElementById(`panel-${tab}`)!.classList.remove("hidden");

    // Force resize since content height changed
    lastRequestedHeight = null;
    schedulePopupResize();
  });
});

// Refresh buttons
document.getElementById("popup-refresh-limits")!.addEventListener("click", () => {
  const btn = document.getElementById("popup-refresh-limits")!;
  btn.classList.add("spinning");
  loadLimits().finally(() => btn.classList.remove("spinning"));
});


// Live updates
listen("stats-updated", () => loadData());

window.addEventListener("resize", schedulePopupResize);

if ("fonts" in document) {
  void (document as Document & { fonts: { ready: Promise<unknown> } }).fonts.ready.then(() => {
    schedulePopupResize();
  });
}

// Initial load
loadProfile();
loadData();
loadLimits();
schedulePopupResize();
