# RustProc Shell — Unix-Like Shell with Integrated System Monitor
## Comprehensive Implementation Plan

---

## 1. What This Project Actually Is

RustProc Shell is a Rust CLI application that occupies the space between a system monitor and a Unix shell. The terminal window is permanently split into two zones: a live telemetry pane on top that refreshes every second, and an interactive command prompt at the bottom that accepts and executes real shell commands. The user never leaves the program to run a command — they type directly below the stats they are watching.

This is not a gimmick. The combination is architecturally meaningful: the shell runs child processes via `fork`/`exec` (on Linux) or `CreateProcess` (on Windows), which are the same mechanisms the OS uses to manage everything visible in the process table above. Demonstrating that linkage — type a command, watch a new PID appear in the table, watch CPU spike, watch it disappear on exit — is a genuinely compelling OS concepts demonstration.

---

## 2. Screen Layout (Final)

```
╔══════════════════════════════════════════════════════════════════════════╗
║  RustProc Shell v0.2  │  CPU: 34%  RAM: 6.1/32 GB  GPU: 41%  PIDs: 211 ║
╠══════════════════════════════════════════════════════════════════════════╣
║                                                                          ║
║  ┌── System Stats ─────────────────────────────────────────────────┐    ║
║  │  CPU  ███████░░░░░░  34%    Temp: 61°C   Freq: 3.9 GHz         │    ║
║  │  RAM  ████████░░░░░  47%    6.1 GB / 32 GB                      │    ║
║  │  GPU  ████░░░░░░░░░  28%    VRAM: 3.2 / 16 GB   Temp: 52°C     │    ║
║  └─────────────────────────────────────────────────────────────────┘    ║
║                                                                          ║
║  ┌── Processes (top 8 by CPU) ─────────────────────────────────────┐    ║
║  │  PID     Name              CPU%    Mem MB   Status              │    ║
║  │  4821    chrome            8.2     1024     running             │    ║
║  │  3301    code              4.1      512     running             │    ║
║  │  1482    python            2.9      128     running             │    ║
║  │  ...                                                             │    ║
║  └─────────────────────────────────────────────────────────────────┘    ║
║                                                                          ║
╠══════════════════════════════════════════════════════════════════════════╣
║  Shell Output                                                            ║
║  $ ls -la                                                                ║
║  total 48                                                                ║
║  drwxr-xr-x  3 user user 4096 May 21 11:04 .                            ║
║  drwxr-xr-x 42 user user 4096 May 21 10:55 ..                           ║
║  -rw-r--r--  1 user user  312 May 21 11:04 main.rs                      ║
║                                                                          ║
╠══════════════════════════════════════════════════════════════════════════╣
║  RustProc> █                                                             ║
╚══════════════════════════════════════════════════════════════════════════╝
```

Three permanent zones, always visible:
- **Top** — live hardware stats (refreshes every second in the background)
- **Middle** — last command's output (scrollable)
- **Bottom** — input prompt (always focused)

---

## 3. Technical Architecture

### 3.1 The Core Problem

A system monitor and a shell have fundamentally incompatible execution models:

- A monitor wants to loop forever, redrawing the screen every tick
- A shell wants to block waiting for user input, then block again while a child process runs

The solution is to separate them by thread:

```
Main Thread
    │
    ├── Monitor Thread (background)
    │       Runs every 1s
    │       Calls sysinfo refresh
    │       Writes to Arc<Mutex<AppState>>
    │
    └── Shell Thread (foreground — main thread)
            Reads input from user
            Reads AppState to draw header
            Executes commands
            Redraws screen after each command
```

The monitor thread never blocks user input. The shell thread redraws the stats pane every time it redraws the screen — which happens after each command and after each tick. This gives the appearance of live stats without any async complexity.

### 3.2 Shared State

