use std::{env, io, path::PathBuf, process::Command};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::{
    app::SharedState,
    commands::{CommandResult, OutputLine, ShellError},
    shell::history::History,
};

pub fn cd(args: &str, state: &SharedState) -> CommandResult {
    let target = if args.trim().is_empty() {
        "~"
    } else {
        args.trim()
    };
    let expanded = expand_tilde(target);
    let cwd = state
        .lock()
        .map(|state| PathBuf::from(&state.cwd))
        .unwrap_or_else(|_| PathBuf::from("."));
    let next = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    let canonical = next
        .canonicalize()
        .map_err(|err| ShellError(format!("cd: {}", err)))?;
    if !canonical.is_dir() {
        return Err(ShellError("cd: target is not a directory".to_owned()));
    }

    env::set_current_dir(&canonical).map_err(|err| ShellError(format!("cd: {}", err)))?;
    if let Ok(mut state) = state.lock() {
        state.cwd = canonical.display().to_string();
        state.last_exit_code = 0;
    }
    Ok(vec![])
}

pub fn view(path: &str) -> CommandResult {
    let path = path.trim();
    if path.is_empty() {
        return Err(ShellError("view: no file specified".to_owned()));
    }

    let content =
        std::fs::read_to_string(path).map_err(|err| ShellError(format!("view: {}", err)))?;
    let mut lines = vec![OutputLine::dim(format!("-- {} --", path))];
    lines.extend(
        content
            .lines()
            .enumerate()
            .map(|(index, line)| OutputLine::normal(format!("{:>4}  {}", index + 1, line))),
    );
    Ok(lines)
}

pub fn edit(path: &str) -> CommandResult {
    let path = path.trim();
    if path.is_empty() {
        return Err(ShellError("edit: no file specified".to_owned()));
    }

    let editor = env::var("EDITOR").unwrap_or_else(|_| "nano".to_owned());
    disable_raw_mode().ok();
    let status = Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|err| ShellError(format!("{}: {}", editor, err)));
    enable_raw_mode().ok();

    let status = status?;
    Ok(vec![OutputLine::dim(format!(
        "[closed {} in {}, exit {}]",
        path,
        editor,
        status.code().unwrap_or(0)
    ))])
}

pub fn project(path: &str, state: &SharedState) -> CommandResult {
    cd(path, state)?;
    let cwd = state
        .lock()
        .map(|state| state.cwd.clone())
        .unwrap_or_else(|_| ".".to_owned());
    let mut counts = (0usize, 0usize);
    if let Ok(entries) = std::fs::read_dir(&cwd) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                counts.0 += 1;
            } else {
                counts.1 += 1;
            }
        }
    }
    Ok(vec![
        OutputLine::success(format!("project: {}", cwd)),
        OutputLine::dim(format!(
            "{} directories, {} files at top level",
            counts.0, counts.1
        )),
    ])
}

pub fn save(path: &str, last_ai_response: &str) -> CommandResult {
    let path = path.trim();
    if path.is_empty() {
        return Err(ShellError("save: no file specified".to_owned()));
    }
    if last_ai_response.trim().is_empty() {
        return Err(ShellError("save: no AI response to write".to_owned()));
    }
    std::fs::write(path, last_ai_response).map_err(|err| ShellError(format!("save: {}", err)))?;
    Ok(vec![OutputLine::success(format!("written to {}", path))])
}

pub fn sysinfo_snapshot(state: &SharedState) -> CommandResult {
    let state = state.lock().map(|state| state.clone()).unwrap_or_default();
    let memory_pct = if state.total_memory_mb == 0 {
        0.0
    } else {
        100.0 * state.used_memory_mb as f64 / state.total_memory_mb as f64
    };

    Ok(vec![
        OutputLine::normal(format!(
            "CPU: {:.1}% | {} ({}) | {} logical cores | max {} MHz",
            state.cpu_usage,
            state.cpu_brand,
            state.cpu_vendor,
            state.cpu_logical_cores,
            state.cpu_frequency_mhz
        )),
        OutputLine::normal(format!(
            "RAM: {} / {} MB ({:.0}%)",
            state.used_memory_mb, state.total_memory_mb, memory_pct
        )),
        OutputLine::normal(format!(
            "GPU: {} {} | VRAM {} | temp {} | source {}",
            state.gpu.vendor,
            state.gpu.model,
            format_vram(state.gpu.vram_used_mb, state.gpu.vram_total_mb),
            format_temp(state.gpu.temperature_c),
            state.gpu.source
        )),
        OutputLine::normal(format!("PIDs: {}", state.process_count)),
    ])
}

pub fn history(history: &History) -> CommandResult {
    Ok(history
        .lines()
        .map(|(index, command)| OutputLine::normal(format!("{:>4}  {}", index + 1, command)))
        .collect())
}

pub fn help(opencode_ok: bool) -> CommandResult {
    let ai_status = if opencode_ok {
        OutputLine::success("AI Status: OpenCode connected")
    } else {
        OutputLine::warning("AI Status: OpenCode not found; install opencode to use ai commands")
    };

    Ok(vec![
        OutputLine::ai_header("-- RustProc Shell --"),
        OutputLine::normal("Shell: cd, clear, edit, help, history, project, save, sysinfo, view"),
        OutputLine::normal("AI: ai \"prompt\", ask \"question\", ai explain/refactor/fix <file>, ai gen <file> \"prompt\", opencode"),
        OutputLine::normal("External commands run from the displayed cwd. Append & for background execution."),
        ai_status,
    ])
}

fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("."));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home_dir().unwrap_or_else(|| PathBuf::from(".")).join(rest);
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn format_vram(used: Option<u64>, total: Option<u64>) -> String {
    match (used, total) {
        (Some(used), Some(total)) => format!("{} / {} MB", used, total),
        (None, Some(total)) => format!("{} MB", total),
        _ => "not reported".to_owned(),
    }
}

fn format_temp(temp: Option<f32>) -> String {
    temp.map(|temp| format!("{:.1} C", temp))
        .unwrap_or_else(|| "n/a".to_owned())
}

impl From<io::Error> for ShellError {
    fn from(value: io::Error) -> Self {
        ShellError(value.to_string())
    }
}
