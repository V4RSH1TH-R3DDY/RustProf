# RustProc Shell — AI-Assisted Developer Environment
## Comprehensive Implementation Plan

---

## 1. What This Project Is

RustProc Shell is a terminal-native developer environment written in Rust. It combines three systems that typically exist in isolation — a Unix shell, a real-time system monitor, and an AI coding assistant — into a single cohesive interface that runs entirely in the terminal.

The pitch in one sentence: **a custom shell that knows what your machine is doing and can ask an AI what your code should do.**

This is not an academic exercise in rebuilding existing tools. It is a systems integration project — the kind of engineering that actually ships in production developer tooling. OpenCode handles the AI layer. `sysinfo` handles the hardware layer. Rust's `std::process` handles the execution layer. RustProc Shell orchestrates all three and presents them through a unified command interface.

The result is something that looks like this during a demo:

```
╔══════════════════════════════════════════════════════════════════════════════╗
║  RustProc Shell v0.3 — AI Developer Environment                             ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  CPU  ████████░░░░  62%   74°C   3.9 GHz  │  RAM  █████░░░░░  47%  6.1/32  ║
║  GPU  ████░░░░░░░░  28%   52°C             │  PIDs: 211   CWD: ~/project     ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  [Shell Output]                                                              ║
║  $ ai "write a binary search function in Rust"                               ║
║                                                                              ║
║  ── AI Response ────────────────────────────────────────────────────────    ║
║  fn binary_search(arr: &[i32], target: i32) -> Option<usize> {              ║
║      let (mut lo, mut hi) = (0, arr.len());                                 ║
║      while lo < hi {                                                        ║
║          let mid = lo + (hi - lo) / 2;                                      ║
║          match arr[mid].cmp(&target) {                                      ║
║              std::cmp::Ordering::Equal => return Some(mid),                 ║
║              std::cmp::Ordering::Less  => lo = mid + 1,                     ║
║              std::cmp::Ordering::Greater => hi = mid,                      ║
║          }                                                                  ║
║      }                                                                      ║
║      None                                                                   ║
║  }                                                                          ║
║  ─────────────────────────────────────────────────────────────────────────  ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  RustProc ~/project > █                                                     ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

---

## 2. What You Are NOT Building

This section is as important as everything else. Scope discipline is what makes this project finishable by tomorrow.

| ❌ Out of Scope | ✅ What You Are Building Instead |
|---|---|
| A full AI IDE | A shell that can call an AI |
| An autonomous coding agent | Built-in `ai` command that passes prompts to OpenCode |
| A language server (LSP) | None — you open files with `$EDITOR` or a minimal pager |
| AST parsing or code analysis | None — OpenCode handles all code intelligence |
| Collaborative editing | None |
| Pipe/redirect syntax (`\|`, `>`, `>>`) | Optional, Phase 9 only if time remains |
| A full Bash/Zsh clone | A focused developer shell with curated built-ins |
| Embedding OpenCode internally | Invoking OpenCode as an external subprocess |

The engineering philosophy is **orchestration, not reimplementation.** OpenCode is a tool. `sysinfo` is a tool. Your job is to build the workflow layer that connects them and presents a unified interface.

---

## 3. Full Feature List

### 3.1 Shell Core
- Command execution (foreground and background with `&`)
- Built-in `cd` with `~` expansion and path completion
- Built-in `exit` / `quit` with clean terminal teardown
- Built-in `help` listing all commands
- Built-in `clear` for the output panel
- Built-in `history` to list past commands
- Up/Down arrow key command history navigation
- Ctrl+C to cancel current input line (not kill the shell)
- Environment variable expansion (`$HOME`, `$PATH`, etc.)

### 3.2 System Monitor (live, background thread)
- CPU global usage %, temperature, current frequency
- RAM used/total in GB with usage %
- GPU load % and temperature (NVIDIA via NVML, fallback N/A)
- Active process count
- Current working directory in status bar
- Compact two-line header — always visible, never scrolls away

### 3.3 AI Integration Layer
- `ai "prompt"` — send a free-form prompt to OpenCode, stream response into output panel
- `ai explain [file]` — ask OpenCode to explain a file's contents
- `ai refactor [file]` — ask OpenCode to suggest refactoring for a file
- `ai fix [file]` — ask OpenCode to identify and fix issues in a file
- `ai gen [file] "prompt"` — ask OpenCode to generate code and write it to a file
- `opencode` — launch the full OpenCode interactive session
- `ask "question"` — shorthand alias for `ai "question"`

### 3.4 File Operations
- `edit [file]` — open file in `$EDITOR` (defaults to `nano` if unset)
- `view [file]` — display file contents in the output panel with line numbers
- `save [content] [file]` — write the last AI response to a named file

### 3.5 Developer Shortcuts
- `run [cmd]` — execute any command and show output (alias for direct execution, useful for clarity: `run cargo build`)
- `project [dir]` — set project root, update CWD, and print directory summary
- `sysinfo` — snapshot of current hardware stats as plain text

---

## 4. Architecture

### 4.1 System Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                        RustProc Shell                                │
│                                                                      │
│  ┌─────────────┐    Arc<Mutex<AppState>>    ┌──────────────────────┐ │
│  │             │ ◄─────────────────────────► │                      │ │
│  │  Monitor    │                             │   Shell REPL         │ │
│  │  Thread     │  writes every 1s           │   (main thread)      │ │
│  │             │                             │                      │ │
│  │  sysinfo    │                             │  ┌────────────────┐  │ │
│  │  nvml       │                             │  │ Command Router │  │ │
│  │  hwmon      │                             │  └───────┬────────┘  │ │
│  └─────────────┘                             │          │           │ │
│                                              │    ┌─────▼──────┐   │ │
│                                              │    │            │   │ │
│                                              │  Built-ins  Executor│ │
│                                              │    │       AI Layer  │ │
│                                              │    │            │   │ │
│                                              │    └─────┬──────┘   │ │
│                                              └──────────┼──────────┘ │
└─────────────────────────────────────────────────────────┼────────────┘
                                                          │
                              ┌───────────────────────────┼──────────────┐
                              │                           │              │
                    ┌─────────▼──────┐        ┌──────────▼──────┐  ┌───▼──────┐
                    │   OS Kernel    │        │   OpenCode CLI  │  │  $EDITOR │
                    │  fork + exec   │        │   (subprocess)  │  │  (nano/  │
                    │  waitpid       │        │   streams JSON  │  │   vim)   │
                    └────────────────┘        └─────────────────┘  └──────────┘
```

