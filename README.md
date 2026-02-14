# Brew Status

A Windows system tray app for monitoring your Claude Code usage in real time. See token consumption, rate limits, session stats, and activity trends at a glance.

![Built with Tauri](https://img.shields.io/badge/Built_with-Tauri_2-FFC131?logo=tauri)
![Platform](https://img.shields.io/badge/Platform-Windows-0078D4?logo=windows)

## Features

- **System Tray** - Lives in your tray. Left-click for a quick popup, right-click for menu.
- **Live Rate Limits** - Track your session, weekly, Sonnet, and Opus quotas with color-coded progress bars (green/yellow/red) and reset countdowns.
- **Token Breakdown** - See input, output, cache read, and cache creation tokens per model (Opus, Sonnet, Haiku).
- **Activity Charts** - 7-day bar chart (tokens, messages, or tool calls) and a 24-hour activity heatmap.
- **Session Stats** - Total sessions, messages, tool calls, first session date, and longest session.
- **Recent History** - Last 50 conversations with timestamps, projects, and message previews.
- **Real-Time Updates** - Watches `~/.claude/` for changes and refreshes automatically.
- **Auto-Update** - Checks for new versions every 24 hours with one-click install.

## Install

Download the latest `.msi` installer from [Releases](https://github.com/hicap-oss/brew-status/releases/latest).

## How It Works

Brew Status reads data that Claude Code already writes to your `~/.claude/` directory:

- **`stats-cache.json`** for aggregated usage statistics
- **`history.jsonl`** for recent conversation history
- **Session files** under `projects/` for today's token counts
- **`.credentials.json`** OAuth token to fetch live rate limits from the Anthropic API

A file watcher detects changes and pushes updates to the UI in real time.

## Two Views

**Popup** - A compact panel near the tray icon with three tabs: rate limits, token usage, and today's stats.

**Dashboard** - A full window with all panels: limits, token breakdown, session stats, hourly heatmap, 7-day chart, history, and an About section with manual update check.

## Development

### Prerequisites

- [Node.js](https://nodejs.org/) 20+
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install)

### Run

```sh
pnpm install
pnpm tauri dev
```

### Build

```sh
pnpm tauri build
```

Produces an `.msi` installer in `src-tauri/target/release/bundle/`.

## Tech Stack

| Layer | Tech |
|-------|------|
| Framework | Tauri 2 |
| Frontend | TypeScript, HTML/CSS, Canvas API |
| Backend | Rust |
| Build | Vite, pnpm |
| CI/CD | GitHub Actions |

## License

MIT
