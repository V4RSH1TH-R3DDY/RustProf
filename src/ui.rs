use std::io::{self, Write};

use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};

use crate::{
    app::AppState,
    commands::{LineKind, OutputLine},
};

const SYSTEM_TOP: u16 = 4;
const SYSTEM_HEIGHT: u16 = 7;
const PROCESS_TOP: u16 = 13;
const PROCESS_HEIGHT: u16 = 8;

pub fn draw(state: &AppState, output_lines: &[OutputLine], input: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    let (width, height) = terminal::size().unwrap_or((100, 32));
    let width = width.max(20);
    let height = height.max(8);
    if width < 74 || height < 26 {
        return draw_too_small(&mut stdout, width, height);
    }

    let shell_divider = height.saturating_sub(8).max(21);
    let prompt_divider = height.saturating_sub(3);

    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    draw_outer_frame(&mut stdout, width, height, shell_divider, prompt_divider)?;
    draw_header(&mut stdout, state, width)?;
    draw_system_panel(&mut stdout, state, width)?;
    draw_process_panel(&mut stdout, state, width)?;
    draw_shell_output(
        &mut stdout,
        output_lines,
        width,
        shell_divider,
        prompt_divider,
    )?;
    draw_prompt(&mut stdout, state, input, width, height)?;
    stdout.flush()
}

fn draw_too_small(stdout: &mut io::Stdout, width: u16, height: u16) -> io::Result<()> {
    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
    let message = "RustProc needs at least 74x26";
    let hint = "Resize terminal or zoom out";
    let y = height.saturating_div(2).saturating_sub(1);
    print_at(
        stdout,
        0,
        y,
        Color::Cyan,
        &pad(&truncate(message, width as usize), width as usize),
    )?;
    if height > y + 1 {
        print_at(
            stdout,
            0,
            y + 1,
            Color::DarkGrey,
            &pad(&truncate(hint, width as usize), width as usize),
        )?;
    }
    stdout.flush()
}

fn draw_outer_frame(
    stdout: &mut io::Stdout,
    width: u16,
    height: u16,
    shell_divider: u16,
    prompt_divider: u16,
) -> io::Result<()> {
    print_at(
        stdout,
        0,
        0,
        Color::Cyan,
        &format!("╔{}╗", "═".repeat(inner(width))),
    )?;
    print_at(
        stdout,
        0,
        2,
        Color::Cyan,
        &format!("╠{}╣", "═".repeat(inner(width))),
    )?;
    print_at(
        stdout,
        0,
        shell_divider,
        Color::Cyan,
        &format!("╠{}╣", "═".repeat(inner(width))),
    )?;
    print_at(
        stdout,
        0,
        prompt_divider,
        Color::Cyan,
        &format!("╠{}╣", "═".repeat(inner(width))),
    )?;
    print_at(
        stdout,
        0,
        height - 1,
        Color::Cyan,
        &format!("╚{}╝", "═".repeat(inner(width))),
    )?;

    for y in 1..height - 1 {
        if y == 2 || y == shell_divider || y == prompt_divider {
            continue;
        }
        print_at(stdout, 0, y, Color::Cyan, "║")?;
        print_at(stdout, width - 1, y, Color::Cyan, "║")?;
    }
    Ok(())
}

fn draw_header(stdout: &mut io::Stdout, state: &AppState, width: u16) -> io::Result<()> {
    let memory_gb = state.used_memory_mb as f64 / 1024.0;
    let total_gb = state.total_memory_mb as f64 / 1024.0;
    let gpu = state
        .gpu
        .temperature_c
        .map(|temp| format!("{:.0} C", temp))
        .unwrap_or_else(|| "n/a".to_owned());
    let header = format!(
        "  RustProc Shell v0.2  │  CPU: {:.0}%  RAM: {:.1}/{:.0} GB  GPU: {}  PIDs: {}",
        state.cpu_usage, memory_gb, total_gb, gpu, state.process_count
    );
    frame_text(stdout, 1, width, &header, Color::Cyan)
}

