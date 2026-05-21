# RustProc — Real-Time Process Monitoring CLI
## Comprehensive Implementation Plan

---

## 1. Project Overview

RustProc is a terminal-based system monitor written in Rust. It renders live system telemetry — CPU, RAM, and per-process statistics — inside a terminal UI that refreshes every second. The goal is a production-quality, demo-safe CLI tool that is visually polished, technically defensible, and straightforward to extend.

The stack is deliberately minimal: three well-maintained crates with no transitive surprises, no unsafe code, and no external services required. The result runs on any machine with `cargo` installed and exits cleanly on a single keypress.

---

## 2. Technical Stack

| Layer | Crate | Version | Role |
|---|---|---|---|
| System telemetry | `sysinfo` | 0.30 | CPU, RAM, process enumeration |
| Terminal UI framework | `ratatui` | 0.26 | Widgets, layout engine, rendering |
| Terminal backend | `crossterm` | 0.27 | Raw mode, alternate screen, input events |

All three crates are pure Rust, cross-platform (Linux, macOS, Windows), and do not require system headers beyond what `cargo build` resolves automatically.

---

## 3. Repository Structure

```
rustproc/
├── Cargo.toml
├── Cargo.lock               # commit this — ensures reproducible demo builds
├── README.md
└── src/
    ├── main.rs              # entry point: terminal setup, event loop
    ├── app.rs               # App state struct and update logic
    ├── ui.rs                # all ratatui rendering logic
    └── system.rs            # sysinfo abstraction layer
```

Keeping rendering, state, and system calls in separate modules means each file has one reason to change. `main.rs` becomes a thin orchestrator: initialise → loop → teardown.

---

## 4. Cargo.toml

```toml
[package]
name = "rustproc"
version = "0.1.0"
edition = "2021"

[dependencies]
sysinfo   = "0.30"
ratatui   = "0.26"
crossterm = "0.27"

[profile.release]
opt-level = 3
strip = true        # strips debug symbols → smaller binary for demo
```

> **Why `strip = true`?** Reduces the release binary from ~8 MB to ~2 MB. Looks cleaner if you `ls -lh target/release/rustproc` during a demo.

---

## 5. Architecture

### 5.1 Data Flow

```
sysinfo::System::refresh_all()
        │
        ▼
  system.rs — collects raw values
        │
        ▼
  app.rs (AppState) — holds snapshot for current frame
        │
        ▼
  ui.rs — converts AppState into ratatui widgets
        │
        ▼
  terminal.draw() — renders frame to alternate screen
```

Each tick, `refresh_all()` is called once, the state struct is updated, and the UI redraws from that snapshot. There is no shared mutable state across threads — the entire loop is single-threaded and deterministic.

### 5.2 AppState Struct

```rust
// src/app.rs
pub struct AppState {
    pub cpu_usage: f32,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub process_count: usize,
    pub processes: Vec<ProcessRow>,
    pub scroll_offset: usize,   // improvement: scrollable process table
}

pub struct ProcessRow {
    pub pid: String,
    pub name: String,
    pub cpu: f32,
    pub memory_mb: u64,
}
```

Separating the data model from both the system layer and the rendering layer makes the project trivially testable: you can construct an `AppState` with synthetic data and assert the UI renders it correctly, without touching real system calls.

### 5.3 System Abstraction (`system.rs`)

```rust
// src/system.rs
use sysinfo::{System, ProcessesToUpdate};

pub fn collect(sys: &mut System) -> crate::app::AppState {
    sys.refresh_all();

    let cpu_usage = sys.global_cpu_info().cpu_usage();
    let total_memory_mb = sys.total_memory() / 1024 / 1024;
    let used_memory_mb  = sys.used_memory()  / 1024 / 1024;

    let mut processes: Vec<_> = sys
        .processes()
        .iter()
        .map(|(pid, p)| crate::app::ProcessRow {
            pid:       pid.to_string(),
            name:      p.name().to_string(),
            cpu:       p.cpu_usage(),
            memory_mb: p.memory() / 1024 / 1024,
        })
        .collect();

    // Sort by CPU descending so the most active processes surface first
    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));

    crate::app::AppState {
        cpu_usage,
        total_memory_mb,
        used_memory_mb,
        process_count: processes.len(),
        processes,
        scroll_offset: 0,
    }
}
```