### 4.2 Thread Model

```
Process Start
    │
    ├── Spawn Monitor Thread ──────────────────────────────────────► [runs forever]
    │       • every 1s: refresh sysinfo, write to Arc<Mutex<AppState>>
    │       • lock held < 1ms per tick
    │
    └── Shell Thread (main) ────────────────────────────────────────► [blocks on input]
            • renderer::draw() reads AppState snapshot
            • input::readline() blocks until Enter
            • router::dispatch() identifies command type
            • result appended to output buffer
            • loop
```

**Why this threading model is correct:**

The monitor thread refreshes hardware stats regardless of what the shell thread is doing. If the user is mid-prompt typing, stats still update. If a `cargo build` is running for 30 seconds, the CPU bar still moves. This is the same pattern used by `htop`, `btop`, and every other production terminal monitor. The `Arc<Mutex<>>` ensures no data race. The lock is held only for atomic value assignment — never across a `sysinfo` refresh call, which can take 10–100ms.

---

## 5. Repository Structure

```
rustproc-shell/
├── Cargo.toml
├── Cargo.lock                        # commit — reproducible builds
├── config.toml.example
├── README.md
├── install.sh                        # installs OpenCode if absent, then builds
└── src/
    ├── main.rs                       # startup: init state, spawn monitor, enter REPL
    ├── state.rs                      # AppState, ProcessRow, SharedState type alias
    ├── config.rs                     # config.toml parsing with sane defaults
    ├── monitor.rs                    # background thread: sysinfo + GPU polling
    ├── gpu.rs                        # NVML wrapper + sysfs fallback
    ├── temps.rs                      # /sys/class/hwmon/ reader (Linux)
    ├── shell/
    │   ├── mod.rs                    # shell::run() — the REPL loop
    │   ├── input.rs                  # raw mode readline with history navigation
    │   ├── history.rs                # History struct with up/down navigation
    │   ├── router.rs                 # parse input, classify, dispatch to handler
    │   └── expander.rs               # $VAR expansion, ~ expansion
    ├── commands/
    │   ├── mod.rs                    # CommandResult type, re-exports
    │   ├── builtins.rs               # cd, help, clear, history, sysinfo, view, save
    │   ├── executor.rs               # external command execution via std::process
    │   └── ai.rs                     # AI integration layer: all ai/* commands
    └── ui/
        ├── mod.rs                    # ui::draw() — full screen render
        ├── header.rs                 # two-line stats bar
        ├── output.rs                 # scrollable output panel
        └── prompt.rs                 # prompt line rendering
```

One directory per concern. `shell/` owns the REPL mechanics. `commands/` owns what happens when a command runs. `ui/` owns everything drawn to the screen. `monitor.rs` and `gpu.rs` own hardware polling. Nothing crosses those boundaries.

---

## 6. Data Structures

### 6.1 `state.rs`

```rust
use std::sync::{Arc, Mutex};

pub type SharedState = Arc<Mutex<AppState>>;

#[derive(Debug, Clone, Default)]
pub struct AppState {
    // CPU
    pub cpu_usage_pct:  f32,
    pub cpu_temp_c:     Option<f32>,
    pub cpu_freq_mhz:   u64,
    // RAM
    pub ram_used_mb:    u64,
    pub ram_total_mb:   u64,
    // GPU
    pub gpu_available:  bool,
    pub gpu_usage_pct:  Option<f32>,
    pub gpu_temp_c:     Option<f32>,
    pub vram_used_mb:   Option<u64>,
    pub vram_total_mb:  Option<u64>,
    // Process table
    pub process_count:  usize,
    pub processes:      Vec<ProcessRow>,
    // Shell context
    pub cwd:            String,
    pub last_exit_code: i32,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessRow {
    pub pid:     u32,
    pub name:    String,
    pub cpu_pct: f32,
    pub mem_mb:  u64,
    pub status:  String,
}
```

### 6.2 `commands/mod.rs` — Command Result Type

```rust
// Every command handler returns this type.
// The shell loop appends lines to the output buffer and records the exit code.

pub type CommandResult = Result<Vec<OutputLine>, ShellError>;

#[derive(Debug, Clone)]
pub struct OutputLine {
    pub text:  String,
    pub kind:  LineKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineKind {
    Normal,      // white — regular output
    Ai,          // cyan  — AI response content
    AiHeader,    // bold cyan — "── AI Response ──"
    Success,     // green
    Warning,     // yellow
    Error,       // red
    Dim,         // grey — metadata lines like [exit 1], [background]
}

#[derive(Debug)]
pub struct ShellError(pub String);

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Convenience constructors
impl OutputLine {
    pub fn normal(s: impl Into<String>)  -> Self { Self { text: s.into(), kind: LineKind::Normal } }
    pub fn ai(s: impl Into<String>)      -> Self { Self { text: s.into(), kind: LineKind::Ai } }
    pub fn ai_header(s: impl Into<String>) -> Self { Self { text: s.into(), kind: LineKind::AiHeader } }
    pub fn error(s: impl Into<String>)   -> Self { Self { text: s.into(), kind: LineKind::Error } }
    pub fn dim(s: impl Into<String>)     -> Self { Self { text: s.into(), kind: LineKind::Dim } }
    pub fn success(s: impl Into<String>) -> Self { Self { text: s.into(), kind: LineKind::Success } }
}
```

The `LineKind` enum is what makes the output panel visually useful. AI response lines are coloured distinctly from normal command output. Error lines are red. This gives the user instant orientation — they never need to read output metadata to know what type of output they're looking at.

---

## 7. Module Deep Dives

### 7.1 `shell/router.rs` — Command Classification

The router is the brain of the shell. It receives a raw input string and returns a `DispatchTarget` that tells the shell loop which handler to call. Classification is done in one match pass — no regex, no complex parsing.