```rust
// src/state.rs

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub cpu_usage:       f32,
    pub cpu_temp_c:      Option<f32>,
    pub cpu_freq_mhz:    u64,
    pub ram_used_mb:     u64,
    pub ram_total_mb:    u64,
    pub gpu_usage:       Option<f32>,
    pub vram_used_mb:    Option<u64>,
    pub vram_total_mb:   Option<u64>,
    pub gpu_temp_c:      Option<f32>,
    pub process_count:   usize,
    pub processes:       Vec<ProcessRow>,
    pub cwd:             String,       // current working directory for prompt
}

#[derive(Debug, Clone, Default)]
pub struct ProcessRow {
    pub pid:     u32,
    pub name:    String,
    pub cpu_pct: f32,
    pub mem_mb:  u64,
    pub status:  String,
}

pub type SharedState = Arc<Mutex<AppState>>;
```

`Arc<Mutex<AppState>>` is the idiomatic Rust pattern for shared mutable state across threads. The monitor thread holds the write lock only for the brief moment it updates values. The shell thread holds the read lock only while drawing. Neither blocks the other in any meaningful way.

### 3.3 Module Structure

```
rustproc-shell/
├── Cargo.toml
├── Cargo.lock
└── src/
    ├── main.rs           — startup: spawn monitor thread, enter shell loop
    ├── state.rs          — AppState, ProcessRow, SharedState type alias
    ├── monitor.rs        — background thread: sysinfo polling, state update
    ├── shell.rs          — REPL: input reading, command dispatch
    ├── builtins.rs       — built-in commands: cd, help, exit, clear, sysinfo
    ├── executor.rs       — external command execution via std::process
    ├── renderer.rs       — all terminal drawing: header, process table, prompt
    └── history.rs        — command history: up/down arrow, persistence
```

Every file has one job. `main.rs` wires them together and is under 50 lines.

---

## 4. Crate Selection

| Crate | Version | Role |
|---|---|---|
| `sysinfo` | 0.30 | CPU, RAM, process data |
| `crossterm` | 0.27 | Terminal control: cursor, colour, clear, raw mode |
| `nvml-wrapper` | 0.9 | NVIDIA GPU stats (optional feature) |
| `dirs` | 5.0 | `~` expansion, home directory resolution |
| `serde` + `toml` | 1.0 / 0.8 | Config file parsing |

Notably absent: `ratatui`. The shell UI is much simpler than the pure monitor because the layout does not animate — it redraws on demand. Direct `crossterm` calls are cleaner here than a full TUI framework. The prompt area uses raw terminal output. This also avoids the `ratatui` alternate screen, which complicates interleaving subprocess output.

---

## 5. Detailed Module Specifications

### 5.1 `main.rs` — Entry Point

```rust
// src/main.rs

fn main() {
    // 1. Load config (tick rate, GPU feature flag, colour thresholds)
    let config = config::load();

    // 2. Initialise shared state
    let state: SharedState = Arc::new(Mutex::new(AppState::default()));

    // 3. Spawn monitor thread
    let state_clone = Arc::clone(&state);
    thread::spawn(move || monitor::run(state_clone, config.tick_rate_ms));

    // 4. Enter shell REPL — this blocks until the user types 'exit'
    shell::run(Arc::clone(&state), config);
}
```

The monitor thread is a daemon: when the main thread exits (user typed `exit`), the monitor thread is killed automatically by the OS. No cleanup required.

### 5.2 `monitor.rs` — Background Thread