> **Improvement over baseline:** The original code calls `.take(15)` without sorting. This means the 15 processes shown are arbitrary. Sorting by CPU descending before truncating ensures the table always shows the most relevant processes.

---

## 6. UI Layout (`ui.rs`)

### 6.1 Layout Design

```
┌─────────────────────────────────────────────────┐
│  RustProc — System Stats                        │
│  CPU Usage:  12.40%                             │
│  RAM Usage:  4821 MB / 16384 MB  ████░░░░ 29%  │
│  Processes:  312                                │
├─────────────────────────────────────────────────┤
│  Processes                          [q] Quit    │
│  PID      │ Process Name      │ CPU % │ Mem MB │
│  ─────────┼───────────────────┼───────┼─────── │
│  1482     │ chrome            │  8.20 │   512  │
│  3301     │ code              │  4.10 │   310  │
│  ...                                            │
└─────────────────────────────────────────────────┘
```

### 6.2 Improvements Over Baseline

| Baseline | Improved |
|---|---|
| Plain text CPU/RAM stats | ASCII progress bar for RAM usage |
| Unsorted 15-item slice | Sorted by CPU %, top N visible |
| No scroll support | Arrow key scrolling through full process list |
| No keyboard hint | `[q] Quit` shown in process table title |
| No colour | CPU bar turns yellow >50%, red >80% |

### 6.3 Rendering Function Skeleton

```rust
// src/ui.rs
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, Paragraph, Row, Table},
};
use crate::app::AppState;

pub fn render(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(6),   // stats panel
            Constraint::Min(10),     // process table
        ])
        .split(f.size());

    render_stats(f, state, chunks[0]);
    render_processes(f, state, chunks[1]);
}

fn ram_color(used: u64, total: u64) -> Color {
    let pct = used as f64 / total as f64;
    if pct > 0.80 { Color::Red }
    else if pct > 0.50 { Color::Yellow }
    else { Color::Green }
}
```

The colour-coding for RAM and CPU makes demo output immediately legible to an audience watching a screen recording. Green → Yellow → Red provides an intuitive load indicator without any explanation needed.

---

## 7. Event Loop (`main.rs`)

### 7.1 Timing Strategy

The baseline does:

```rust
thread::sleep(Duration::from_secs(1));
```

This is fine for a demo, but means input polling blocks for up to 1 second after a keypress. The improved approach polls in a tight loop with a short timeout and tracks elapsed time to decide when to refresh:

```rust
// src/main.rs — improved tick loop
use std::time::{Duration, Instant};

let tick_rate = Duration::from_secs(1);
let mut last_tick = Instant::now();

loop {
    if event::poll(Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Down      => state.scroll_offset = state.scroll_offset.saturating_add(1),
                KeyCode::Up        => state.scroll_offset = state.scroll_offset.saturating_sub(1),
                _                  => {}
            }
        }
    }

    if last_tick.elapsed() >= tick_rate {
        state = system::collect(&mut sys);
        last_tick = Instant::now();
    }

    terminal.draw(|f| ui::render(f, &state))?;
}
```

This makes `q` respond in under 50 ms regardless of where in the tick cycle the keypress happens. Arrow key scrolling also works without waiting for a refresh cycle.

### 7.2 Terminal Setup and Teardown

Setup and teardown must be symmetric or the terminal is left in raw mode on crash. The pattern below uses a dedicated function and calls it from both the normal exit path and any error handler:

```rust
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}
```

Call `teardown_terminal` both at the end of `main` and inside any `Err` branch to avoid corrupting the shell session after a panic during demo.

---

## 8. Implementation Phases

### Phase 1 — Skeleton (30 minutes)

Goal: compile and run with static placeholder data.

