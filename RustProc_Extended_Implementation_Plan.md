# RustProc Extended — Full System Telemetry Monitor
## Comprehensive Implementation Plan

---

## 1. Project Vision

RustProc Extended is a full-spectrum terminal system monitor. Where the base project covered CPU usage and a process list, this plan adds every hardware and OS-level metric a developer or power user would care about: per-core CPU utilisation and clock speeds, GPU load and VRAM, RAM pressure with swap, disk throughput and latency, network interface RX/TX rates, and hardware temperatures across CPU, GPU, and NVMe.

The design philosophy is unchanged: single binary, zero runtime dependencies, terminal-native, exits cleanly. What changes is the depth of instrumentation and the visual complexity of the dashboard.

---

## 2. Hardware Metrics Scope

| Category | Metrics Collected |
|---|---|
| **CPU** | Global usage %, per-core usage %, per-core clock speed (MHz), base/boost frequency, logical/physical core count, CPU model name, architecture |
| **GPU** | GPU name, load %, VRAM used/total (MB), GPU clock (MHz), memory clock (MHz), power draw (W), fan speed (RPM) |
| **RAM** | Total, used, free, available (MB), swap total/used/free, usage % |
| **Disk** | Per-device read MB/s, write MB/s, total read/write since boot, mount points, filesystem type, capacity used % |
| **Network** | Per-interface RX bytes/s, TX bytes/s, total RX/TX since boot, interface name, link state |
| **Temperatures** | CPU package temp (°C), per-core temps, GPU temp (°C), NVMe/SSD temps, motherboard temp (if exposed) |
| **Clock Speeds** | Per-core current frequency, GPU core clock, GPU memory clock |

---

## 3. Crate Selection and Justification

### 3.1 Core Crates (Mandatory)

| Crate | Version | Provides | Notes |
|---|---|---|---|
| `sysinfo` | 0.30 | CPU, RAM, Disk, Network, Processes, some temps | Cross-platform, solid baseline |
| `ratatui` | 0.26 | Terminal UI, all widget types | Actively maintained fork of tui-rs |
| `crossterm` | 0.27 | Terminal backend, input, raw mode | Cross-platform terminal control |

### 3.2 Extended Crates (Per Feature)

| Crate | Version | Provides | Platform |
|---|---|---|---|
| `nvml-wrapper` | 0.9 | Full NVIDIA GPU telemetry via NVML | Linux, Windows |
| `wgpu` | latest | GPU detection fallback (non-NVIDIA) | Cross-platform |
| `sysfs-class` | 0.2 | Linux sysfs hardware sensors (temps, fans) | Linux only |
| `windows` | 0.52 | WMI queries for GPU/temp on Windows | Windows only |
| `serde` + `toml` | latest | Config file parsing | All platforms |
| `chrono` | 0.4 | Timestamp formatting for log output | All platforms |

### 3.3 Platform Coverage Matrix

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| CPU stats | sysinfo | sysinfo | sysinfo |
| RAM stats | sysinfo | sysinfo | sysinfo |
| Disk stats | sysinfo | sysinfo | sysinfo |
| Network stats | sysinfo | sysinfo | sysinfo |
| CPU temps | sysfs-class / sysinfo | sysinfo (limited) | windows crate / WMI |
| GPU (NVIDIA) | nvml-wrapper | nvml-wrapper | nvml-wrapper |
| GPU (AMD) | sysfs-class | — | windows crate |
| GPU (Intel iGPU) | sysfs-class | — | windows crate |
| Per-core clocks | sysfs (`/sys/devices/system/cpu`) | sysctl | windows crate |

> **Decision rule:** Implement Linux first and completely. Use `#[cfg(target_os = "linux")]` guards to gate platform-specific code. On unsupported platforms, gracefully render "N/A" rather than panicking.

---

## 4. Repository Structure