```rust
// src/shell/router.rs

pub enum DispatchTarget<'a> {
    Builtin(BuiltinCmd<'a>),
    AiCommand(AiCmd<'a>),
    External(&'a str),
    Empty,
}

pub enum BuiltinCmd<'a> {
    Cd(&'a str),
    Help,
    Clear,
    History,
    Sysinfo,
    View(&'a str),
    Save(&'a str, &'a str),   // (content_source, filename)
    Edit(&'a str),
    Project(&'a str),
    Exit,
}

pub enum AiCmd<'a> {
    Prompt(&'a str),          // ai "..."  or  ask "..."
    Explain(&'a str),         // ai explain <file>
    Refactor(&'a str),        // ai refactor <file>
    Fix(&'a str),             // ai fix <file>
    Generate(&'a str, &'a str), // ai gen <file> "prompt"
    OpenCode,                 // opencode — launch interactive session
}

pub fn classify(raw: &str) -> DispatchTarget<'_> {
    let trimmed = raw.trim();
    if trimmed.is_empty() { return DispatchTarget::Empty; }

    let (cmd, rest) = split_first_word(trimmed);

    match cmd {
        // Built-ins
        "exit" | "quit"   => DispatchTarget::Builtin(BuiltinCmd::Exit),
        "cd"              => DispatchTarget::Builtin(BuiltinCmd::Cd(rest)),
        "help"            => DispatchTarget::Builtin(BuiltinCmd::Help),
        "clear"           => DispatchTarget::Builtin(BuiltinCmd::Clear),
        "history"         => DispatchTarget::Builtin(BuiltinCmd::History),
        "sysinfo"         => DispatchTarget::Builtin(BuiltinCmd::Sysinfo),
        "view"            => DispatchTarget::Builtin(BuiltinCmd::View(rest)),
        "edit"            => DispatchTarget::Builtin(BuiltinCmd::Edit(rest)),
        "project"         => DispatchTarget::Builtin(BuiltinCmd::Project(rest)),

        // AI commands
        "ai" | "ask"      => classify_ai(rest),
        "opencode"        => DispatchTarget::AiCommand(AiCmd::OpenCode),

        // Everything else goes to the OS
        _                 => DispatchTarget::External(trimmed),
    }
}

fn classify_ai(rest: &str) -> DispatchTarget<'_> {
    let (subcmd, arg) = split_first_word(rest);
    match subcmd {
        "explain"  => DispatchTarget::AiCommand(AiCmd::Explain(arg)),
        "refactor" => DispatchTarget::AiCommand(AiCmd::Refactor(arg)),
        "fix"      => DispatchTarget::AiCommand(AiCmd::Fix(arg)),
        "gen"      => {
            let (file, prompt) = split_first_word(arg);
            DispatchTarget::AiCommand(AiCmd::Generate(file, prompt.trim_matches('"')))
        }
        _          => DispatchTarget::AiCommand(AiCmd::Prompt(rest.trim_matches('"'))),
    }
}

fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim();
    match s.find(|c: char| c.is_whitespace()) {
        Some(i) => (&s[..i], s[i..].trim()),
        None    => (s, ""),
    }
}
```

### 7.2 `commands/ai.rs` — AI Integration Layer

This is the most important module in the project from an architectural standpoint. It handles all communication with OpenCode. The key insight: **OpenCode is a subprocess**. RustProc Shell does not embed any AI model. It constructs a command, spawns OpenCode, captures its output, and presents it in the output panel. This is identical in principle to how `git` calls `gpg` for signed commits, or how `npm` calls system `node`.

#### 7.2.1 OpenCode Detection

```rust
// src/commands/ai.rs

use std::process::{Command, Stdio};

/// Check if OpenCode is installed and accessible on $PATH
pub fn opencode_available() -> bool {
    Command::new("opencode")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

This is called once at startup. If OpenCode is not found, the `ai` command family prints a helpful install message instead of attempting to run. The rest of the shell still works perfectly.

#### 7.2.2 Free-Form Prompt (`ai "..."`)

```rust
pub fn ai_prompt(prompt: &str) -> CommandResult {
    let mut lines = vec![
        OutputLine::ai_header("── AI Response ─────────────────────────────────────"),
    ];

    let output = Command::new("opencode")
        .args(["run", "--print", prompt])
        .output()
        .map_err(|e| ShellError(format!("opencode: {}", e)))?;

    if output.status.success() {
        let response = String::from_utf8_lossy(&output.stdout);
        lines.extend(
            response.lines().map(|l| OutputLine::ai(l))
        );
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        lines.push(OutputLine::error(format!("opencode error: {}", err.trim())));
    }

    lines.push(OutputLine::ai_header("─────────────────────────────────────────────────────"));
    Ok(lines)
}
```

> **Note on the `--print` flag:** Check OpenCode's current CLI reference for the correct flag to run a one-shot prompt and print the output. As of May 2026, the flag may be `--print`, `--message`, or the subcommand structure may differ. Run `opencode --help` and adjust the argument accordingly. The pattern — subprocess invocation + stdout capture — remains identical regardless of the flag name.

#### 7.2.3 File-Context Commands (`ai explain`, `ai refactor`, `ai fix`)

These commands read the file first, then construct a prompt that includes the file contents. This is the correct pattern: OpenCode does not know your filesystem — you send it the content.

```rust
pub fn ai_explain(file_path: &str) -> CommandResult {
    let content = read_file_for_ai(file_path)?;

    let prompt = format!(
        "Explain what this code does. Be concise and focused on the key logic:\n\n```\n{}\n```",
        content
    );

    // Prepend the command header, then call ai_prompt
    let mut result = vec![
        OutputLine::dim(format!("→ ai explain {}", file_path)),
    ];
    result.extend(ai_prompt(&prompt)?);
    Ok(result)
}

pub fn ai_refactor(file_path: &str) -> CommandResult {
    let content = read_file_for_ai(file_path)?;
    let prompt = format!(
        "Suggest specific refactoring improvements for this code. \
         Focus on readability, idiomatic style, and correctness:\n\n```\n{}\n```",
        content
    );
    let mut result = vec![OutputLine::dim(format!("→ ai refactor {}", file_path))];
    result.extend(ai_prompt(&prompt)?);
    Ok(result)
}

