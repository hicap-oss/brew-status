const COLORS = {
  bar: "#7c3aed",
  barHover: "#9b5de5",
  gridLine: "#2a2a4a",
  label: "#8888aa",
  text: "#e0e0e0",
  heatmapEmpty: "#1e1e3a",
  heatmapLow: "#3b2070",
  heatmapMid: "#6d28d9",
  heatmapHigh: "#a78bfa",
};

function setupCanvas(
  canvas: HTMLCanvasElement,
  width: number,
  height: number
): CanvasRenderingContext2D {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = width * dpr;
  canvas.height = height * dpr;
  canvas.style.width = width + "px";
  canvas.style.height = height + "px";
  const ctx = canvas.getContext("2d")!;
  ctx.scale(dpr, dpr);
  return ctx;
}

export function renderBarChart(
  canvas: HTMLCanvasElement,
  labels: string[],
  values: number[],
  opts: { width?: number; height?: number; color?: string } = {}
): void {
  const width = opts.width ?? (canvas.clientWidth || 400);
  const height = opts.height ?? (canvas.clientHeight || 200);
  const ctx = setupCanvas(canvas, width, height);

  const padding = { top: 20, right: 15, bottom: 30, left: 55 };
  const chartW = width - padding.left - padding.right;
  const chartH = height - padding.top - padding.bottom;
  const maxVal = Math.max(...values, 1);
  const barWidth = Math.min(chartW / labels.length - 8, 40);

  ctx.clearRect(0, 0, width, height);

  // Grid lines
  ctx.strokeStyle = COLORS.gridLine;
  ctx.lineWidth = 0.5;
  for (let i = 0; i <= 4; i++) {
    const y = padding.top + (chartH / 4) * i;
    ctx.beginPath();
    ctx.moveTo(padding.left, y);
    ctx.lineTo(width - padding.right, y);
    ctx.stroke();

    ctx.fillStyle = COLORS.label;
    ctx.font = "11px system-ui";
    ctx.textAlign = "right";
    const label = formatAxisLabel(maxVal - (maxVal / 4) * i);
    ctx.fillText(label, padding.left - 8, y + 4);
  }

  // Bars
  const gap = chartW / labels.length;
  for (let i = 0; i < values.length; i++) {
    const barH = (values[i] / maxVal) * chartH;
    const x = padding.left + gap * i + (gap - barWidth) / 2;
    const y = padding.top + chartH - barH;

    const gradient = ctx.createLinearGradient(x, y, x, y + barH);
    gradient.addColorStop(0, opts.color ?? COLORS.bar);
    gradient.addColorStop(1, adjustAlpha(opts.color ?? COLORS.bar, 0.6));
    ctx.fillStyle = gradient;

    roundRect(ctx, x, y, barWidth, barH, 3);

    // Label
    ctx.fillStyle = COLORS.label;
    ctx.font = "10px system-ui";
    ctx.textAlign = "center";
    ctx.fillText(labels[i], x + barWidth / 2, height - 8);
  }
}

export function renderMiniBar(
  canvas: HTMLCanvasElement,
  values: number[],
  labels: string[]
): void {
  const width = canvas.clientWidth || 280;
  const height = canvas.clientHeight || 60;
  const ctx = setupCanvas(canvas, width, height);

  const maxVal = Math.max(...values, 1);
  const barWidth = Math.min(width / values.length - 4, 30);
  const gap = width / values.length;

  ctx.clearRect(0, 0, width, height);

  for (let i = 0; i < values.length; i++) {
    const barH = Math.max((values[i] / maxVal) * (height - 18), 2);
    const x = gap * i + (gap - barWidth) / 2;
    const y = height - 16 - barH;

    const gradient = ctx.createLinearGradient(x, y, x, y + barH);
    gradient.addColorStop(0, COLORS.bar);
    gradient.addColorStop(1, adjustAlpha(COLORS.bar, 0.5));
    ctx.fillStyle = gradient;
    roundRect(ctx, x, y, barWidth, barH, 2);

    ctx.fillStyle = COLORS.label;
    ctx.font = "9px system-ui";
    ctx.textAlign = "center";
    ctx.fillText(labels[i], x + barWidth / 2, height - 3);
  }
}

export function renderHourlyHeatmap(
  canvas: HTMLCanvasElement,
  hourCounts: Record<string, number>
): void {
  const width = canvas.clientWidth || 400;
  const height = canvas.clientHeight || 50;
  const ctx = setupCanvas(canvas, width, height);

  const maxCount = Math.max(...Object.values(hourCounts), 1);
  const cellW = (width - 40) / 24;
  const cellH = height - 22;
  const offsetX = 20;

  ctx.clearRect(0, 0, width, height);

  for (let h = 0; h < 24; h++) {
    const count = hourCounts[h.toString()] || 0;
    const intensity = count / maxCount;
    const x = offsetX + h * cellW;

    if (intensity === 0) {
      ctx.fillStyle = COLORS.heatmapEmpty;
    } else if (intensity < 0.33) {
      ctx.fillStyle = COLORS.heatmapLow;
    } else if (intensity < 0.66) {
      ctx.fillStyle = COLORS.heatmapMid;
    } else {
      ctx.fillStyle = COLORS.heatmapHigh;
    }

    roundRect(ctx, x + 1, 0, cellW - 2, cellH, 3);

    if (h % 6 === 0) {
      ctx.fillStyle = COLORS.label;
      ctx.font = "9px system-ui";
      ctx.textAlign = "center";
      ctx.fillText(`${h}:00`, x + cellW / 2, height - 3);
    }
  }
}

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number
): void {
  if (h <= 0) return;
  r = Math.min(r, h / 2, w / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
  ctx.fill();
}

function formatAxisLabel(val: number): string {
  if (val >= 1_000_000) return (val / 1_000_000).toFixed(1) + "M";
  if (val >= 1_000) return (val / 1_000).toFixed(0) + "K";
  return Math.round(val).toString();
}

function adjustAlpha(hex: string, alpha: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