```
rustproc-extended/
├── Cargo.toml
├── Cargo.lock
├── config.toml.example       # default user config with comments
├── README.md
└── src/
    ├── main.rs               # terminal lifecycle, event loop
    ├── app.rs                # AppState: all metric snapshots
    ├── config.rs             # config file parsing (tick rate, units, thresholds)
    ├── ui/
    │   ├── mod.rs            # top-level render() dispatcher
    │   ├── layout.rs         # chunk/panel layout definitions
    │   ├── cpu.rs            # CPU panel widgets
    │   ├── gpu.rs            # GPU panel widgets
    │   ├── memory.rs         # RAM + swap panel widgets
    │   ├── disk.rs           # disk I/O panel widgets
    │   ├── network.rs        # network panel widgets
    │   ├── temps.rs          # temperature + fan panel widgets
    │   └── processes.rs      # process table widget
    └── collectors/
        ├── mod.rs            # CollectorSet: owns all sub-collectors
        ├── cpu.rs            # CPU + clock data via sysinfo + sysfs
        ├── gpu.rs            # GPU data via nvml-wrapper or sysfs
        ├── memory.rs         # RAM + swap via sysinfo
        ├── disk.rs           # disk I/O delta calculation
        ├── network.rs        # network RX/TX delta calculation
        └── temps.rs          # temperature sensor discovery + polling
```

Each collector owns its own `previous_snapshot` for computing deltas (bytes/s requires two samples). Each UI module is given only the slice of `AppState` it needs — no module reaches into another's data.

---

## 5. AppState Design

```rust
// src/app.rs

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub cpu:      CpuState,
    pub gpu:      GpuState,
    pub memory:   MemoryState,
    pub disks:    Vec<DiskState>,
    pub networks: Vec<NetworkState>,
    pub temps:    TempState,
    pub processes: Vec<ProcessRow>,
    pub scroll_offset: usize,
    pub active_tab: Tab,
    pub tick_count: u64,
}

#[derive(Debug, Clone, Default)]
pub enum Tab { #[default] Overview, Cpu, Gpu, Memory, Disk, Network, Temps, Processes }

// ─── CPU ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct CpuState {
    pub model_name:     String,
    pub physical_cores: usize,
    pub logical_cores:  usize,
    pub global_usage:   f32,                  // %
    pub per_core_usage: Vec<f32>,             // % per logical core
    pub per_core_freq:  Vec<u64>,             // MHz per logical core
    pub base_freq_mhz:  u64,
    pub boost_freq_mhz: Option<u64>,
    pub history:        Vec<f32>,             // last 60 ticks for sparkline
}

// ─── GPU ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct GpuState {
    pub available:       bool,
    pub name:            String,
    pub load_pct:        f32,
    pub vram_used_mb:    u64,
    pub vram_total_mb:   u64,
    pub core_clock_mhz:  u32,
    pub mem_clock_mhz:   u32,
    pub power_draw_w:    f32,
    pub fan_speed_rpm:   Option<u32>,
    pub temp_celsius:    Option<f32>,
    pub load_history:    Vec<f32>,            // last 60 ticks for sparkline
}

// ─── MEMORY ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct MemoryState {
    pub total_mb:     u64,
    pub used_mb:      u64,
    pub free_mb:      u64,
    pub available_mb: u64,
    pub swap_total_mb: u64,
    pub swap_used_mb:  u64,
    pub usage_pct:    f32,
    pub swap_pct:     f32,
    pub history:      Vec<f32>,
}

// ─── DISK ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct DiskState {
    pub name:          String,              // e.g. "sda", "nvme0n1"
    pub mount:         String,
    pub fs_type:       String,
    pub total_gb:      f64,
    pub used_gb:       f64,
    pub used_pct:      f32,
    pub read_bps:      u64,                // bytes/sec (delta)
    pub write_bps:     u64,                // bytes/sec (delta)
}

// ─── NETWORK ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct NetworkState {
    pub interface:    String,
    pub rx_bps:       u64,                 // bytes/sec
    pub tx_bps:       u64,
    pub total_rx_mb:  u64,
    pub total_tx_mb:  u64,
    pub is_up:        bool,
}

// ─── TEMPS ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TempState {
    pub cpu_package_c:  Option<f32>,
    pub per_core_c:     Vec<Option<f32>>,
    pub gpu_c:          Option<f32>,
    pub nvme_c:         Vec<(String, f32)>,  // (device label, temp)
    pub motherboard_c:  Option<f32>,
    pub fan_speeds:     Vec<(String, u32)>,  // (fan label, RPM)
}

// ─── PROCESS ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ProcessRow {
    pub pid:        String,
    pub name:       String,
    pub cpu_pct:    f32,
    pub mem_mb:     u64,
    pub status:     String,
    pub threads:    u32,
}
```