pub fn ai_fix(file_path: &str) -> CommandResult {
    let content = read_file_for_ai(file_path)?;
    let prompt = format!(
        "Identify bugs and issues in this code and provide corrected versions. \
         Explain each fix:\n\n```\n{}\n```",
        content
    );
    let mut result = vec![OutputLine::dim(format!("→ ai fix {}", file_path))];
    result.extend(ai_prompt(&prompt)?);
    Ok(result)
}

pub fn ai_generate(file_path: &str, prompt_text: &str) -> CommandResult {
    let prompt = format!(
        "{}\n\nWrite clean, production-quality code. \
         Output only the code, no explanation unless asked.",
        prompt_text
    );

    let mut lines = vec![
        OutputLine::dim(format!("→ ai gen {} \"{}\"", file_path, prompt_text)),
    ];

    let result = ai_prompt(&prompt)?;
    lines.extend(result.clone());

    // Extract code block content and write to file
    let code = extract_code_block(
        &result.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
    );

    if !code.is_empty() && !file_path.is_empty() {
        match std::fs::write(file_path, &code) {
            Ok(_)  => lines.push(OutputLine::success(format!("✓ written to {}", file_path))),
            Err(e) => lines.push(OutputLine::error(format!("could not write {}: {}", file_path, e))),
        }
    }

    Ok(lines)
}

fn read_file_for_ai(path: &str) -> Result<String, ShellError> {
    if path.is_empty() {
        return Err(ShellError("ai: no file specified".into()));
    }
    std::fs::read_to_string(path)
        .map_err(|e| ShellError(format!("cannot read {}: {}", path, e)))
}

/// Extract code from a fenced code block in AI response.
/// Falls back to the raw response if no fence found.
fn extract_code_block(response: &str) -> String {
    let lines: Vec<&str> = response.lines().collect();
    let start = lines.iter().position(|l| l.starts_with("```"));
    let end   = lines.iter().rposition(|l| l.starts_with("```"));

    match (start, end) {
        (Some(s), Some(e)) if e > s => lines[s+1..e].join("\n"),
        _                           => response.to_string(),
    }
}
```

#### 7.2.4 Interactive OpenCode Launch

```rust
pub fn launch_opencode() -> CommandResult {
    // Restore terminal before handing off to OpenCode
    crossterm::terminal::disable_raw_mode().ok();
    crossterm::execute!(
        std::io::stdout(),
        crossterm::style::ResetColor
    ).ok();

    let status = Command::new("opencode")
        .status()
        .map_err(|e| ShellError(format!("opencode: {}", e)))?;

    // Re-enable raw mode for our shell's input reader
    crossterm::terminal::enable_raw_mode().ok();

    Ok(vec![
        OutputLine::dim(format!("[opencode exited with code {}]",
            status.code().unwrap_or(0)))
    ])
}
```

When OpenCode runs interactively, it needs full terminal control. The shell yields the terminal to OpenCode, waits for it to exit, then reclaims control. This is the same handoff pattern used by `git commit` when it opens `$EDITOR`.

### 7.3 `commands/builtins.rs`

```rust
// src/commands/builtins.rs

use crate::state::SharedState;
use super::{CommandResult, OutputLine};

pub fn cd(args: &str, state: &SharedState) -> CommandResult {
    let target = if args.is_empty() || args == "~" {
        dirs::home_dir().ok_or_else(|| crate::commands::ShellError("no home directory".into()))?
    } else {
        std::path::PathBuf::from(expand_tilde(args))
    };

    std::env::set_current_dir(&target)
        .map_err(|e| crate::commands::ShellError(format!("cd: {}", e)))?;

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    state.lock().unwrap().cwd = cwd;
    Ok(vec![]) // cd is silent on success — identical to bash behaviour
}

pub fn view(path: &str) -> CommandResult {
    if path.is_empty() {
        return Err(crate::commands::ShellError("view: no file specified".into()));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| crate::commands::ShellError(format!("view: {}", e)))?;

    let mut lines = vec![
        OutputLine::dim(format!("── {} ─────────────────────────────────────────", path)),
    ];

    lines.extend(
        content.lines().enumerate()
            .map(|(i, l)| OutputLine::normal(format!("{:>4}  {}", i + 1, l)))
    );

    lines.push(OutputLine::dim("──────────────────────────────────────────────────────".into()));
    Ok(lines)
}

pub fn edit(path: &str) -> CommandResult {
    if path.is_empty() {
        return Err(crate::commands::ShellError("edit: no file specified".into()));
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".into());

    crossterm::terminal::disable_raw_mode().ok();

    std::process::Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|e| crate::commands::ShellError(format!("{}: {}", editor, e)))?;

    crossterm::terminal::enable_raw_mode().ok();

    Ok(vec![OutputLine::dim(format!("[closed {} in {}]", path, editor))])
}

pub fn sysinfo_snapshot(state: &SharedState) -> CommandResult {
    let s = state.lock().unwrap().clone();
    Ok(vec![
        OutputLine::normal(format!("CPU   {:.1}%{}",
            s.cpu_usage_pct,
            s.cpu_temp_c.map(|t| format!("  {:.0}°C", t)).unwrap_or_default()
        )),
        OutputLine::normal(format!("RAM   {} MB / {} MB  ({:.0}%)",
            s.ram_used_mb, s.ram_total_mb,
            100.0 * s.ram_used_mb as f32 / s.ram_total_mb.max(1) as f32
        )),
        s.gpu_usage_pct.map(|g| OutputLine::normal(format!(
            "GPU   {:.1}%{}",
            g,
            s.gpu_temp_c.map(|t| format!("  {:.0}°C", t)).unwrap_or_default()
        ))).unwrap_or(OutputLine::normal("GPU   N/A".into())),
        OutputLine::normal(format!("PIDs  {}", s.process_count)),
    ])
}

