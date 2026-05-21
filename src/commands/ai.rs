use std::process::{Command, Stdio};

use crossterm::{
    execute,
    style::ResetColor,
    terminal::{disable_raw_mode, enable_raw_mode},
};

use crate::commands::{CommandResult, OutputLine, ShellError};

pub fn opencode_available() -> bool {
    Command::new("opencode")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn unavailable_message() -> CommandResult {
    Ok(vec![OutputLine::error(
        "AI: OpenCode not installed or not on PATH. Install it, then restart RustProc.",
    )])
}

pub fn ai_prompt(prompt: &str) -> CommandResult {
    if prompt.trim().is_empty() {
        return Err(ShellError("ai: no prompt specified".to_owned()));
    }

    let mut lines = vec![OutputLine::ai_header("-- AI Response --")];
    let output = Command::new("opencode")
        .args(["run", "--print", prompt])
        .output()
        .map_err(|err| ShellError(format!("opencode: {}", err)))?;

    if output.status.success() {
        let response = String::from_utf8_lossy(&output.stdout);
        lines.extend(response.lines().map(OutputLine::ai));
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        lines.push(OutputLine::error(format!(
            "opencode error: {}",
            stderr.trim()
        )));
    }
    lines.push(OutputLine::ai_header("-- end AI Response --"));
    Ok(lines)
}

pub fn ai_explain(file_path: &str) -> CommandResult {
    let content = read_file_for_ai(file_path)?;
    let prompt = format!(
        "Explain what this code does. Be concise and focus on key logic:\n\n```\n{}\n```",
        content
    );
    let mut lines = vec![OutputLine::dim(format!("ai explain {}", file_path))];
    lines.extend(ai_prompt(&prompt)?);
    Ok(lines)
}

pub fn ai_refactor(file_path: &str) -> CommandResult {
    let content = read_file_for_ai(file_path)?;
    let prompt = format!(
        "Suggest specific refactoring improvements for this code. Focus on readability, idiomatic style, and correctness:\n\n```\n{}\n```",
        content
    );
    let mut lines = vec![OutputLine::dim(format!("ai refactor {}", file_path))];
    lines.extend(ai_prompt(&prompt)?);
    Ok(lines)
}

pub fn ai_fix(file_path: &str) -> CommandResult {
    let content = read_file_for_ai(file_path)?;
    let prompt = format!(
        "Identify bugs and issues in this code and provide corrected versions. Explain each fix:\n\n```\n{}\n```",
        content
    );
    let mut lines = vec![OutputLine::dim(format!("ai fix {}", file_path))];
    lines.extend(ai_prompt(&prompt)?);
    Ok(lines)
}

pub fn ai_generate(file_path: &str, prompt_text: &str) -> CommandResult {
    if file_path.trim().is_empty() {
        return Err(ShellError("ai gen: no output file specified".to_owned()));
    }
    if prompt_text.trim().is_empty() {
        return Err(ShellError("ai gen: no prompt specified".to_owned()));
    }

    let prompt = format!(
        "{}\n\nWrite clean code. Output only code, preferably in a fenced code block.",
        prompt_text
    );
    let mut lines = vec![OutputLine::dim(format!(
        "ai gen {} \"{}\"",
        file_path, prompt_text
    ))];
    let result = ai_prompt(&prompt)?;
    let response_text = result
        .iter()
        .filter(|line| matches!(line.kind, crate::commands::LineKind::Ai))
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    lines.extend(result);

    let code = extract_code_block(&response_text);
    std::fs::write(file_path, code).map_err(|err| {
        ShellError(format!(
            "ai gen: could not write generated code to {}: {}",
            file_path, err
        ))
    })?;
    lines.push(OutputLine::success(format!("written to {}", file_path)));
    Ok(lines)
}

pub fn launch_opencode() -> CommandResult {
    disable_raw_mode().ok();
    execute!(std::io::stdout(), ResetColor).ok();
    let status = Command::new("opencode")
        .status()
        .map_err(|err| ShellError(format!("opencode: {}", err)));
    enable_raw_mode().ok();

    let status = status?;
    Ok(vec![OutputLine::dim(format!(
        "[opencode exited with code {}]",
        status.code().unwrap_or(0)
    ))])
}

fn read_file_for_ai(path: &str) -> Result<String, ShellError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(ShellError("ai: no file specified".to_owned()));
    }
    std::fs::read_to_string(path)
        .map_err(|err| ShellError(format!("cannot read {}: {}", path, err)))
}

fn extract_code_block(response: &str) -> String {
    let lines = response.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with("```"));
    let end = lines
        .iter()
        .rposition(|line| line.trim_start().starts_with("```"));

    match (start, end) {
        (Some(start), Some(end)) if end > start => lines[start + 1..end].join("\n"),
        _ => response.to_owned(),
    }
}