---

## 6. Collector Design

### 6.1 Collector Trait

All collectors implement a common trait so the collector set can update them uniformly:

```rust
// src/collectors/mod.rs

pub trait Collector {
    type Output;
    fn collect(&mut self) -> Self::Output;
}
```

### 6.2 Delta Calculation (Disk and Network)

Bytes/sec requires two consecutive samples. Each collector stores the previous raw value and the timestamp:

```rust
// src/collectors/disk.rs (sketch)

pub struct DiskCollector {
    sys: sysinfo::System,
    prev: HashMap<String, (u64, u64, Instant)>,  // name → (read_bytes, write_bytes, time)
}

impl DiskCollector {
    pub fn collect(&mut self) -> Vec<DiskState> {
        self.sys.refresh_disks_list();
        self.sys.refresh_disks();
        let now = Instant::now();

        self.sys.disks().iter().map(|disk| {
            let name = disk.name().to_string_lossy().to_string();
            let read  = disk.total_read_bytes();
            let write = disk.total_written_bytes();

            let (read_bps, write_bps) = if let Some((prev_r, prev_w, prev_t)) = self.prev.get(&name) {
                let dt = now.duration_since(*prev_t).as_secs_f64().max(0.001);
                (
                    ((read  - prev_r) as f64 / dt) as u64,
                    ((write - prev_w) as f64 / dt) as u64,
                )
            } else {
                (0, 0)
            };

            self.prev.insert(name.clone(), (read, write, now));

            DiskState {
                name,
                mount:     disk.mount_point().to_string_lossy().to_string(),
                fs_type:   disk.file_system().to_string_lossy().to_string(),
                total_gb:  disk.total_space() as f64 / 1e9,
                used_gb:   (disk.total_space() - disk.available_space()) as f64 / 1e9,
                used_pct:  100.0 * (1.0 - disk.available_space() as f32 / disk.total_space() as f32),
                read_bps,
                write_bps,
            }
        }).collect()
    }
}
```

The same delta pattern applies identically to the network collector.

### 6.3 GPU Collector (NVIDIA via NVML)

```rust
// src/collectors/gpu.rs

#[cfg(feature = "nvidia")]
use nvml_wrapper::Nvml;

pub struct GpuCollector {
    #[cfg(feature = "nvidia")]
    nvml: Option<Nvml>,
    load_history: Vec<f32>,
}

impl GpuCollector {
    pub fn new() -> Self {
        #[cfg(feature = "nvidia")]
        let nvml = Nvml::init().ok();
        Self {
            #[cfg(feature = "nvidia")]
            nvml,
            load_history: Vec::with_capacity(60),
        }
    }

    pub fn collect(&mut self) -> GpuState {
        #[cfg(feature = "nvidia")]
        if let Some(nvml) = &self.nvml {
            if let Ok(device) = nvml.device_by_index(0) {
                let load        = device.utilization_rates().map(|u| u.gpu as f32).unwrap_or(0.0);
                let mem_info    = device.memory_info().unwrap_or_default();
                let core_clock  = device.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Graphics).unwrap_or(0);
                let mem_clock   = device.clock_info(nvml_wrapper::enum_wrappers::device::Clock::Memory).unwrap_or(0);
                let power       = device.power_usage().map(|p| p as f32 / 1000.0).unwrap_or(0.0); // mW → W
                let temp        = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).map(|t| t as f32).ok();
                let fan         = device.fan_speed(0).ok();
                let name        = device.name().unwrap_or_default();

                self.load_history.push(load);
                if self.load_history.len() > 60 { self.load_history.remove(0); }

                return GpuState {
                    available: true,
                    name,
                    load_pct: load,
                    vram_used_mb:  mem_info.used  / 1024 / 1024,
                    vram_total_mb: mem_info.total / 1024 / 1024,
                    core_clock_mhz: core_clock,
                    mem_clock_mhz:  mem_clock,
                    power_draw_w:   power,
                    fan_speed_rpm:  fan,
                    temp_celsius:   temp,
                    load_history:   self.load_history.clone(),
                };
            }
        }

        // Fallback: GPU not available or not NVIDIA
        GpuState { available: false, ..Default::default() }
    }
}
```