pub fn help(opencode_ok: bool) -> CommandResult {
    let ai_status = if opencode_ok { "✓ OpenCode connected" } else { "✗ OpenCode not found — run: npm i -g opencode-ai" };

    Ok(vec![
        OutputLine::ai_header("── RustProc Shell ─────────────────────────────────────"),
        OutputLine::normal("Shell".into()),
        OutputLine::normal("  cd [dir]          Change directory".into()),
        OutputLine::normal("  edit [file]       Open file in $EDITOR".into()),
        OutputLine::normal("  view [file]       Display file with line numbers".into()),
        OutputLine::normal("  project [dir]     Set project root".into()),
        OutputLine::normal("  sysinfo           Hardware snapshot".into()),
        OutputLine::normal("  history           List command history".into()),
        OutputLine::normal("  clear             Clear output panel".into()),
        OutputLine::normal("  exit / quit       Exit shell".into()),
        OutputLine::normal("".into()),
        OutputLine::normal("AI Commands".into()),
        OutputLine::normal("  ai \"prompt\"       Free-form AI prompt".into()),
        OutputLine::normal("  ask \"question\"    Alias for ai \"...\"".into()),
        OutputLine::normal("  ai explain [file] Explain a file".into()),
        OutputLine::normal("  ai refactor [f]   Suggest refactoring".into()),
        OutputLine::normal("  ai fix [file]     Find and fix bugs".into()),
        OutputLine::normal("  ai gen [f] \"p\"    Generate code to file".into()),
        OutputLine::normal("  opencode          Launch OpenCode interactively".into()),
        OutputLine::normal("".into()),
        OutputLine::normal("  All other input is passed to the OS as external commands.".into()),
        OutputLine::normal("  Append & to run in background: cargo build &".into()),
        OutputLine::normal("".into()),
        if opencode_ok {
            OutputLine::success(format!("  AI Status: {}", ai_status))
        } else {
            OutputLine::error(format!("  AI Status: {}", ai_status))
        },
        OutputLine::ai_header("─────────────────────────────────────────────────────────"),
    ])
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}
```

### 7.4 `shell/mod.rs` — The REPL Loop

```rust
// src/shell/mod.rs

pub fn run(state: SharedState, opencode_ok: bool, config: Config) {
    let mut history  = History::new();
    let mut output:  Vec<OutputLine> = vec![
        OutputLine::success("RustProc Shell ready. Type 'help' for commands.".into()),
        if opencode_ok {
            OutputLine::success("AI: OpenCode connected. Try: ai \"write hello world in Rust\"".into())
        } else {
            OutputLine::error("AI: OpenCode not found. Install: npm i -g opencode-ai".into())
        },
    ];

    loop {
        // Draw full screen from current state
        ui::draw(&state, &output, &config);

        // Block until user submits a line
        let raw = match input::readline(&mut history) {
            Ok(line) => line,
            Err(_)   => break,   // Ctrl+D
        };

        let trimmed = raw.trim();
        if trimmed.is_empty() { continue; }
        history.push(trimmed.to_string());

        // Echo the command into the output buffer
        output.push(OutputLine::dim(format!("$ {}", trimmed)));

        // Classify and dispatch
        let result: CommandResult = match router::classify(trimmed) {
            DispatchTarget::Empty              => Ok(vec![]),
            DispatchTarget::Builtin(cmd)       => dispatch_builtin(cmd, &state),
            DispatchTarget::AiCommand(cmd)     => dispatch_ai(cmd, opencode_ok),
            DispatchTarget::External(raw)      => executor::run(raw),
        };

        // Append result lines
        match result {
            Ok(lines)  => output.extend(lines),
            Err(e)     => output.push(OutputLine::error(e.to_string())),
        }

        // Cap output buffer to prevent unbounded memory growth
        const MAX_LINES: usize = 500;
        if output.len() > MAX_LINES {
            output.drain(0..output.len() - MAX_LINES);
        }

        // Handle exit — check if the last command was cd (state.cwd changed)
        // and handle 'exit' specially since it breaks the loop
        if trimmed == "exit" || trimmed == "quit" { break; }
    }

    ui::cleanup();
    println!("Goodbye.");
}
```

### 7.5 `ui/mod.rs` — Renderer

```rust
// src/ui/mod.rs

use crossterm::{cursor, execute, style::{Color, Print, SetForegroundColor,
    SetBackgroundColor, Attribute, SetAttribute, ResetColor}, terminal};
use std::io::{stdout, Write};
use crate::{state::SharedState, commands::LineKind};

pub fn draw(state: &SharedState, output: &[OutputLine], config: &Config) {
    let snapshot = state.lock().unwrap().clone();
    let (width, height) = terminal::size().unwrap_or((80, 24));
    let mut stdout = stdout();

    execute!(stdout,
        cursor::Hide,
        cursor::MoveTo(0, 0),
        terminal::Clear(terminal::ClearType::All),
    ).ok();

    // ── Line 1: Application header ──────────────────────────────────────────
    header::draw_title(&mut stdout, width);

    // ── Lines 2–3: Hardware stats ────────────────────────────────────────────
    header::draw_stats(&mut stdout, &snapshot, width);

    // ── Separator ───────────────────────────────────────────────────────────
    draw_separator(&mut stdout, width, "");

    // ── Output panel: fills remaining height minus 3 lines for prompt area ──
    let reserved   = 6usize;  // title + 2 stat lines + separator + prompt line + separator
    let panel_height = (height as usize).saturating_sub(reserved);

    output::draw(&mut stdout, output, panel_height, width);

    // ── Prompt ───────────────────────────────────────────────────────────────
    draw_separator(&mut stdout, width, "");
    prompt::draw(&mut stdout, &snapshot.cwd, snapshot.last_exit_code);

    stdout.flush().ok();
    execute!(stdout, cursor::Show).ok();
}

pub fn cleanup() {
    let mut stdout = stdout();
    execute!(stdout,
        cursor::Show,
        ResetColor,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0),
    ).ok();
}