- Create project with `cargo new rustproc`
- Add all three dependencies to `Cargo.toml`
- Implement `setup_terminal` / `teardown_terminal` in `main.rs`
- Create `ui.rs` with a single `Paragraph` widget showing hardcoded text
- Confirm `cargo run` opens alternate screen and exits cleanly on `q`

Success criterion: you can press `q` and your terminal is unmodified afterward.

### Phase 2 — Live System Data (30 minutes)

Goal: real numbers in the stats panel.

- Implement `system.rs` with the `collect()` function
- Populate `AppState` from real sysinfo data
- Wire `collect()` into the main loop tick
- Update `ui.rs` stats panel to render live CPU and RAM values

Success criterion: CPU number changes between ticks; RAM total matches `free -h` output.

### Phase 3 — Process Table (30 minutes)

Goal: scrollable, sorted process table.

- Extend `ui.rs` with the `Table` widget
- Add CPU-descending sort in `system.rs`
- Implement `scroll_offset` in `AppState`
- Handle `KeyCode::Up` / `KeyCode::Down` in the event loop

Success criterion: you can scroll the process list; top row is always the highest CPU consumer.

### Phase 4 — Visual Polish (20 minutes)

Goal: colours, progress bar, keyboard hints.

- Add `Gauge` widget for RAM (coloured by threshold)
- Apply `Color::Yellow` / `Color::Red` styling to CPU % column rows above 50% / 80%
- Add `[↑↓] Scroll  [q] Quit` to the process table block title

Success criterion: demo looks visually distinct from a default terminal app at a glance.

### Phase 5 — Hardening (20 minutes)

Goal: zero crashes during demo.

- Wrap all `sysinfo` field accesses defensively (process name can be empty on some Linux kernels — default to `"[unknown]"`)
- Ensure `teardown_terminal` is called even if `terminal.draw()` returns an error
- Run `cargo clippy` and resolve all warnings
- Test `cargo run --release` and confirm the release binary works identically

Success criterion: `cargo clippy -- -D warnings` exits 0.

---

## 9. Potential Extensions (Post-Demo)

These are not required for the core submission but each takes under 2 hours and substantially upgrades the project story:

| Extension | What it adds | Relevant crate |
|---|---|---|
| CPU-per-core graph | Sparkline widget per logical core | ratatui `Sparkline` |
| Process filter | `/` to enter a search string, filter table in real time | built-in |
| Disk I/O stats | Read/write MB/s per disk | `sysinfo` `disks()` |
| Network throughput | RX/TX bytes per interface | `sysinfo` `networks()` |
| Config file | Tick rate, max rows, colour thresholds via TOML | `toml` + `serde` |
| Mouse support | Click a row to highlight, scroll wheel support | `crossterm` mouse events |

---

## 10. Build and Run Reference

```bash
# Development build (fast compile, debug symbols)
cargo run

# Release build (optimised, stripped binary)
cargo build --release
./target/release/rustproc

# Lint check (run before demo)
cargo clippy -- -D warnings

# Format check
cargo fmt --check
```

Keybindings at runtime:

| Key | Action |
|---|---|
| `q` | Quit cleanly |
| `↑` / `↓` | Scroll process table |

---

## 11. Demo Script (2-Minute Walkthrough)

1. **Show the source structure** — four small files, each with a single responsibility.
2. **`cargo run`** — dashboard opens immediately.
3. **Point at CPU row** — explain that `sysinfo` polls the OS kernel, same data source as `htop`.
4. **Open a CPU-heavy process** in another terminal (e.g. `stress --cpu 2`) — live number climbs, colour shifts yellow/red.
5. **Scroll the process table** — demonstrate keyboard input is handled independently of the refresh tick.
6. **Press `q`** — terminal restored cleanly, cursor visible, no artefacts.
7. **Mention extensions** — network stats, per-core sparklines, config file are all drop-in additions within the same architecture.

---

## 12. Checklist

- [ ] `cargo build --release` exits 0
- [ ] `cargo clippy -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `q` exits cleanly from both normal run and after a scroll interaction
- [ ] Process table is sorted by CPU descending
- [ ] RAM gauge changes colour above 50% and 80%
- [ ] Tested on the actual demo machine at least once before presentation