```rust
// src/monitor.rs

pub fn run(state: SharedState, tick_ms: u64) {
    let mut sys = System::new_all();

    loop {
        sys.refresh_all();

        let cpu_usage = sys.global_cpu_info().cpu_usage();
        let ram_used  = sys.used_memory()  / 1024 / 1024;
        let ram_total = sys.total_memory() / 1024 / 1024;

        let mut processes: Vec<ProcessRow> = sys.processes()
            .iter()
            .map(|(pid, p)| ProcessRow {
                pid:     pid.as_u32(),
                name:    p.name().to_string(),
                cpu_pct: p.cpu_usage(),
                mem_mb:  p.memory() / 1024 / 1024,
                status:  format!("{:?}", p.status()),
            })
            .collect();

        processes.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap_or(Equal));

        // GPU collection (NVIDIA via NVML or sysfs fallback)
        let (gpu_usage, vram_used, vram_total, gpu_temp) = gpu::collect();

        // CPU temp (Linux hwmon / sysinfo)
        let cpu_temp = temp::cpu_package();

        // Write to shared state — lock held < 1ms
        {
            let mut s = state.lock().unwrap();
            s.cpu_usage    = cpu_usage;
            s.ram_used_mb  = ram_used;
            s.ram_total_mb = ram_total;
            s.gpu_usage    = gpu_usage;
            s.vram_used_mb = vram_used;
            s.vram_total_mb = vram_total;
            s.gpu_temp_c   = gpu_temp;
            s.cpu_temp_c   = cpu_temp;
            s.process_count = processes.len();
            s.processes    = processes;
        }

        thread::sleep(Duration::from_millis(tick_ms));
    }
}
```

The lock is held only for the final assignment block, not during the `sysinfo` refresh. This means the shell thread can read state freely during the (relatively slow) system polling.

### 5.3 `shell.rs` — REPL

The shell loop has four responsibilities: draw the screen, read a line of input, dispatch the command, and repeat.

```rust
// src/shell.rs

pub fn run(state: SharedState, config: Config) {
    let mut history = History::new();
    let mut output_lines: Vec<String> = vec![
        "Welcome to RustProc Shell. Type 'help' for commands.".into()
    ];

    loop {
        // 1. Draw the full screen
        renderer::draw(&state, &output_lines, &config);

        // 2. Read input with history navigation
        let input = match input::readline(&mut history) {
            Ok(line) => line,
            Err(_)   => break,
        };

        let trimmed = input.trim();
        if trimmed.is_empty() { continue; }
        history.push(trimmed.to_string());

        // 3. Parse and dispatch
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let cmd  = parts[0];
        let args = parts.get(1).unwrap_or(&"");

        let result = match cmd {
            "exit" | "quit" => break,
            "cd"    => builtins::cd(args, &state),
            "help"  => builtins::help(),
            "clear" => { output_lines.clear(); continue; }
            "sysinfo" => builtins::sysinfo_snapshot(&state),
            "jobs"  => builtins::jobs(),
            _       => executor::run(trimmed),
        };

        // 4. Append result to output buffer (keep last N lines)
        match result {
            Ok(lines)  => output_lines.extend(lines),
            Err(e)     => output_lines.push(format!("error: {}", e)),
        }
        if output_lines.len() > 200 {
            output_lines.drain(0..output_lines.len() - 200);
        }
    }

    renderer::cleanup();
    println!("\nGoodbye.");
}
```

The output buffer holds the last 200 lines of shell output across all commands. This persists across commands — the user can scroll up and see the output of previous commands while still watching live stats at the top.

### 5.4 `executor.rs` — External Command Execution

This is where the OS concepts live. Running a command creates a child process. The shell waits for it synchronously (foreground), or detaches it (background with `&`).

```rust
// src/executor.rs

use std::process::{Command, Stdio};

pub fn run(input: &str) -> Result<Vec<String>, String> {
    // Detect background process request
    let (cmd_str, background) = if input.ends_with(" &") {
        (&input[..input.len()-2], true)
    } else {
        (input, false)
    };

    // Parse command + arguments
    let mut parts = cmd_str.split_whitespace();
    let cmd  = parts.next().ok_or("empty command")?;
    let args: Vec<&str> = parts.collect();

    if background {
        // Spawn and detach — do not wait
        Command::new(cmd)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn failed: {}", e))?;

        return Ok(vec![format!("[background] {}", cmd_str)]);
    }

    // Foreground: capture output and wait
    let output = Command::new(cmd)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("{}: command not found", cmd)
            } else {
                format!("{}: {}", cmd, e)
            }
        })?;

    let mut lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(String::from)
        .collect();

    if !output.stderr.is_empty() {
        lines.extend(
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .map(|l| format!("[stderr] {}", l))
        );
    }

    if let Some(code) = output.status.code() {
        if code != 0 {
            lines.push(format!("[exit {}]", code));
        }
    }

    Ok(lines)
}
```

