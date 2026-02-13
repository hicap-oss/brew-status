import { formatTokenCount, formatNumber, formatShortDate, modelDisplayName } from "./shared/formatters";
import { renderMiniBar } from "./shared/chart";
import type { TodaySummary, StatsCache } from "./shared/types";

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

    // Mini chart
    const canvas = document.getElementById("mini-chart") as HTMLCanvasElement;
    const values = stats.dailyModelTokens.map((d) => {
      return Object.values(d.tokensByModel).reduce((a, b) => a + b, 0);
    });
    const labels = stats.dailyModelTokens.map((d) => formatShortDate(d.date));
    renderMiniBar(canvas, values, labels);
  } catch (e) {
    console.error("Failed to load data:", e);
  } finally {
    schedulePopupResize();
  }
}

// Live updates
listen("stats-updated", () => loadData());

window.addEventListener("resize", schedulePopupResize);

if ("fonts" in document) {
  void (document as Document & { fonts: { ready: Promise<unknown> } }).fonts.ready.then(() => {
    schedulePopupResize();
  });
}

// Initial load
loadData();
schedulePopupResize();
