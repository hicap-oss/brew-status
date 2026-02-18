# Brew Status

A Windows and macOS tray/menu bar app for monitoring your Claude Code usage in real time. See token consumption, rate limits, session stats, and activity trends at a glance.

![Built with Tauri](https://img.shields.io/badge/Built_with-Tauri_2-FFC131?logo=tauri)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS_(Apple%20Silicon)-0078D4)

## Features

- **System Tray / Menu Bar** - Lives in your tray (Windows) or top menu bar (macOS). Left-click for a quick popup, right-click for menu.
- **Live Rate Limits** - Track your session, weekly, Sonnet, and Opus quotas with color-coded progress bars (green/yellow/red) and reset countdowns.
- **Token Breakdown** - See input, output, cache read, and cache creation tokens per model (Opus, Sonnet, Haiku).
- **Activity Charts** - 7-day bar chart (tokens, messages, or tool calls) and a 24-hour activity heatmap.
- **Session Stats** - Total sessions, messages, tool calls, first session date, and longest session.
- **Recent History** - Last 50 conversations with timestamps, projects, and message previews.
- **Real-Time Updates** - Watches `~/.claude/` for changes and refreshes automatically.
- **Auto-Update** - Checks for new versions every 24 hours with one-click install.

## Install

Download the latest release from [Releases](https://github.com/hicap-oss/brew-status/releases/latest):

- **Windows:** `.msi` installer
- **macOS (Apple Silicon):** `.dmg` (unsigned test build)

For unsigned macOS test builds, install from the `.dmg` by dragging `brew-status.app` to `/Applications`.

If macOS shows `"brew-status" is damaged and can't be opened`, clear quarantine and launch:

```sh
xattr -dr com.apple.quarantine "/Applications/brew-status.app"
open "/Applications/brew-status.app"
```

If it still fails, run the binary directly once to print the real error:

```sh
"/Applications/brew-status.app/Contents/MacOS/brew-status"
```

## How It Works

Brew Status reads data that Claude Code already writes:

- **`~/.claude/history.jsonl`** for recent conversation history
- **Session files** under `~/.claude/projects/` for usage aggregation
- **`~/.claude/stats-cache.json`** when available (older/newer clients may omit this)
- **OAuth token source for live limits/profile:**
  - macOS: Claude Desktop token cache decrypted with macOS Keychain (`Claude Safe Storage`)
  - Windows/Linux: `~/.claude/.credentials.json`

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

Produces platform bundles in `src-tauri/target/release/bundle/` (for example, `.msi` on Windows and `.dmg`/`.app` on macOS).

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