### 6.4 Temperature Collector (Linux sysfs)

On Linux, hardware temperatures are exposed under `/sys/class/hwmon/`. Each `hwmon*` directory contains `name` (chip label) and `temp*_input` files (value in millidegrees Celsius).

```rust
// src/collectors/temps.rs (Linux path)

#[cfg(target_os = "linux")]
pub fn read_hwmon_temps() -> Vec<(String, f32)> {
    let mut results = Vec::new();
    let base = std::path::Path::new("/sys/class/hwmon");

    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            let chip_name = std::fs::read_to_string(path.join("name"))
                .unwrap_or_default()
                .trim()
                .to_string();

            let mut i = 1u32;
            loop {
                let temp_path  = path.join(format!("temp{}_input", i));
                let label_path = path.join(format!("temp{}_label", i));
                if !temp_path.exists() { break; }

                let raw   = std::fs::read_to_string(&temp_path).unwrap_or_default();
                let label = std::fs::read_to_string(&label_path).unwrap_or(format!("temp{}", i));
                if let Ok(millideg) = raw.trim().parse::<f32>() {
                    results.push((format!("{}/{}", chip_name, label.trim()), millideg / 1000.0));
                }
                i += 1;
            }
        }
    }
    results
}
```

This approach does not require any extra crate on Linux — it reads the kernel's own sensor interface directly. The `sysfs-class` crate wraps the same interface with a more ergonomic API if preferred.

---

## 7. UI Design

### 7.1 Tab Layout