pub fn draw_separator(out: &mut impl Write, width: u16, label: &str) {
    let line = if label.is_empty() {
        "─".repeat(width as usize)
    } else {
        let inner = format!("─ {} ", label);
        let remaining = (width as usize).saturating_sub(inner.len());
        format!("{}{}", inner, "─".repeat(remaining))
    };
    execute!(out,
        SetForegroundColor(Color::DarkGrey),
        Print(line),
        Print("\n"),
        ResetColor,
    ).ok();
}
```

```rust
// src/ui/header.rs

pub fn draw_title(out: &mut impl Write, width: u16) {
    let title = " RustProc Shell — AI Developer Environment";
    execute!(out,
        SetBackgroundColor(Color::DarkBlue),
        SetForegroundColor(Color::White),
        SetAttribute(Attribute::Bold),
        Print(format!("{:<width$}", title, width = width as usize)),
        ResetColor,
        Print("\n"),
    ).ok();
}

pub fn draw_stats(out: &mut impl Write, s: &AppState, width: u16) {
    // Left column: CPU + RAM
    let cpu_bar  = gauge_bar(s.cpu_usage_pct, 100.0, 14);
    let ram_pct  = 100.0 * s.ram_used_mb as f32 / s.ram_total_mb.max(1) as f32;
    let ram_bar  = gauge_bar(ram_pct, 100.0, 14);

    let cpu_temp = s.cpu_temp_c.map(|t| format!("{:.0}°C", t)).unwrap_or("N/A".into());

    // Right column: GPU + process count
    let gpu_str = match s.gpu_usage_pct {
        Some(g) => {
            let bar  = gauge_bar(g, 100.0, 10);
            let temp = s.gpu_temp_c.map(|t| format!("{:.0}°C", t)).unwrap_or("N/A".into());
            format!("GPU {} {:.0}% {}", bar, g, temp)
        }
        None => "GPU N/A".into(),
    };

    execute!(out, SetForegroundColor(threshold_colour(s.cpu_usage_pct, 50.0, 80.0))).ok();
    print!("  CPU {} {:.0}%  {}  ", cpu_bar, s.cpu_usage_pct, cpu_temp);
    execute!(out, ResetColor, SetForegroundColor(Color::DarkGrey)).ok();
    print!("│  ");
    execute!(out, SetForegroundColor(threshold_colour(s.gpu_usage_pct.unwrap_or(0.0), 50.0, 80.0))).ok();
    println!("{}", gpu_str);
    execute!(out, ResetColor).ok();

    execute!(out, SetForegroundColor(threshold_colour(ram_pct, 60.0, 85.0))).ok();
    print!("  RAM {} {:.0}%  {}/{} GB  ", ram_bar,
        ram_pct, s.ram_used_mb/1024, s.ram_total_mb/1024);
    execute!(out, ResetColor, SetForegroundColor(Color::DarkGrey)).ok();
    print!("│  PIDs: {}   ", s.process_count);
    execute!(out, ResetColor).ok();
    println!();
}

fn gauge_bar(value: f32, max: f32, width: usize) -> String {
    let filled = ((value / max) * width as f32).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn threshold_colour(v: f32, warn: f32, crit: f32) -> Color {
    if v >= crit { Color::Red }
    else if v >= warn { Color::Yellow }
    else { Color::Green }
}
```

```rust
// src/ui/output.rs

pub fn draw(out: &mut impl Write, lines: &[OutputLine], height: usize, width: u16) {
    let start = lines.len().saturating_sub(height);
    let visible = &lines[start..];

    for line in visible {
        let colour = match line.kind {
            LineKind::Ai       => Color::Cyan,
            LineKind::AiHeader => Color::DarkCyan,
            LineKind::Error    => Color::Red,
            LineKind::Success  => Color::Green,
            LineKind::Warning  => Color::Yellow,
            LineKind::Dim      => Color::DarkGrey,
            LineKind::Normal   => Color::Reset,
        };

        let bold = matches!(line.kind, LineKind::AiHeader);
        if bold { execute!(out, SetAttribute(Attribute::Bold)).ok(); }

        execute!(out, SetForegroundColor(colour),
            Print(format!("  {}\n", &line.text)),
            ResetColor,
        ).ok();

        if bold { execute!(out, SetAttribute(Attribute::Reset)).ok(); }
    }

    // Pad remaining lines so layout is stable
    let shown = visible.len();
    for _ in shown..height {
        execute!(out, Print("\n")).ok();
    }
}
```

```rust
// src/ui/prompt.rs

pub fn draw(out: &mut impl Write, cwd: &str, last_exit: i32) {
    let short_cwd = abbreviate_home(cwd);

    // Exit code indicator: green ❯ on success, red ✗ on failure
    let (indicator, colour) = if last_exit == 0 {
        ("❯", Color::Cyan)
    } else {
        (format!("[{}] ❯", last_exit).as_str(), Color::Red)
        // note: handle this cleanly in real code using a String
    };

    execute!(out,
        SetForegroundColor(Color::Blue),
        SetAttribute(Attribute::Bold),
        Print(format!("  {} ", short_cwd)),
        ResetColor,
        SetForegroundColor(colour),
        Print(format!("{} ", indicator)),
        ResetColor,
    ).ok();

    out.flush().ok();
}

fn abbreviate_home(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let h = home.to_string_lossy();
        if path.starts_with(h.as_ref()) {
            return path.replacen(h.as_ref(), "~", 1);
        }
    }
    path.to_string()
}
```

---

## 8. OpenCode Integration Reference

### 8.1 Installation

```bash
# Via npm (recommended)
npm i -g opencode-ai

# Via curl installer
curl -fsSL https://opencode.ai/install | bash

# Verify
opencode --version
```

### 8.2 CLI Interface Used by RustProc Shell

| RustProc Command | OpenCode CLI Invocation |
|---|---|
| `ai "prompt"` | `opencode run --print "prompt"` |
| `ai explain f` | `opencode run --print "<file contents + explain prompt>"` |
| `opencode` | `opencode` (full interactive session) |

> **Important:** OpenCode's CLI flags are actively evolving. Before finalising `ai.rs`, run `opencode --help` and `opencode run --help` to confirm the exact flag names. The subprocess invocation pattern will not change — only the flag string may differ.

### 8.3 Fallback Behaviour (OpenCode Not Installed)

If `opencode_available()` returns `false` at startup:

- `ai`, `ask`, `opencode` commands print: `"AI: OpenCode not installed. Run: npm i -g opencode-ai"`
- All other shell and monitoring features function normally
- `help` displays the install message in red
- The startup banner shows the AI status as a red warning

This means the project is demonstrable on machines without OpenCode — just with the AI features disabled.

---

## 9. Config File

```toml
# ~/.config/rustproc/config.toml