**What this covers in terms of OS concepts:**

`Command::new(cmd).spawn()` calls `fork()` under the hood on Linux — the kernel creates a new process by duplicating the current one, then the child calls `exec()` to replace itself with the target program. `output.status` is the kernel's `waitpid()` return value. Exit codes, `stderr`, and background execution are all real Unix primitives exposed through Rust's standard library.

### 5.5 `builtins.rs` — Shell Built-ins

Built-in commands cannot be external processes because they need to modify the shell's own state (like the working directory). `cd` is the canonical example — if it were external, it would change the child's directory, not the shell's.

```rust
// src/builtins.rs

pub fn cd(args: &str, state: &SharedState) -> Result<Vec<String>, String> {
    let target = if args.is_empty() || args == "~" {
        dirs::home_dir().ok_or("could not determine home directory")?
    } else {
        std::path::PathBuf::from(shellexpand::tilde(args).as_ref())
    };

    std::env::set_current_dir(&target)
        .map_err(|e| format!("cd: {}: {}", target.display(), e))?;

    // Update the cwd in shared state so the prompt reflects the change
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    state.lock().unwrap().cwd = cwd.clone();

    Ok(vec![]) // cd produces no output on success — same as real bash
}

pub fn help() -> Result<Vec<String>, String> {
    Ok(vec![
        "RustProc Shell — built-in commands:".into(),
        "  cd [dir]     Change working directory".into(),
        "  clear        Clear shell output panel".into(),
        "  sysinfo      Print current system snapshot".into(),
        "  jobs         List background processes".into(),
        "  help         Show this message".into(),
        "  exit / quit  Exit RustProc Shell".into(),
        "".into(),
        "All other input is passed to the OS as external commands.".into(),
        "Append & to run a command in the background: sleep 10 &".into(),
    ])
}

pub fn sysinfo_snapshot(state: &SharedState) -> Result<Vec<String>, String> {
    let s = state.lock().unwrap();
    Ok(vec![
        format!("CPU:  {:.1}%  |  Temp: {}",
            s.cpu_usage,
            s.cpu_temp_c.map(|t| format!("{:.0}°C", t)).unwrap_or("N/A".into())
        ),
        format!("RAM:  {} MB / {} MB  ({:.0}%)",
            s.ram_used_mb, s.ram_total_mb,
            100.0 * s.ram_used_mb as f32 / s.ram_total_mb.max(1) as f32
        ),
        s.gpu_usage.map(|g| format!("GPU:  {:.1}%  |  Temp: {}",
            g,
            s.gpu_temp_c.map(|t| format!("{:.0}°C", t)).unwrap_or("N/A".into())
        )).unwrap_or("GPU:  N/A".into()),
        format!("PIDs: {}", s.process_count),
    ])
}
```

### 5.6 `renderer.rs` — Terminal Drawing

The renderer draws the entire screen from scratch on each call. It uses `crossterm` directly — no ratatui, no alternate screen — so subprocess output appears naturally in the output panel.