The dashboard is too information-dense for a single screen. Tabs let the user focus on one subsystem:

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  [1] Overview  [2] CPU  [3] GPU  [4] Memory  [5] Disk  [6] Net  [7] Temps  [8] Procs  │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│   (tab-specific content)                                                     │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
│  [1-8] Switch Tab   [↑↓] Scroll   [q] Quit   [r] Force Refresh              │
└──────────────────────────────────────────────────────────────────────────────┘
```

Number keys `1`–`8` switch tabs instantly. The tab bar highlights the active tab with a bold + coloured style.

### 7.2 Overview Tab

```
┌─ CPU ──────────────────────────┐ ┌─ GPU ──────────────────────────────────┐
│ Intel Core i7-12700K           │ │ NVIDIA GeForce RTX 4080                │
│ Global:  ███████░░░ 68%        │ │ Load:    ██████████ 94%   Temp: 71°C   │
│ Temp:    74°C                  │ │ VRAM:    ████░░░░░░ 6.1 / 16.0 GB      │
│ Freq:    4.2 GHz (boost)       │ │ Core:    2520 MHz   Mem: 10501 MHz     │
│                                │ │ Power:   280 W                         │
└────────────────────────────────┘ └────────────────────────────────────────┘
┌─ Memory ───────────────────────┐ ┌─ Top Processes ────────────────────────┐
│ RAM:   ██████░░░░ 11.2 / 32 GB │ │ PID    Name           CPU%   Mem MB   │
│ Swap:  █░░░░░░░░░  0.9 / 8 GB  │ │ 4821   chrome          8.2    1024    │
│                                │ │ 3301   code            4.1     512    │
└────────────────────────────────┘ │ 1482   python          2.9     128    │
┌─ Disk I/O ─────────────────────┐ └────────────────────────────────────────┘
│ nvme0n1  R: 312 MB/s W: 88 MB/s│
│ sda      R:   0 MB/s W:  0 MB/s│
└────────────────────────────────┘
```

### 7.3 CPU Tab

```
┌─ CPU Detail ─────────────────────────────────────────────────────────────────┐
│ Model: Intel Core i7-12700K  │  12 Physical / 20 Logical  │  Base: 3.6 GHz  │
├──────────────────────────────────────────────────────────────────────────────┤
│ Core  0 [ 82% ████████░░ ] 4200 MHz     Core  1 [ 45% ████░░░░░░ ] 3900 MHz │
│ Core  2 [ 71% ███████░░░ ] 4100 MHz     Core  3 [ 30% ███░░░░░░░ ] 3700 MHz │
│ Core  4 [ 55% █████░░░░░ ] 4000 MHz     Core  5 [ 22% ██░░░░░░░░ ] 3600 MHz │
│ ...                                                                          │
├──────────────────────────────────────────────────────────────────────────────┤
│ Global CPU History (last 60s)                                                │
│  100 ┤                                                                       │
│   75 ┤      ╭──╮     ╭─╮                                                     │
│   50 ┤ ─────╯  ╰─────╯ ╰─────────────────────────────                        │
│   25 ┤                                                                       │
│    0 ┤────────────────────────────────────────────────────────               │
└──────────────────────────────────────────────────────────────────────────────┘
```

Each core shows a mini progress bar followed by current frequency. The history chart uses ratatui's `LineChart` (or `Sparkline` for compact mode).

### 7.4 GPU Tab

```
┌─ GPU: NVIDIA GeForce RTX 4080 ───────────────────────────────────────────────┐
│                                                                              │
│  Load:        ██████████░ 94%            Temp:    71°C  (limit: 83°C)        │
│  VRAM:        █████████░░ 6.1 / 16.0 GB                                      │
│  Core Clock:  2520 MHz                   Mem Clock:  10501 MHz               │
│  Power Draw:  280 W                      Fan Speed:  1840 RPM                │
│                                                                              │
├─ GPU Load History ───────────────────────────────────────────────────────────┤
│  100 ┤                    ╭─────────────────────────────────                 │
│   75 ┤          ╭─────────╯                                                  │
│   50 ┤ ─────────╯                                                            │
│    0 ┤──────────────────────────────────────────────────                     │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 7.5 Memory Tab

```
┌─ RAM ────────────────────────────────────────────────────────────────────────┐
│  Total:     32768 MB                                                         │
│  Used:      11468 MB  ████████████████████░░░░░░░░░░░░░░░░  35%             │
│  Available: 21300 MB                                                         │
│  Free:       8012 MB                                                         │
├─ Swap ───────────────────────────────────────────────────────────────────────┤
│  Total:      8192 MB                                                         │
│  Used:        921 MB  ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  11%             │
│  Free:        7271 MB                                                         │
├─ RAM Usage History ──────────────────────────────────────────────────────────┤
│  100 ┤                                                                       │
│   50 ┤──────────────────────────────────────────────                         │
│    0 ┤────────────────────────────────────────────────────                   │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 7.6 Disk Tab

```
┌─ Disk I/O ───────────────────────────────────────────────────────────────────┐
│  Device      Mount   FS     Total    Used    Used%    Read/s    Write/s      │
│  nvme0n1p1   /       ext4   512 GB   210 GB   41%    312 MB/s   88 MB/s     │
│  sda1        /home   ext4  2048 GB   810 GB   40%      0 MB/s    0 MB/s     │
│  sdb1        /data   ntfs  4096 GB  3200 GB   78%      0 MB/s    1 MB/s     │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 7.7 Network Tab