[display]
tick_rate_ms    = 1000          # monitor refresh interval
max_output_lines = 500          # output buffer cap
show_process_table = true       # show processes in header (set false for minimal header)

[shell]
default_editor  = "nano"        # fallback if $EDITOR not set
history_file    = "~/.rustproc_history"
history_limit   = 1000

[ai]
opencode_path   = "opencode"    # path to opencode binary (default: from $PATH)
max_response_lines = 200        # truncate AI responses longer than this

[thresholds]
cpu_warn = 50.0
cpu_crit = 80.0
ram_warn = 60.0
ram_crit = 85.0
gpu_warn = 50.0
gpu_crit = 80.0
```

---

## 10. Cargo.toml

```toml
[package]
name    = "rustproc-shell"
version = "0.3.0"
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

# Optional — NVIDIA GPU telemetry
nvml-wrapper = { version = "0.9", optional = true }

[profile.release]
opt-level = 3
strip     = true
lto       = true
```

---

## 11. Implementation Phases

### Phase 1 — Scaffold and Layout (45 minutes)

Goal: project compiles, screen layout renders with placeholder data, exits cleanly.

- `cargo new rustproc-shell`
- Create all source files with stub implementations
- Implement `ui::draw` with hardcoded `AppState::default()`
- Implement `ui::cleanup` to restore terminal
- Confirm `cargo run` opens layout and exits on Ctrl+C

Success criterion: the three-zone layout is visible with placeholder values. Terminal is clean after exit.

---

### Phase 2 — Monitor Thread (45 minutes)

Goal: real hardware stats in the header.

- Implement `state.rs` fully
- Implement `monitor::run` using `sysinfo`
- Spawn monitor thread from `main.rs`
- Replace placeholder values in `header.rs` with live `AppState` reads
- Verify: RAM matches `free -h`, CPU changes under load

Success criterion: CPU bar visibly changes when running `stress --cpu 2` in another terminal.

---

### Phase 3 — Shell Input and External Commands (1 hour)

Goal: user can type and execute real commands.

- Implement `input::readline` with raw mode, character echo, and backspace
- Implement `executor::run` for foreground external commands
- Implement `router::classify` for the `External` arm only
- Wire `shell::run` loop: draw → readline → execute → append → repeat
- Implement `ui::output::draw` with colour-coded output lines

Success criterion: `ls`, `pwd`, `echo hello`, `cargo --version` all produce correct output in the panel.

---

### Phase 4 — Built-in Commands (45 minutes)

Goal: `cd`, `help`, `clear`, `view`, `edit`, `sysinfo`, `exit` all work.

- Implement all built-ins in `builtins.rs`
- Wire them through `router.rs` and `shell::run`
- Verify `cd /tmp` updates prompt; `pwd` confirms it
- Verify `view main.rs` shows line-numbered file contents
- Verify `edit main.rs` opens nano (or `$EDITOR`) and returns cleanly

Success criterion: all 7 built-ins execute correctly without crashing.

---

### Phase 5 — Command History (30 minutes)

Goal: up/down arrow key navigation through previous commands.

- Implement `history.rs` with `push`, `prev`, `next`
- Extend `input::readline` with `KeyCode::Up` and `KeyCode::Down` handling
- Implement history cursor reset on `Enter`

Success criterion: after 5 commands, pressing up cycles through all 5; pressing down returns to empty prompt.

---

### Phase 6 — AI Integration: Free-Form Prompt (45 minutes)

Goal: `ai "..."` and `ask "..."` work end-to-end.

- Implement `opencode_available()` check in `ai.rs`
- Call it in `main.rs` and pass `opencode_ok: bool` through to `shell::run`
- Implement `ai_prompt()` using `Command::new("opencode")` with correct flags
- Wire `AiCmd::Prompt` through `router.rs` and `shell::run`
- Test with a simple prompt: `ai "write hello world in Rust"`

Success criterion: the AI response appears in the output panel in cyan, delimited by the header/footer separator lines.

---

### Phase 7 — AI Integration: File Commands (45 minutes)

Goal: `ai explain`, `ai refactor`, `ai fix`, `ai gen` all work.

- Implement `read_file_for_ai`, `ai_explain`, `ai_refactor`, `ai_fix`, `ai_generate`
- Implement `extract_code_block` for `ai gen` file writing
- Wire all four through router and shell

Success criterion: `ai explain main.rs` returns a coherent explanation of the file; `ai gen out.rs "write bubble sort"` creates the file.

---

### Phase 8 — Interactive OpenCode Launch (20 minutes)

Goal: `opencode` command hands terminal to OpenCode and reclaims it on exit.

- Implement `launch_opencode()` with terminal disable/enable raw mode handoff
- Test: `opencode` launches the full OpenCode session; pressing its exit key returns to RustProc Shell with stats intact

Success criterion: `opencode` opens, user interacts with it, exits back to RustProc Shell without visual artefacts.

---

### Phase 9 — Background Process Support (20 minutes)

Goal: `cargo build &` returns the prompt immediately.

- Extend `executor::run` to detect trailing `&`
- Use `.spawn()` instead of `.output()` for background commands
- Return a dim `[background] cmd` line

Success criterion: `sleep 10 &` returns prompt in under 100ms. The `sleep` PID appears in the process table in the header.

---

### Phase 10 — GPU Support (30 minutes)

Goal: GPU stats in header on NVIDIA hardware; "N/A" on others.

- Implement `gpu.rs` with `nvml-wrapper` behind the `nvidia` feature flag
- Add `gpu_available`, `gpu_usage_pct`, `gpu_temp_c` to `AppState`
- Update `monitor::run` to call `gpu::collect()`
- Update `header::draw_stats` to render GPU line

Success criterion: on an NVIDIA machine, GPU % and temp appear. On a machine without NVML, the header shows "GPU N/A" and does not panic.

---

### Phase 11 — Hardening and Demo Prep (45 minutes)

Goal: zero crashes under any demo scenario.

- Run `cargo clippy --all-features -- -D warnings`, resolve all warnings
- Run `cargo fmt --check`
- Test: open shell, run 30 mixed commands (shell + AI + file ops), no crash
- Test: run on a machine without OpenCode installed — AI commands show install message, nothing else breaks
- Test: resize the terminal window while the shell is running — layout adapts, no visual corruption
- Test: `Ctrl+C` during command input clears the line (does not kill the shell)
- Test: run `ai explain` on a non-existent file — prints error, shell continues

---

## 12. OS Concepts Coverage Table

Use this in your report and presentation:

| Feature | OS Concept | Location in Code |
|---|---|---|
| `Command::new().spawn()` | Process creation — `fork` + `exec` | `executor.rs`, `ai.rs` |
| `output.status.code()` | Process termination — `waitpid` exit codes | `executor.rs` |
| Background `&` command | Process states: foreground vs background | `executor.rs` |
| Process table in header | Process scheduling visibility | `monitor.rs` |
| `sysinfo` CPU usage | CPU scheduling metrics | `monitor.rs` |
| RAM gauge | Memory management: allocation tracking | `monitor.rs` |
| GPU VRAM usage | Device and resource management | `gpu.rs` |
| `Arc<Mutex<AppState>>` | Concurrency: mutual exclusion | `state.rs`, `monitor.rs` |
| Background monitor thread | Concurrency: multi-threading | `main.rs` |
| `std::env::set_current_dir` | Per-process working directory (kernel PCB attribute) | `builtins.rs` |
| Built-in `cd` vs external | Why some commands must be built-in | `builtins.rs` |
| `Stdio::piped()` | IPC via anonymous pipes | `executor.rs`, `ai.rs` |
| OpenCode subprocess handoff | Process hierarchy, `exec` family | `ai.rs` |
| Terminal raw mode | TTY control, user-space/kernel interface | `input.rs` |
| Shell prompt loop | User-space process as kernel interface | `shell/mod.rs` |

---

## 13. Demo Script (4-Minute Walkthrough)

**Minute 1 — Shell and Monitor**

```
cargo run
```

Shell opens. Point at the header: CPU, RAM, GPU, PID count — all live, updating every second.

```
ls -la
pwd
cd src
ls
```

Show that the prompt updates with `cd`. The output panel accumulates across commands.

**Minute 2 — System Integration**

```
sysinfo
```

Snapshot printed inline. Then:

```bash
# In another terminal pane (or via background):
stress --cpu 4 &
```

CPU bar in the header visibly climbs while the shell remains responsive. Kill it:

```
kill [PID]
```

Find the PID from the process table in the header. CPU bar drops back.

**Minute 3 — AI Integration**

```
ai "write a merge sort function in Rust"
```

AI response streams into the output panel in cyan. Then:

```
view main.rs
ai explain main.rs
```

Shell reads the file, sends contents to OpenCode, explanation appears.

```
ai gen sorted.rs "write bubble sort in Rust with comments"
ls
view sorted.rs
```

AI generates code. File is written. Verify with `view`.

**Minute 4 — Architecture Explanation**

Point at the code structure on screen:

- Two threads: monitor (background) and shell (foreground). `Arc<Mutex<>>` shared state.
- `router.rs` classifies every input: built-in, AI, or OS.
- `ai.rs` constructs a subprocess call to OpenCode — no AI model embedded, same pattern as `git` calling `gpg`.
- Built-in `cd` exists because changing directory modifies a per-process kernel attribute — an external `cd` would change its own directory, then exit.

Close:

```
help
exit
```

Terminal restored cleanly.

---

## 14. Build and Run Reference

```bash
# Development
cargo run