```rust
// src/renderer.rs

use crossterm::{cursor, execute, style::{Color, Print, SetForegroundColor, ResetColor}, terminal};
use std::io::{stdout, Write};

pub fn draw(state: &SharedState, output: &[String], config: &Config) {
    let s = state.lock().unwrap().clone();
    drop(state); // release lock before drawing

    let (width, height) = terminal::size().unwrap_or((80, 24));
    let output_lines    = (height as usize).saturating_sub(12); // lines for output panel
    let process_rows    = 8usize;

    let mut out = stdout();

    // Move cursor to top-left and clear screen
    execute!(out, cursor::MoveTo(0, 0), terminal::Clear(terminal::ClearType::All)).ok();

    // ── Header bar ──────────────────────────────────────────────────────────
    let gpu_str = s.gpu_usage
        .map(|g| format!("GPU: {:.0}%", g))
        .unwrap_or("GPU: N/A".into());

    let header = format!(
        " RustProc Shell  │  CPU: {:.0}%  RAM: {}/{} GB  {}  PIDs: {}",
        s.cpu_usage,
        s.ram_used_mb / 1024,
        s.ram_total_mb / 1024,
        gpu_str,
        s.process_count,
    );

    execute!(out,
        SetForegroundColor(Color::Black),
        // (background would use SetBackgroundColor — apply accent colour here)
        Print(format!("{:<width$}", header, width = width as usize)),
        ResetColor,
        Print("\n"),
    ).ok();

    // ── Stats panel ─────────────────────────────────────────────────────────
    print_separator(&mut out, width, "System Stats");
    print_gauge(&mut out, "CPU ", s.cpu_usage, 100.0,
        s.cpu_temp_c.map(|t| format!("  Temp: {:.0}°C", t)).unwrap_or_default(), width);
    print_gauge(&mut out, "RAM ", (s.ram_used_mb as f32 / s.ram_total_mb.max(1) as f32) * 100.0, 100.0,
        format!("  {}/{} GB", s.ram_used_mb/1024, s.ram_total_mb/1024), width);
    if let Some(gpu) = s.gpu_usage {
        let vram_str = match (s.vram_used_mb, s.vram_total_mb) {
            (Some(u), Some(t)) => format!("  VRAM: {}/{} GB", u/1024, t/1024),
            _ => String::new(),
        };
        print_gauge(&mut out, "GPU ", gpu, 100.0, vram_str, width);
    }

    // ── Process table ────────────────────────────────────────────────────────
    print_separator(&mut out, width, &format!("Processes (top {} by CPU)", process_rows));
    println!("  {:<8} {:<20} {:>6}  {:>8}  {}", "PID", "Name", "CPU%", "Mem MB", "Status");
    println!("  {}", "─".repeat(width as usize - 4));

    for row in s.processes.iter().take(process_rows) {
        let cpu_colour = threshold_colour(row.cpu_pct, 50.0, 80.0);
        execute!(out, SetForegroundColor(cpu_colour)).ok();
        println!("  {:<8} {:<20} {:>5.1}%  {:>7}  {}",
            row.pid, truncate(&row.name, 20), row.cpu_pct, row.mem_mb, row.status);
        execute!(out, ResetColor).ok();
    }

    // ── Output panel ─────────────────────────────────────────────────────────
    print_separator(&mut out, width, "Shell Output");
    let start = output.len().saturating_sub(output_lines);
    for line in &output[start..] {
        println!("  {}", line);
    }
    // Pad remaining space
    let shown = output.len().min(output_lines);
    for _ in shown..output_lines {
        println!();
    }

    // ── Prompt ───────────────────────────────────────────────────────────────
    print_separator(&mut out, width, "");
    let cwd = abbreviate_path(&s.cwd);
    execute!(out, SetForegroundColor(Color::Cyan)).ok();
    print!("  RustProc {} > ", cwd);
    execute!(out, ResetColor).ok();
    out.flush().ok();
}

fn print_gauge(out: &mut impl Write, label: &str, value: f32, max: f32, suffix: String, width: u16) {
    let bar_width = 20usize;
    let filled    = ((value / max) * bar_width as f32) as usize;
    let bar       = format!("{}{}", "█".repeat(filled), "░".repeat(bar_width - filled));
    let colour    = threshold_colour(value, 50.0, 80.0);
    execute!(out, SetForegroundColor(colour)).ok();
    println!("  {}  {}  {:.0}%{}", label, bar, value, suffix);
    execute!(out, ResetColor).ok();
}

fn threshold_colour(v: f32, warn: f32, crit: f32) -> Color {
    if v >= crit { Color::Red }
    else if v >= warn { Color::Yellow }
    else { Color::Green }
}

fn abbreviate_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path.starts_with(home_str.as_ref()) {
            return path.replacen(home_str.as_ref(), "~", 1);
        }
    }
    path.to_string()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

pub fn cleanup() {
    execute!(stdout(), ResetColor, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0)).ok();
}
```

### 5.7 `history.rs` — Command History