```
┌─ Network Interfaces ─────────────────────────────────────────────────────────┐
│  Interface   State   RX/s       TX/s       Total RX    Total TX              │
│  enp6s0      UP      12.4 MB/s   2.1 MB/s   14.2 GB    3.8 GB              │
│  lo          UP       0.0 MB/s   0.0 MB/s    0.1 GB    0.1 GB              │
│  wlo1        DOWN     —          —           —          —                   │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 7.8 Temperatures Tab

```
┌─ CPU Temps ──────────────────────┐ ┌─ GPU Temp ────────────────────────────┐
│  Package:   74°C                 │ │  GPU Core:   71°C                      │
│  Core 0:    72°C                 │ │  GPU Mem:    65°C  (if exposed)        │
│  Core 1:    71°C                 │ └────────────────────────────────────────┘
│  Core 2:    75°C                 │ ┌─ Storage ─────────────────────────────┐
│  ...                             │ │  nvme0n1:   42°C                       │
└──────────────────────────────────┘ │  sda:       31°C                       │
┌─ Fans ───────────────────────────┐ └────────────────────────────────────────┘
│  CPU Fan:    1200 RPM            │
│  Case Fan1:   900 RPM            │
│  GPU Fan:    1840 RPM            │
└──────────────────────────────────┘
```

---

## 8. Colour Coding Reference

Consistent thresholds across all panels:

| Metric | Green | Yellow | Red |
|---|---|---|---|
| CPU / GPU usage % | < 50% | 50–80% | > 80% |
| RAM / VRAM usage % | < 60% | 60–85% | > 85% |
| Disk usage % | < 70% | 70–90% | > 90% |
| CPU temp | < 60°C | 60–80°C | > 80°C |
| GPU temp | < 65°C | 65–83°C | > 83°C |
| NVMe temp | < 45°C | 45–60°C | > 60°C |

These thresholds should be overridable in `config.toml`.

---

## 9. Config File (`config.toml`)

```toml
# RustProc Extended Configuration

[display]
tick_rate_ms = 1000          # refresh interval in milliseconds
default_tab = "overview"     # overview | cpu | gpu | memory | disk | network | temps | procs
temperature_unit = "celsius" # celsius | fahrenheit
process_limit = 20           # number of processes shown in table

[thresholds.cpu]
warn_pct  = 50
crit_pct  = 80
warn_temp = 60
crit_temp = 80

[thresholds.gpu]
warn_pct  = 50
crit_pct  = 80
warn_temp = 65
crit_temp = 83

[thresholds.memory]
warn_pct = 60
crit_pct = 85

[thresholds.disk]
warn_pct = 70
crit_pct = 90

[network]
exclude_interfaces = ["lo"]  # interfaces to hide from the network tab