# Without NVIDIA GPU support
cargo run --no-default-features

# Release binary
cargo build --release
./target/release/rustproc-shell

# Install OpenCode (prerequisite for AI features)
npm i -g opencode-ai
# or
curl -fsSL https://opencode.ai/install | bash

# Lint (run before demo)
cargo clippy --all-features -- -D warnings
cargo fmt --check
```

---

## 15. Final Checklist

### Shell
- [ ] External commands execute and capture stdout + stderr
- [ ] `cd` updates working directory and prompt path
- [ ] `exit` restores terminal cleanly — cursor visible, no artefacts
- [ ] `help` lists all built-ins and shows AI status
- [ ] `clear` clears the output panel
- [ ] `view [file]` shows file with line numbers
- [ ] `edit [file]` opens `$EDITOR` and returns cleanly
- [ ] `sysinfo` prints hardware snapshot
- [ ] Up/Down arrows navigate command history
- [ ] `Ctrl+C` clears current input line, does not kill shell
- [ ] Background `&` returns prompt immediately

### Monitor
- [ ] CPU bar and % match `htop` output
- [ ] RAM values match `free -h`
- [ ] GPU shows "N/A" gracefully on non-NVIDIA hardware
- [ ] Stats update while user is typing at the prompt

### AI
- [ ] `ai "prompt"` returns response in cyan, delimited by separator lines
- [ ] `ask "..."` is a working alias for `ai "..."`
- [ ] `ai explain [file]` reads file and returns explanation
- [ ] `ai refactor [file]` returns concrete refactoring suggestions
- [ ] `ai fix [file]` identifies and explains bugs
- [ ] `ai gen [file] "prompt"` generates code AND writes the file
- [ ] `opencode` launches interactive session and returns to shell on exit
- [ ] All AI commands print a helpful install message if OpenCode is not found

### Stability
- [ ] No `unwrap()` in command handlers — all fallible paths handled
- [ ] Output buffer capped at 500 lines
- [ ] Terminal fully restored after `exit`
- [ ] No crash when running AI commands on non-existent files
- [ ] `cargo clippy --all-features -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] Tested: 30 mixed commands without crash or visual artefact
- [ ] Tested on actual demo machine