Arrow key navigation requires intercepting raw key events before the line is submitted. This means temporarily enabling raw mode during input, intercepting `Up`/`Down`/`Backspace`/`Enter`, and rendering the current line in place.

```rust
// src/history.rs

pub struct History {
    entries: Vec<String>,
    cursor:  usize,           // index into entries (0 = newest)
    saved:   String,          // preserves partially typed input when navigating
}

impl History {
    pub fn new() -> Self { Self { entries: vec![], cursor: 0, saved: String::new() } }

    pub fn push(&mut self, line: String) {
        if self.entries.first().map(|s| s != &line).unwrap_or(true) {
            self.entries.insert(0, line);
        }
        self.cursor = 0;
    }

    pub fn prev(&mut self, current: &str) -> Option<&str> {
        if self.cursor == 0 { self.saved = current.to_string(); }
        if self.cursor < self.entries.len() {
            self.cursor += 1;
            Some(&self.entries[self.cursor - 1])
        } else { None }
    }

    pub fn next(&mut self) -> Option<&str> {
        if self.cursor > 0 {
            self.cursor -= 1;
            if self.cursor == 0 { Some(&self.saved) }
            else { Some(&self.entries[self.cursor - 1]) }
        } else { None }
    }
}
```

### 5.8 `input.rs` — Raw Input with Arrow Keys

```rust
// src/input.rs

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::execute;
use crossterm::cursor;
use std::io::{stdout, Write};

pub fn readline(history: &mut History) -> Result<String, ()> {
    enable_raw_mode().map_err(|_| ())?;
    let mut buf = String::new();
    let mut out = stdout();

    loop {
        if let Ok(Event::Key(key)) = event::read() {
            match (key.code, key.modifiers) {

                // Submit
                (KeyCode::Enter, _) => break,

                // Ctrl+C / Ctrl+D — interrupt
                (KeyCode::Char('c'), m) | (KeyCode::Char('d'), m)
                    if m.contains(KeyModifiers::CONTROL) =>
                {
                    disable_raw_mode().ok();
                    return Err(());
                }

                // Backspace
                (KeyCode::Backspace, _) => {
                    if buf.pop().is_some() {
                        execute!(out, cursor::MoveLeft(1),
                            crossterm::style::Print(" "),
                            cursor::MoveLeft(1)).ok();
                    }
                }

                // History navigation
                (KeyCode::Up, _) => {
                    if let Some(prev) = history.prev(&buf) {
                        let prev = prev.to_string();
                        clear_line(&mut out, buf.len());
                        buf = prev.clone();
                        print!("{}", prev);
                        out.flush().ok();
                    }
                }
                (KeyCode::Down, _) => {
                    if let Some(next) = history.next() {
                        let next = next.to_string();
                        clear_line(&mut out, buf.len());
                        buf = next.clone();
                        print!("{}", next);
                        out.flush().ok();
                    }
                }

                // Regular character
                (KeyCode::Char(c), _) => {
                    buf.push(c);
                    print!("{}", c);
                    out.flush().ok();
                }

                _ => {}
            }
        }
    }

    disable_raw_mode().ok();
    println!(); // newline after Enter
    Ok(buf)
}

fn clear_line(out: &mut impl Write, len: usize) {
    if len > 0 {
        execute!(out,
            cursor::MoveLeft(len as u16),
            crossterm::style::Print(" ".repeat(len)),
            cursor::MoveLeft(len as u16),
        ).ok();
    }
}
```

---

## 6. OS Concepts Coverage Map

This project maps directly to core OS curriculum topics. Use this table verbatim in your report/presentation:

