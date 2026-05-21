#[derive(Debug, Default)]
pub struct History {
    entries: Vec<String>,
    cursor: Option<usize>,
}

impl History {
    pub fn push(&mut self, command: String) {
        if command.trim().is_empty() {
            return;
        }
        if self.entries.last() != Some(&command) {
            self.entries.push(command);
        }
        self.cursor = None;
    }

    pub fn previous(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let index = self
            .cursor
            .map(|index| index.saturating_sub(1))
            .unwrap_or_else(|| self.entries.len().saturating_sub(1));
        self.cursor = Some(index);
        self.entries.get(index).map(String::as_str)
    }

    pub fn next(&mut self) -> Option<&str> {
        let index = self.cursor?;
        if index + 1 >= self.entries.len() {
            self.cursor = None;
            return Some("");
        }
        let next = index + 1;
        self.cursor = Some(next);
        self.entries.get(next).map(String::as_str)
    }

    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn lines(&self) -> impl Iterator<Item = (usize, &String)> {
        self.entries.iter().enumerate()
    }
}
