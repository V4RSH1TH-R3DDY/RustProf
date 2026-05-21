use std::process::Command;

use crate::{
    app::SharedState,
    commands::{CommandResult, OutputLine, ShellError},
};

pub fn run(raw: &str, state: &SharedState) -> CommandResult {
    let command = raw.trim();
    if command.is_empty() {
        return Ok(vec![]);
    }

    let background = command.ends_with('&');
    let command = command.trim_end_matches('&').trim();
    let cwd = state
        .lock()
        .map(|state| state.cwd.clone())
        .unwrap_or_else(|_| ".".to_owned());

    let mut process = shell_command(command);
    process.current_dir(cwd);

    if background {
        let child = process
            .spawn()
            .map_err(|err| ShellError(format!("failed to start background command: {}", err)))?;
        if let Ok(mut state) = state.lock() {
            state.last_exit_code = 0;
        }
        return Ok(vec![OutputLine::dim(format!(
            "[background pid {}] {}",
            child.id(),
            command
        ))]);
    }

    let output = process
        .output()
        .map_err(|err| ShellError(format!("failed to execute command: {}", err)))?;
    let exit_code = output.status.code().unwrap_or(1);
    if let Ok(mut state) = state.lock() {
        state.last_exit_code = exit_code;
    }

    let mut lines = Vec::new();
    lines.extend(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(OutputLine::normal),
    );
    lines.extend(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .map(OutputLine::error),
    );
    if !output.status.success() {
        lines.push(OutputLine::dim(format!("[exit {}]", exit_code)));
    }
    Ok(lines)
}

fn shell_command(command: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut process = Command::new("cmd");
        process.args(["/C", command]);
        process
    } else {
        let mut process = Command::new("sh");
        process.args(["-c", command]);
        process
    }
}