[history]
depth = 60                   # number of ticks kept for sparkline/chart history
```

---

## 10. Implementation Phases

### Phase 1 — Project Scaffold (45 minutes)

Goal: all modules compile, tabs switch, no real data yet.

- Create `rustproc-extended` with the full directory structure from Section 4
- Stub all collector `collect()` methods to return `Default::default()`
- Implement `AppState` with all sub-structs
- Implement tab switching in the event loop (`1`–`8` keys)
- Implement the tab bar widget in `ui/layout.rs`
- Render placeholder text in each tab

Success criterion: number keys 1–8 switch between tabs showing named placeholder panels.

---

### Phase 2 — CPU Panel (1 hour)

Goal: per-core usage, per-core frequency, global history chart.

- Implement `collectors/cpu.rs` using `sysinfo` for usage
- Read per-core frequencies from `/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq` on Linux
- Populate `CpuState.history` ring buffer (push on each tick, truncate to `config.history.depth`)
- Implement `ui/cpu.rs`: per-core mini-gauges, global sparkline

Success criterion: each core shows a distinct usage % that changes under load; history chart scrolls.

---

### Phase 3 — Memory Panel (30 minutes)

Goal: RAM and swap gauges with history.

- Implement `collectors/memory.rs` using `sysinfo`
- Populate `MemoryState.history`
- Implement `ui/memory.rs`: two Gauge widgets, history chart

Success criterion: RAM bar matches `free -h` output; swap bar appears when swap is active.

---

### Phase 4 — Disk Panel (1 hour)

Goal: per-device capacity and live I/O throughput.

- Implement `collectors/disk.rs` with delta calculation (two-sample bytes/sec as described in Section 6.2)
- Test with `dd if=/dev/zero of=/tmp/test bs=1M count=512` — write rate should spike
- Implement `ui/disk.rs`: table widget with colour-coded capacity column

Success criterion: running `dd` shows a clear write spike on the target device row.

---

### Phase 5 — Network Panel (45 minutes)

Goal: per-interface RX/TX bytes/sec.

- Implement `collectors/network.rs` with delta calculation
- Filter out loopback and excluded interfaces from config
- Implement `ui/network.rs`: table widget

Success criterion: downloading a large file shows matching RX rate on the active interface.

---

### Phase 6 — Temperature Panel (1.5 hours)

Goal: CPU package, per-core, GPU, and NVMe temps.

- Implement `collectors/temps.rs` using the `/sys/class/hwmon/` reader described in Section 6.4
- Map `hwmon` chips to known names: `k10temp` (AMD CPU), `coretemp` (Intel CPU), `nvme` (NVMe), `nouveau`/`amdgpu` (GPU if not using NVML)
- Implement `ui/temps.rs`: temp + fan table with colour coding

**This is the most platform-specific phase.** Budget extra time if targeting macOS or Windows — those paths use different APIs (`sysctl` and WMI respectively).

Success criterion: CPU package temp matches the value shown in `sensors` (lm-sensors) output on Linux.

---

### Phase 7 — GPU Panel (1.5 hours)

Goal: NVIDIA GPU load, VRAM, clocks, power, temp via NVML.

- Add `nvml-wrapper` behind a Cargo feature flag: `features = ["nvidia"]`
- Implement `collectors/gpu.rs` as described in Section 6.3
- Implement sysfs AMD fallback under `#[cfg(target_os = "linux")]`
- Implement `ui/gpu.rs`: load gauge, VRAM gauge, clock/power table, history chart

Success criterion: GPU load gauge rises during a GPU-heavy workload (e.g., running a CUDA sample or a game).

---

### Phase 8 — Overview Tab (30 minutes)

Goal: compose summary widgets from all panels.

- Implement `ui/overview.rs` by reusing the smaller gauge/stat widgets from each panel module
- 2×2 grid: CPU, GPU, Memory, Disk I/O summary + top-3 processes

Success criterion: overview shows consistent numbers with the individual tabs.

---

### Phase 9 — Config File Integration (30 minutes)

Goal: tick rate, thresholds, and excluded interfaces read from `config.toml`.

- Implement `src/config.rs` using `serde` + `toml`
- Load from `~/.config/rustproc/config.toml`, fall back to compiled-in defaults
- Pass `Config` into all collectors and UI modules

Success criterion: changing `tick_rate_ms = 500` in config doubles the refresh rate.

---

### Phase 10 — Hardening and Polish (1 hour)

Goal: no crashes under any normal system condition.

- All `sysinfo` and `sysfs` reads wrapped defensively — `unwrap_or_default()` or explicit `match` with fallback text
- Fahrenheit conversion implemented and gated by config
- `cargo clippy -- -D warnings` exits 0
- `cargo fmt --check` exits 0
- Test on a machine with no GPU — all GPU fields show "N/A", no panic
- Test with `RUST_LOG=debug cargo run` to confirm no spurious log noise

---

## 11. Cargo.toml (Final)

