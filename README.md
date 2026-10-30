<div align="center">

# ⚡ ProcDash (`procdash`)

**Blazingly fast Terminal UI (TUI) & CLI for inspecting active ports, monitoring real-time CPU/RAM telemetry, and killing port-hogging processes with a single keystroke.**

[![Rust](https://img.shields.io/badge/Built_with-Rust_🦀-orange?style=for-the-badge)](https://www.rust-lang.org/)
[![Ratatui](https://img.shields.io/badge/TUI-Ratatui_0.29-00d7ff?style=for-the-badge)](https://ratatui.rs/)
[![Platform](https://img.shields.io/badge/Platform-Windows_%7C_Linux_%7C_macOS-success?style=for-the-badge)](#installation)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](LICENSE)

</div>

---

## ✨ Preview

```text
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│ ⚡ PROCDASH // PORTKILLER  ● LIVE   Host: dev-station (Windows)  Procs: 248  Sockets: 18/94       │
├────────────────────────────────────────────────┬─────────────────────────────────────────────────┤
│ CPU LOAD [ 14.2%] (16 Cores)                   │ MEMORY USAGE [11.4 / 32.0 GB (36%)]             │
│ ███████░░░░░░░░░░░░░░░░░ 14.2% Total CPU       │ █████████░░░░░░░░░░░░░░░ 11.40 GB | Swap: 0.2GB │
│ ▁▂▃▂▄▃▂▂▁▂▃▅▄▃▂▂▂▂▃▄▃▂▁▂▃▂▂▁▂▃▄▃▂▂▁▂▂▃▂▁▂▃▂    │ ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄ │
├────────────────────────────────────────────────┴─────────────────────────────────────────────────┤
│ 🔎 Filter [/]: port, process, PID...   [l] LISTEN ONLY  [d] All Ports  [p] TCP+UDP  [s/r] PORT ▲ │
├─────────────────────────────────────────────────────────────────┬────────────────────────────────┤
│ ACTIVE PORTS & PROCESSES                                        │ PROCESS INSPECTOR              │
│ PORT       PROTO  SERVICE          PID      PROCESS   CPU%  RAM │ Process:  node.exe             │
│ ▶ ★ :3000  TCP    Node/React Dev   18420    node.exe  4.2%  142 │ PID:      18420  Uptime: 14m   │
│   ★ :5173  TCP    Vite Dev         21094    node.exe  1.8%  118 │ Port:     :3000 (TCP)          │
│   ★ :5432  TCP    PostgreSQL       4912     postgres  0.4%   86 │ Service:  Node/React Dev       │
│   ★ :6379  TCP    Redis            6108     redis     0.1%   24 │ Bind:     127.0.0.1:3000       │
│   ★ :8080  TCP    HTTP Proxy/Dev   15332    docker    0.9%  210 │ All PID Ports: :3000, :9229    │
│     :443   TCP    HTTPS            9912     nginx     0.0%   18 │                                │
│                                                                 │ [k / Del] Kill Process         │
└─────────────────────────────────────────────────────────────────┴────────────────────────────────┘
 [↑/↓] Navigate  [/] Search  [k/Del] Kill PID  [l] Listen/All  [d] Dev Ports  [p] Proto  [q] Quit
```

---

## 🚀 Features

- **⚡ Instant Port-to-Process Mapping:** See exactly which process (`PID`, executable name, full command line) is occupying any TCP or UDP port (`IPv4` & `IPv6`).
- **🎯 One-Key PortKiller:** Select a stuck server (`:3000`, `:5173`, `:8080`) and press `k` or `Delete` (with a safety confirmation modal) — or run `procdash --kill 3000` directly from your shell.
- **📈 Real-Time CPU & Memory Sparklines:** Animated history graphs and color-coded load gauges for global CPU, RAM, Swap, and per-process CPU/RAM/Disk I/O metrics.
- **★ Smart Developer Port Detection:** Automatically highlights common development ports (`Node/React :3000`, `Vite :5173`, `PostgreSQL :5432`, `Redis :6379`, `MongoDB :27017`, `Ollama :11434`, `FastAPI/Django :8000`, etc.) and lets you filter exclusively for dev ports with `d`.
- **🔍 Live Fuzzy Filtering & Multi-Column Sorting:** Press `/` to filter in real-time by port number, process name, service tag, or PID. Sort by `PORT`, `CPU%`, `RAM`, `PID`, or `PROCESS`.
- **🛠 Dual Mode (Interactive TUI + Scriptable CLI):** Launch the full terminal dashboard or query/terminate ports non-interactively in scripts.

---

## 📦 Installation

### Build from Source (Cargo)

```bash
git clone https://github.com/myhticc/procdash.git
cd procdash
cargo install --path .
```

Or run directly in release mode:

```bash
cargo run --release
```

---

## 💻 Usage (TUI & CLI)

### 1. Launch Interactive TUI Dashboard
```bash
procdash
```

### 2. Launch TUI Pre-Filtered to a Port or Process
```bash
procdash --filter 3000
procdash --filter node
```

### 3. Kill a Process on a Specific Port Immediately (CLI Mode)
```bash
procdash --kill 3000
# Output: ✔ Terminated node.exe (PID 18420) on port :3000 (TCP)
```

### 4. Inspect Who Is Using a Specific Port (CLI Mode)
```bash
procdash --port 8080
```

### 5. Print Snapshot Table to Terminal (CLI Mode)
```bash
procdash --list
procdash --list --filter python
```

---

## ⌨️ Keyboard Shortcuts (TUI)

| Key | Action |
| :--- | :--- |
| `↑` / `↓` or `j` / `u` | Move selection up / down |
| `PgUp` / `PgDn` | Jump 10 rows up / down |
| `/` | Focus live search bar (filter by port, process, PID, service) |
| `Esc` | Clear search filter or close active modal |
| `k` or `Delete` | **Kill selected process** (opens confirmation modal) |
| `l` | Toggle **LISTEN Only** vs **All Socket States** (`ESTABLISHED`, `TIME_WAIT`, etc.) |
| `d` | Toggle **★ Developer Ports Only** (`3000`, `5173`, `8080`, `5432`, `6379`, etc.) |
| `p` | Cycle protocol filter (`TCP+UDP` → `TCP Only` → `UDP Only`) |
| `s` or `Tab` | Cycle sort column (`PORT` → `CPU%` → `RAM` → `PID` → `PROCESS`) |
| `r` | Reverse sort order (`▲ Ascending` / `▼ Descending`) |
| `Space` | Pause / Resume live telemetry updates |
| `?` or `F1` | Open keyboard shortcuts help overlay |
| `q` or `Ctrl+C` | Quit ProcDash |

---

## 🇹🇷 Türkçe Özet

**ProcDash**, bilgisayarınızda hangi portun (`TCP`/`UDP`) hangi uygulama tarafından işgal edildiğini gerçek zamanlı gösteren, tek tuşla (`k` veya `Delete`) ilgili işlemi sonlandıran (PortKiller) ve anlık **CPU / RAM** kullanım grafiklerini (Sparkline) çizen **Rust** tabanlı bir Terminal Arayüzü (TUI) ve komut satırı (CLI) aracıdır.

- **TUI Başlatma:** `procdash`
- **Belirli Portu Anında Kapatma:** `procdash --kill 3000`
- **Belirli Portu Sorgulama:** `procdash --port 5173`
- **Liste Görünümü:** `procdash --list`

---

## 🤝 Contributors

- **[@myhticc](https://github.com/myhticc)** — Creator & Maintainer
- **[@claude](https://github.com/claude)** — AI Co-Author (Anthropic)

---

## 📄 License

Distributed under the [MIT License](LICENSE).
