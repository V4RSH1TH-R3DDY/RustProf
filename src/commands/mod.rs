pub mod ai;
pub mod builtins;
pub mod executor;

pub type CommandResult = Result<Vec<OutputLine>, ShellError>;

#[derive(Debug, Clone)]
pub struct OutputLine {
    pub text: String,
    pub kind: LineKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineKind {
    Normal,
    Ai,
    AiHeader,
    Success,
    Warning,
    Error,
    Dim,
}

#[derive(Debug)]
pub struct ShellError(pub String);

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ShellError {}

impl OutputLine {
    pub fn normal(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Normal,
        }
    }

    pub fn ai(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Ai,
        }
    }

    pub fn ai_header(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::AiHeader,
        }
    }

    pub fn success(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Success,
        }
    }

    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Warning,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Error,
        }
    }

    pub fn dim(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: LineKind::Dim,
        }
    }
}
