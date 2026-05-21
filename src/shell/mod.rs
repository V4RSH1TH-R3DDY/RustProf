pub mod history;
pub mod router;

use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::{
    app::SharedState,
    commands::{ai, builtins, executor, CommandResult, LineKind, OutputLine, ShellError},
    shell::{
        history::History,
        router::{AiCmd, BuiltinCmd, DispatchTarget},
    },
    ui,
};

const MAX_LINES: usize = 500;

pub fn run(state: SharedState, opencode_ok: bool) -> io::Result<()> {
    let mut input = String::new();
    let mut output = vec![
        OutputLine::success("RustProc Shell ready. Type 'help' for commands."),
        if opencode_ok {
            OutputLine::success("AI: OpenCode connected.")
        } else {
            OutputLine::warning("AI: OpenCode not found. AI commands will show install guidance.")
        },
    ];
    let mut history = History::default();
    let mut last_ai_response = String::new();

    loop {
        draw(&state, &output, &input)?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.clear();
                history.reset_cursor();
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Char(ch) => {
                input.push(ch);
                history.reset_cursor();
            }
            KeyCode::Backspace => {
                input.pop();
            }
            KeyCode::Enter => {
                let command = input.trim().to_owned();
                input.clear();
                history.reset_cursor();
                if command.is_empty() {
                    continue;
                }

                history.push(command.clone());
                output.push(OutputLine::dim(format!("$ {}", command)));

                match dispatch(&command, &state, &history, opencode_ok, &last_ai_response) {
                    DispatchOutcome::Exit => break,
                    DispatchOutcome::Clear => output.clear(),
                    DispatchOutcome::Lines(result) => match result {
                        Ok(lines) => {
                            update_last_ai_response(&lines, &mut last_ai_response);
                            output.extend(lines);
                        }
                        Err(err) => output.push(OutputLine::error(err.to_string())),
                    },
                }

                if output.len() > MAX_LINES {
                    output.drain(0..output.len() - MAX_LINES);
                }
            }
            KeyCode::Esc => break,
            KeyCode::Up => {
                if let Some(previous) = history.previous() {
                    input = previous.to_owned();
                }
            }
            KeyCode::Down => {
                if let Some(next) = history.next() {
                    input = next.to_owned();
                }
            }
            _ => {}
        }
    }

    Ok(())
}

enum DispatchOutcome {
    Lines(CommandResult),
    Clear,
    Exit,
}

fn dispatch(
    command: &str,
    state: &SharedState,
    history: &History,
    opencode_ok: bool,
    last_ai_response: &str,
) -> DispatchOutcome {
    match router::classify(command) {
        DispatchTarget::Empty => DispatchOutcome::Lines(Ok(vec![])),
        DispatchTarget::Builtin(BuiltinCmd::Exit) => DispatchOutcome::Exit,
        DispatchTarget::Builtin(BuiltinCmd::Clear) => DispatchOutcome::Clear,
        DispatchTarget::Builtin(cmd) => DispatchOutcome::Lines(dispatch_builtin(
            cmd,
            state,
            history,
            opencode_ok,
            last_ai_response,
        )),
        DispatchTarget::AiCommand(cmd) => {
            if !opencode_ok {
                DispatchOutcome::Lines(ai::unavailable_message())
            } else {
                DispatchOutcome::Lines(dispatch_ai(cmd))
            }
        }
        DispatchTarget::External(raw) => DispatchOutcome::Lines(executor::run(raw, state)),
    }
}

fn dispatch_builtin(
    command: BuiltinCmd<'_>,
    state: &SharedState,
    history: &History,
    opencode_ok: bool,
    last_ai_response: &str,
) -> CommandResult {
    match command {
        BuiltinCmd::Cd(path) => builtins::cd(path, state),
        BuiltinCmd::Help => builtins::help(opencode_ok),
        BuiltinCmd::History => builtins::history(history),
        BuiltinCmd::Sysinfo => builtins::sysinfo_snapshot(state),
        BuiltinCmd::View(path) => builtins::view(path),
        BuiltinCmd::Save(path) => builtins::save(path, last_ai_response),
        BuiltinCmd::Edit(path) => builtins::edit(path),
        BuiltinCmd::Project(path) => builtins::project(path, state),
        BuiltinCmd::Clear | BuiltinCmd::Exit => {
            Err(ShellError("internal dispatch error".to_owned()))
        }
    }
}

fn dispatch_ai(command: AiCmd<'_>) -> CommandResult {
    match command {
        AiCmd::Prompt(prompt) => ai::ai_prompt(prompt),
        AiCmd::Explain(path) => ai::ai_explain(path),
        AiCmd::Refactor(path) => ai::ai_refactor(path),
        AiCmd::Fix(path) => ai::ai_fix(path),
        AiCmd::Generate(path, prompt) => ai::ai_generate(path, prompt),
        AiCmd::OpenCode => ai::launch_opencode(),
    }
}

fn draw(state: &SharedState, output_lines: &[OutputLine], input: &str) -> io::Result<()> {
    let state = state.lock().map(|state| state.clone()).unwrap_or_default();
    ui::draw(&state, output_lines, input)
}

fn update_last_ai_response(lines: &[OutputLine], last_ai_response: &mut String) {
    let ai_lines = lines
        .iter()
        .filter(|line| matches!(line.kind, LineKind::Ai))
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>();
    if !ai_lines.is_empty() {
        *last_ai_response = ai_lines.join("\n");
    }
}
