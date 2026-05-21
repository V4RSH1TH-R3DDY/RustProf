use std::{
    env, io,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use sysinfo::{Components, System};

use crate::app::{AppState, SharedState};

mod app;
mod commands;
mod shell;
mod system;
mod ui;

fn main() -> io::Result<()> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let state: SharedState = Arc::new(Mutex::new(AppState {
        cwd: cwd.display().to_string(),
        ..AppState::default()
    }));

    spawn_monitor(Arc::clone(&state));
    let opencode_ok = commands::ai::opencode_available();
    run_shell(state, opencode_ok)
}

fn spawn_monitor(state: SharedState) {
    thread::spawn(move || {
        let mut sys = System::new_all();
        let mut components = Components::new_with_refreshed_list();

        loop {
            let cwd = state
                .lock()
                .map(|state| state.cwd.clone())
                .unwrap_or_else(|_| ".".to_owned());
            let mut next_state = system::collect(&mut sys, &mut components, cwd);
            if let Ok(mut state) = state.lock() {
                next_state.last_exit_code = state.last_exit_code;
                *state = next_state;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn run_shell(state: SharedState, opencode_ok: bool) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, Hide)?;

    let result = shell::run(state, opencode_ok);

    execute!(io::stdout(), Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    result
}
