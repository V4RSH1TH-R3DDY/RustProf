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
    Save(&'a str),
    Edit(&'a str),
    Project(&'a str),
    Exit,
}

pub enum AiCmd<'a> {
    Prompt(&'a str),
    Explain(&'a str),
    Refactor(&'a str),
    Fix(&'a str),
    Generate(&'a str, &'a str),
    OpenCode,
}

pub fn classify(raw: &str) -> DispatchTarget<'_> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DispatchTarget::Empty;
    }

    let (cmd, rest) = split_first_word(trimmed);
    match cmd {
        "exit" | "quit" => DispatchTarget::Builtin(BuiltinCmd::Exit),
        "cd" => DispatchTarget::Builtin(BuiltinCmd::Cd(rest)),
        "help" => DispatchTarget::Builtin(BuiltinCmd::Help),
        "clear" => DispatchTarget::Builtin(BuiltinCmd::Clear),
        "history" => DispatchTarget::Builtin(BuiltinCmd::History),
        "sysinfo" => DispatchTarget::Builtin(BuiltinCmd::Sysinfo),
        "view" => DispatchTarget::Builtin(BuiltinCmd::View(rest)),
        "save" => DispatchTarget::Builtin(BuiltinCmd::Save(rest)),
        "edit" => DispatchTarget::Builtin(BuiltinCmd::Edit(rest)),
        "project" => DispatchTarget::Builtin(BuiltinCmd::Project(rest)),
        "ai" | "ask" => classify_ai(rest),
        "opencode" => DispatchTarget::AiCommand(AiCmd::OpenCode),
        "run" => DispatchTarget::External(rest),
        _ => DispatchTarget::External(trimmed),
    }
}

fn classify_ai(rest: &str) -> DispatchTarget<'_> {
    let (subcmd, arg) = split_first_word(rest);
    match subcmd {
        "explain" => DispatchTarget::AiCommand(AiCmd::Explain(arg)),
        "refactor" => DispatchTarget::AiCommand(AiCmd::Refactor(arg)),
        "fix" => DispatchTarget::AiCommand(AiCmd::Fix(arg)),
        "gen" | "generate" => {
            let (file, prompt) = split_first_word(arg);
            DispatchTarget::AiCommand(AiCmd::Generate(file, trim_matching_quotes(prompt)))
        }
        _ => DispatchTarget::AiCommand(AiCmd::Prompt(trim_matching_quotes(rest))),
    }
}

fn split_first_word(value: &str) -> (&str, &str) {
    let value = value.trim();
    match value.find(char::is_whitespace) {
        Some(index) => (&value[..index], value[index..].trim()),
        None => (value, ""),
    }
}

fn trim_matching_quotes(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}