| Feature in RustProc Shell | OS Concept Demonstrated | Where in Code |
|---|---|---|
| `Command::new().spawn()` | Process creation (`fork` + `exec`) | `executor.rs` |
| `output.status.code()` | Process termination and `waitpid` | `executor.rs` |
| Background `&` command | Process states: running vs detached | `executor.rs` |
| Process table sorted by CPU | Process scheduling visibility | `monitor.rs` |
| `sysinfo` CPU usage | CPU scheduling metrics | `monitor.rs` |
| RAM usage with `sysinfo` | Memory management: allocation tracking | `monitor.rs` |
| GPU VRAM usage | Device/resource management | `monitor.rs` / `gpu.rs` |
| `Arc<Mutex<AppState>>` | Concurrency: mutual exclusion | `state.rs`, `monitor.rs` |
| Monitor runs on separate thread | Concurrency: multi-threading | `main.rs` |
| `std::env::set_current_dir` | Process working directory (kernel attribute) | `builtins.rs` |
| Built-in `cd` vs external commands | Why some commands must be built-in | `builtins.rs` |
| `Stdio::piped()` capturing output | IPC via pipes | `executor.rs` |
| Shell prompt loop | User-space/kernel-space interface | `shell.rs` |

---

## 7. Implementation Phases

### Phase 1 — Compilable Skeleton (30 minutes)

Goal: all modules exist, project compiles, nothing crashes.

- `cargo new rustproc-shell`
- Create all 8 source files with stub implementations
- Confirm `cargo build` exits 0
- Write `main.rs`: spawn a dummy monitor thread that sleeps, call `shell::run` which immediately returns

Success criterion: `cargo run` starts and exits without error.

---

### Phase 2 — Static Screen Layout (45 minutes)

Goal: full screen layout renders with placeholder values.

- Implement `renderer::draw` with hardcoded `AppState::default()`
- Implement `print_gauge` with filled/empty bar characters
- Implement `print_separator` with a labelled divider line
- Implement `cleanup` to reset terminal on exit
- Implement `renderer::draw` called once from `shell::run` before the input loop

Success criterion: running the program shows the full 3-panel layout before quitting.

---

### Phase 3 — Live Monitor Thread (45 minutes)

Goal: stats panel shows real hardware data.

- Implement `monitor::run` with `sysinfo` refresh loop
- Wire `SharedState` between `main.rs` and `monitor::run`
- Replace hardcoded values in `renderer::draw` with `state.lock().unwrap().clone()`
- Verify RAM total matches `free -h`

Success criterion: CPU bar changes value between renders; RAM value matches system tools.

---

### Phase 4 — Input and Command Dispatch (1 hour)

Goal: user can type commands that execute and show output.

- Implement `input::readline` with `enable_raw_mode` / `disable_raw_mode`
- Implement basic character and backspace handling (no arrow keys yet)
- Implement `executor::run` for foreground external commands
- Wire `shell::run` loop: draw → readline → execute → append output → repeat

Success criterion: `ls`, `echo hello`, and `pwd` all produce correct output in the output panel.

---

### Phase 5 — Built-in Commands (30 minutes)

Goal: `cd`, `help`, `clear`, `sysinfo`, `exit` all work.

- Implement all builtins in `builtins.rs`
- Verify `cd` updates the prompt path
- Verify `exit` causes clean shutdown and terminal restore

Success criterion: `cd /tmp` changes the next command's working directory; `pwd` confirms it.

---

### Phase 6 — Command History (30 minutes)

Goal: up/down arrows navigate previous commands.

- Implement `History` struct in `history.rs`
- Add `Up`/`Down` key handling in `input::readline`
- Push each successful command to history

Success criterion: press up after three commands, cycle through all three, press down to return to empty prompt.

---

### Phase 7 — Background Process Support (20 minutes)

Goal: `sleep 10 &` returns immediately without blocking the shell.

- Detect trailing `&` in `executor::run`
- Use `.spawn()` instead of `.output()` for background commands
- Return a `[background] cmd` line to the output buffer

Success criterion: `sleep 10 &` shows `[background] sleep 10` and returns the prompt instantly; the PID appears in the process table.

---

### Phase 8 — Polish and Hardening (30 minutes)

Goal: stable, demo-ready.

- Handle `Ctrl+C` without crashing (it should interrupt the current command or clear the input line, not kill the shell)
- Handle very long command output gracefully (output buffer capped at 200 lines)
- Handle terminal resize: re-query `terminal::size()` on each draw call (already done if `draw` calls `terminal::size()` fresh each time)
- Test: run `cargo run`, execute 20 different commands, verify the monitor stats stay live throughout
- `cargo clippy -- -D warnings` exits 0