fn draw_system_panel(stdout: &mut io::Stdout, state: &AppState, width: u16) -> io::Result<()> {
    let x = 3;
    let panel_width = width.saturating_sub(6);
    draw_panel(
        stdout,
        x,
        SYSTEM_TOP,
        panel_width,
        SYSTEM_HEIGHT,
        "System Stats",
    )?;

    let memory_pct = ratio(state.used_memory_mb, state.total_memory_mb);
    let cpu_temp = state
        .temperatures
        .iter()
        .find(|temp| temp.label.to_ascii_lowercase().contains("package"))
        .or_else(|| state.temperatures.first());
    let critical = cpu_temp
        .and_then(|temp| temp.critical_c)
        .map(|temp| format!("{:.0} C", temp))
        .unwrap_or_else(|| "n/a".to_owned());

    panel_text(
        stdout,
        x,
        SYSTEM_TOP + 1,
        panel_width,
        &format!(
            "CPU  {} {:>5.1}%   Temp: {:<8} Freq: {} MHz",
            bar(state.cpu_usage as f64 / 100.0, 13),
            state.cpu_usage,
            format_temp(cpu_temp.map(|temp| temp.temperature_c)),
            state.cpu_frequency_mhz
        ),
        load_color(state.cpu_usage as f64 / 100.0),
    )?;
    panel_text(
        stdout,
        x,
        SYSTEM_TOP + 2,
        panel_width,
        &format!(
            "RAM  {} {:>5.1}%   {:.1} GB / {:.1} GB",
            bar(memory_pct, 13),
            memory_pct * 100.0,
            state.used_memory_mb as f64 / 1024.0,
            state.total_memory_mb as f64 / 1024.0
        ),
        load_color(memory_pct),
    )?;
    panel_text(
        stdout,
        x,
        SYSTEM_TOP + 3,
        panel_width,
        &format!(
            "GPU  {}   VRAM: {:<18} Temp: {}",
            truncate(&state.gpu.vendor_model(), 20),
            format_vram(state.gpu.vram_used_mb, state.gpu.vram_total_mb),
            format_temp(state.gpu.temperature_c)
        ),
        Color::White,
    )?;
    panel_text(
        stdout,
        x,
        SYSTEM_TOP + 4,
        panel_width,
        &format!(
            "CPU  {:<34} Cores: {:<3} Critical: {}",
            truncate(&state.cpu_brand, 34),
            state.cpu_logical_cores,
            critical
        ),
        Color::DarkGrey,
    )?;
    panel_text(
        stdout,
        x,
        SYSTEM_TOP + 5,
        panel_width,
        &format!(
            "CLK  GPU core {:<10} mem {:<10} Cores {}",
            format_clock(state.gpu.core_clock_mhz),
            format_clock(state.gpu.memory_clock_mhz),
            core_summary(state, 28)
        ),
        Color::DarkGrey,
    )
}

fn draw_process_panel(stdout: &mut io::Stdout, state: &AppState, width: u16) -> io::Result<()> {
    let x = 3;
    let panel_width = width.saturating_sub(6);
    draw_panel(
        stdout,
        x,
        PROCESS_TOP,
        panel_width,
        PROCESS_HEIGHT,
        "Processes (top 8 by CPU)",
    )?;
    panel_text(
        stdout,
        x,
        PROCESS_TOP + 1,
        panel_width,
        "PID      Name                 CPU%    Mem MB   Status",
        Color::DarkGrey,
    )?;

    for (index, process) in state.processes.iter().take(5).enumerate() {
        panel_text(
            stdout,
            x,
            PROCESS_TOP + 2 + index as u16,
            panel_width,
            &format!(
                "{:<8} {:<20} {:>5.1}   {:>6}   {}",
                process.pid,
                truncate(&process.name, 20),
                process.cpu,
                process.memory_mb,
                truncate(&process.status, 12)
            ),
            load_color(process.cpu as f64 / 100.0),
        )?;
    }
    Ok(())
}

fn draw_shell_output(
    stdout: &mut io::Stdout,
    output_lines: &[OutputLine],
    width: u16,
    shell_divider: u16,
    prompt_divider: u16,
) -> io::Result<()> {
    frame_text(
        stdout,
        shell_divider + 1,
        width,
        "  Shell Output",
        Color::Cyan,
    )?;
    let output_top = shell_divider + 2;
    let available = prompt_divider.saturating_sub(output_top) as usize;
    let start = output_lines.len().saturating_sub(available);

    for (index, line) in output_lines.iter().skip(start).take(available).enumerate() {
        frame_text(
            stdout,
            output_top + index as u16,
            width,
            &format!("  {}", truncate(&line.text, inner(width).saturating_sub(3))),
            line_color(&line.kind),
        )?;
    }
    Ok(())
}

fn draw_prompt(
    stdout: &mut io::Stdout,
    state: &AppState,
    input: &str,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let cwd = abbreviate_home(&state.cwd);
    let status = if state.last_exit_code == 0 {
        String::new()
    } else {
        format!("[{}] ", state.last_exit_code)
    };
    let prompt = format!("  RustProc {} {}> {}█", cwd, status, input);
    frame_text(stdout, height - 2, width, &prompt, Color::Green)
}

