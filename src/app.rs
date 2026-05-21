use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub cpu_usage: f32,
    pub cpu_brand: String,
    pub cpu_vendor: String,
    pub cpu_frequency_mhz: u64,
    pub cpu_logical_cores: usize,
    pub cpu_cores: Vec<CpuCoreRow>,
    pub temperatures: Vec<TemperatureRow>,
    pub gpu: GpuInfo,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub process_count: usize,
    pub processes: Vec<ProcessRow>,
    pub cwd: String,
    pub last_exit_code: i32,
}

pub type SharedState = Arc<Mutex<AppState>>;

#[derive(Debug, Clone, Default)]
pub struct CpuCoreRow {
    pub name: String,
    pub usage: f32,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TemperatureRow {
    pub label: String,
    pub temperature_c: f32,
    pub critical_c: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub model: String,
    pub vendor: String,
    pub vram_total_mb: Option<u64>,
    pub vram_used_mb: Option<u64>,
    pub core_clock_mhz: Option<u64>,
    pub memory_clock_mhz: Option<u64>,
    pub temperature_c: Option<f32>,
    pub source: String,
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            model: "Not detected".to_owned(),
            vendor: "Unknown".to_owned(),
            vram_total_mb: None,
            vram_used_mb: None,
            core_clock_mhz: None,
            memory_clock_mhz: None,
            temperature_c: None,
            source: "sysinfo/sysfs".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessRow {
    pub pid: String,
    pub name: String,
    pub cpu: f32,
    pub memory_mb: u64,
    pub status: String,
}
