# RustProc Shell

RustProc Shell is an interactive terminal shell with a live system monitor built
into the same screen. The top pane shows CPU, memory, GPU, temperature, and
process telemetry while the bottom prompt runs normal shell commands.

## Run

```bash
cargo run
```

## Release Build

```bash
cargo build --release
./target/release/rustproc
```

## Built-Ins

| Command | Action |
| --- | --- |
| `help` | Show built-in commands |
| `cd <dir>` | Change the shell working directory |
| `clear` | Clear the shell output pane |
| `sysinfo` | Print a compact telemetry snapshot |
| `exit` / `quit` | Exit RustProc Shell |

Any other input is executed through the system shell from the displayed current
working directory.

## Keys

| Key | Action |
| --- | --- |
| `Up` / `Down` | Navigate command history |
| `Ctrl+C`, `Ctrl+D`, `Esc` | Exit |

## Checks

```bash
cargo fmt --check
cargo build --release
```
# RustProf