fn draw_panel(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
) -> io::Result<()> {
    let title = format!("── {} ", title);
    let top_fill = width.saturating_sub(title.chars().count() as u16 + 2) as usize;
    print_at(
        stdout,
        x,
        y,
        Color::Cyan,
        &format!("┌{}{}┐", title, "─".repeat(top_fill)),
    )?;
    for row in y + 1..y + height - 1 {
        print_at(
            stdout,
            x,
            row,
            Color::Cyan,
            &format!("│{}│", " ".repeat(width.saturating_sub(2) as usize)),
        )?;
    }
    print_at(
        stdout,
        x,
        y + height - 1,
        Color::Cyan,
        &format!("└{}┘", "─".repeat(width.saturating_sub(2) as usize)),
    )
}

fn panel_text(
    stdout: &mut io::Stdout,
    panel_x: u16,
    y: u16,
    panel_width: u16,
    text: &str,
    color: Color,
) -> io::Result<()> {
    let width = panel_width.saturating_sub(4) as usize;
    print_at(
        stdout,
        panel_x + 3,
        y,
        color,
        &pad(&truncate(text, width), width),
    )
}

fn frame_text(
    stdout: &mut io::Stdout,
    y: u16,
    frame_width: u16,
    text: &str,
    color: Color,
) -> io::Result<()> {
    let width = inner(frame_width);
    print_at(stdout, 1, y, color, &pad(&truncate(text, width), width))
}

fn print_at(stdout: &mut io::Stdout, x: u16, y: u16, color: Color, text: &str) -> io::Result<()> {
    execute!(
        stdout,
        MoveTo(x, y),
        SetForegroundColor(color),
        Print(text),
        ResetColor
    )
}

fn inner(width: u16) -> usize {
    width.saturating_sub(2) as usize
}

fn pad(text: &str, width: usize) -> String {
    format!("{:<width$}", text, width = width)
}

fn ratio(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64).clamp(0.0, 1.0)
    }
}

fn bar(ratio: f64, width: usize) -> String {
    let filled = (ratio.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}

fn core_summary(state: &AppState, max_chars: usize) -> String {
    let summary = state
        .cpu_cores
        .iter()
        .take(4)
        .map(|core| {
            format!(
                "{}:{:.0}%/{}",
                core.name.replace("cpu", "c"),
                core.usage,
                core.frequency_mhz
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    truncate(&summary, max_chars)
}

fn load_color(ratio: f64) -> Color {
    if ratio > 0.80 {
        Color::Red
    } else if ratio > 0.50 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn line_color(kind: &LineKind) -> Color {
    match kind {
        LineKind::Normal => Color::White,
        LineKind::Ai => Color::Cyan,
        LineKind::AiHeader => Color::DarkCyan,
        LineKind::Success => Color::Green,
        LineKind::Warning => Color::Yellow,
        LineKind::Error => Color::Red,
        LineKind::Dim => Color::DarkGrey,
    }
}

fn format_vram(used: Option<u64>, total: Option<u64>) -> String {
    match (used, total) {
        (Some(used), Some(total)) => format!(
            "{:.1} / {:.1} GB",
            used as f64 / 1024.0,
            total as f64 / 1024.0
        ),
        (None, Some(total)) => format!("{:.1} GB", total as f64 / 1024.0),
        _ => "n/a".to_owned(),
    }
}

fn format_clock(clock: Option<u64>) -> String {
    clock
        .map(|clock| format!("{} MHz", clock))
        .unwrap_or_else(|| "n/a".to_owned())
}

fn format_temp(temp: Option<f32>) -> String {
    temp.map(|temp| format!("{:.0} C", temp))
        .unwrap_or_else(|| "n/a".to_owned())
}

fn abbreviate_home(path: &str) -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return path.to_owned();
    };
    let home = home.to_string_lossy();
    if path == home {
        "~".to_owned()
    } else if let Some(rest) = path.strip_prefix(&format!("{}/", home)) {
        format!("~/{}", rest)
    } else {
        path.to_owned()
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut chars = value.chars();
    let mut truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        if max_chars > 1 {
            truncated.pop();
            truncated.push('~');
        }
        truncated
    } else {
        truncated
    }
}

trait GpuName {
    fn vendor_model(&self) -> String;
}

impl GpuName for crate::app::GpuInfo {
    fn vendor_model(&self) -> String {
        if self.model == "Not detected" {
            self.model.clone()
        } else {
            format!("{} {}", self.vendor, self.model)
        }
    }
}