```toml
[package]
name    = "rustproc-extended"
version = "0.2.0"
edition = "2021"

[features]
default = ["nvidia"]
nvidia  = ["nvml-wrapper"]

[dependencies]
sysinfo      = "0.30"
ratatui      = "0.26"
crossterm    = "0.27"
serde        = { version = "1", features = ["derive"] }
toml         = "0.8"
chrono       = "0.4"

# Optional — NVIDIA GPU telemetry
nvml-wrapper = { version = "0.9", optional = true }

[target.'cfg(target_os = "linux")'.dependencies]
# No extra crate needed — we read sysfs directly

[profile.release]
opt-level = 3
strip     = true
lto       = true
codegen-units = 1   # maximises inlining; slows compile but produces fastest binary
```

---

## 12. Build and Run Reference

```bash
# Standard build (with NVIDIA support if NVML is installed)
cargo run

# Build without NVIDIA (for systems without CUDA toolkit)
cargo run --no-default-features

# Release binary
cargo build --release
./target/release/rustproc-extended

# Lint
cargo clippy --all-features -- -D warnings

# Format
cargo fmt --check
```

### Keybindings

| Key | Action |
|---|---|
| `1` – `8` | Switch tabs |
| `↑` / `↓` | Scroll process table / disk list |
| `r` | Force immediate data refresh |
| `f` | Toggle °C / °F |
| `q` | Quit cleanly |

---

## 13. Known Platform Limitations

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| Per-core clock speed | Full (sysfs) | Partial (sysctl) | Partial (WMI) |
| CPU temps | Full (hwmon) | Limited (SMC) | Partial (WMI) |
| NVMe temps | Full (hwmon) | Not available | Partial (WMI) |
| AMD GPU | sysfs (amdgpu) | Not implemented | Not implemented |
| Intel iGPU | sysfs (i915) | Not implemented | Not implemented |
| Fan speeds | hwmon (if exposed) | SMC | WMI |

For macOS, the `sysinfo` crate covers the fundamentals, but hardware sensor access requires either the `powermetrics` binary (root only) or the `SMCKit` approach via FFI. These are explicitly out of scope for a demo build — render "N/A (macOS)" and move on.

---

## 14. Full Implementation Checklist

### Data Collection
- [ ] CPU global usage, per-core usage, per-core frequency
- [ ] CPU package temp, per-core temps (Linux hwmon)
- [ ] GPU load, VRAM, core clock, mem clock, power, fan, temp (NVML)
- [ ] GPU fallback renders "N/A" cleanly when NVML unavailable
- [ ] RAM total/used/free/available
- [ ] Swap total/used/free
- [ ] Disk per-device capacity, read bytes/sec, write bytes/sec
- [ ] Network per-interface RX bytes/sec, TX bytes/sec
- [ ] All delta collectors handle first-tick (prev = None) without panicking

### UI
- [ ] Tab bar with number-key switching
- [ ] Overview tab with all subsystem summaries
- [ ] CPU tab: per-core gauges + frequency + history chart
- [ ] GPU tab: load/VRAM gauges + clocks + history chart
- [ ] Memory tab: RAM/swap gauges + history chart
- [ ] Disk tab: capacity bar + I/O throughput table
- [ ] Network tab: per-interface RX/TX table
- [ ] Temps tab: CPU/GPU/NVMe temps + fan speeds
- [ ] Process tab: sorted, scrollable, with CPU/mem columns
- [ ] Colour thresholds applied consistently across all panels

### Config
- [ ] `config.toml` loaded from `~/.config/rustproc/config.toml`
- [ ] Sensible compiled-in defaults used when file is absent
- [ ] Tick rate respected
- [ ] Temperature unit toggle works (`f` key + config option)

### Hardening
- [ ] No `unwrap()` in collector code — all fallible paths handled
- [ ] Terminal teardown called on both normal exit and error paths
- [ ] Tested on machine with no GPU (no crash, all GPU fields N/A)
- [ ] `cargo clippy -- -D warnings` exits 0
- [ ] `cargo fmt --check` exits 0
- [ ] `cargo build --release` exits 0