---

## 8. Cargo.toml

```toml
[package]
name    = "rustproc-shell"
version = "0.1.0"
edition = "2021"

[features]
default = ["nvidia"]
nvidia  = ["nvml-wrapper"]

[dependencies]
sysinfo      = "0.30"
crossterm    = "0.27"
dirs         = "5.0"
serde        = { version = "1", features = ["derive"] }
toml         = "0.8"

# Optional NVIDIA GPU support
nvml-wrapper = { version = "0.9", optional = true }

[profile.release]
opt-level = 3
strip     = true
```

---

## 9. Build and Run Reference

```bash
# Development
cargo run

# Without NVIDIA (machines without CUDA/NVML)
cargo run --no-default-features

# Release binary
cargo build --release
./target/release/rustproc-shell

# Lint before demo
cargo clippy --all-features -- -D warnings
cargo fmt --check
```

---

## 10. Shell Built-in Reference

| Command | Arguments | OS Concept |
|---|---|---|
| `cd [dir]` | Path or `~` | Working directory is a per-process kernel attribute |
| `help` | — | — |
| `clear` | — | Clears output buffer |
| `sysinfo` | — | Prints current telemetry snapshot |
| `jobs` | — | Lists background child processes |
| `exit` / `quit` | — | Clean terminal teardown |
| `[cmd] &` | Any command | Fork without waitpid |

---

## 11. Demo Script (3-Minute Walkthrough)

1. **`cargo run`** — shell opens, all three panels visible, stats already live.
2. **Type `ls`** — output appears in the middle panel. Stats at top unchanged.
3. **Type `cd src`** — prompt updates to `~/rustproc-shell/src`. Type `ls` again — different output.
4. **Type `sysinfo`** — snapshot of current CPU/RAM/GPU printed inline.
5. **Open stress in background:** `stress --cpu 4 &` — shell returns immediately. Watch CPU bar climb in the stats panel. Find the `stress` PID in the process table.
6. **Type `help`** — built-in command list appears.
7. **Press ↑** — previous command recalled. Demonstrate history navigation.
8. **Type `exit`** — terminal restored cleanly.
9. **Explain the architecture:** two threads, shared mutex, `fork+exec` under the hood, built-ins vs externals.

---

## 12. What Makes This Project Stand Out

Other student projects demonstrate one concept. RustProc Shell demonstrates the interaction between concepts — which is where real OS understanding lives:

- You can **see** a `fork()` happen the moment you type a command
- You can **watch** CPU scheduling change in real time as a process runs
- You can **observe** memory allocation in the process table while running memory-heavy commands
- The `cd` built-in exists precisely because of how per-process working directories work at the kernel level — and you can explain that

That is not a monitoring dashboard. That is not a mini shell. That is a **live demonstration of how a Unix system actually works**, running in a terminal, written in a systems language, built over a weekend.

---

## 13. Final Checklist

### Shell
- [ ] External commands execute and capture stdout/stderr
- [ ] `cd` updates working directory and prompt
- [ ] `exit` restores terminal cleanly
- [ ] `help` lists all built-ins
- [ ] `clear` clears the output panel
- [ ] Background `&` commands return prompt immediately
- [ ] `Ctrl+C` does not kill the shell (clears current input line)
- [ ] Up/down arrow history navigation works

### Monitor
- [ ] CPU bar reflects real usage (cross-check with `htop`)
- [ ] RAM values match `free -h`
- [ ] GPU panel shows "N/A" gracefully on non-NVIDIA hardware
- [ ] Process table sorted by CPU descending
- [ ] Stats visibly update while user types at the prompt

### Stability
- [ ] Output buffer never grows unbounded
- [ ] Terminal fully restored after `exit`
- [ ] `cargo clippy -- -D warnings` exits 0
- [ ] Tested: run 20 commands without crash or visual artefact
- [ ] Tested on the actual demo machine
