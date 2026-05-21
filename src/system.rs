use std::cmp::Ordering;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;

use sysinfo::{Components, System};

use crate::app::{AppState, CpuCoreRow, GpuInfo, ProcessRow, TemperatureRow};

pub fn collect(sys: &mut System, components: &mut Components, cwd: String) -> AppState {
    sys.refresh_all();
    components.refresh_list();
    components.refresh();

    let mut processes = sys
        .processes()
        .iter()
        .map(|(pid, process)| {
            let name = process.name();
            let name = if name.trim().is_empty() {
                "[unknown]".to_owned()
            } else {
                name.to_owned()
            };

            ProcessRow {
                pid: pid.to_string(),
                name,
                cpu: process.cpu_usage(),
                memory_mb: bytes_to_mb(process.memory()),
                status: format!("{:?}", process.status()),
            }
        })
        .collect::<Vec<_>>();

    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(Ordering::Equal));

    let cpu_cores = sys
        .cpus()
        .iter()
        .map(|cpu| CpuCoreRow {
            name: cpu.name().to_owned(),
            usage: cpu.cpu_usage(),
            frequency_mhz: cpu.frequency(),
        })
        .collect::<Vec<_>>();
    let cpu_brand = sys
        .cpus()
        .iter()
        .find_map(|cpu| non_empty(cpu.brand()))
        .or_else(|| non_empty(sys.global_cpu_info().brand()))
        .unwrap_or_else(|| "Unknown CPU".to_owned());
    let cpu_vendor = sys
        .cpus()
        .iter()
        .find_map(|cpu| non_empty(cpu.vendor_id()))
        .or_else(|| non_empty(sys.global_cpu_info().vendor_id()))
        .unwrap_or_else(|| "Unknown".to_owned());
    let cpu_frequency_mhz = sys
        .cpus()
        .iter()
        .map(|cpu| cpu.frequency())
        .max()
        .unwrap_or_else(|| sys.global_cpu_info().frequency());
    let process_count = processes.len();

    AppState {
        cpu_usage: sys.global_cpu_info().cpu_usage(),
        cpu_brand,
        cpu_vendor,
        cpu_frequency_mhz,
        cpu_logical_cores: sys.cpus().len(),
        cpu_cores,
        gpu: collect_gpu_info(components),
        temperatures: collect_temperatures(components),
        total_memory_mb: bytes_to_mb(sys.total_memory()),
        used_memory_mb: bytes_to_mb(sys.used_memory()),
        process_count,
        processes,
        cwd,
        last_exit_code: 0,
    }
}

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / 1024 / 1024
}

fn collect_temperatures(components: &Components) -> Vec<TemperatureRow> {
    let mut rows = components
        .iter()
        .filter(|component| component.temperature().is_finite())
        .map(|component| TemperatureRow {
            label: component.label().to_owned(),
            temperature_c: component.temperature(),
            critical_c: component.critical(),
        })
        .collect::<Vec<_>>();

    rows.sort_by(|a, b| {
        b.temperature_c
            .partial_cmp(&a.temperature_c)
            .unwrap_or(Ordering::Equal)
    });
    rows
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

#[cfg(target_os = "linux")]
fn collect_gpu_info(components: &Components) -> GpuInfo {
    use std::fs;

    let mut gpu = GpuInfo {
        temperature_c: gpu_temperature_from_components(components),
        ..GpuInfo::default()
    };

    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return gpu;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }

        let device_path = entry.path().join("device");
        let vendor_id = read_trimmed(device_path.join("vendor"));
        let device_id = read_trimmed(device_path.join("device"));
        if vendor_id.is_none() && device_id.is_none() {
            continue;
        }

        let vendor_id = vendor_id.unwrap_or_default();
        let device_id = device_id.unwrap_or_default();
        gpu.vendor = gpu_vendor_name(&vendor_id).to_owned();
        gpu.model = gpu_model(&device_path, &gpu.vendor, &device_id);
        gpu.vram_total_mb = read_u64(device_path.join("mem_info_vram_total")).map(bytes_to_mb);
        gpu.vram_used_mb = read_u64(device_path.join("mem_info_vram_used")).map(bytes_to_mb);
        gpu.core_clock_mhz = read_active_dpm_clock(device_path.join("pp_dpm_sclk"))
            .or_else(|| read_u64(device_path.join("gt_cur_freq_mhz")))
            .or_else(|| read_u64(device_path.join("gt_act_freq_mhz")));
        gpu.memory_clock_mhz = read_active_dpm_clock(device_path.join("pp_dpm_mclk"));
        gpu.temperature_c = gpu
            .temperature_c
            .or_else(|| read_hwmon_temperature(&device_path));
        gpu.source = format!("linux sysfs {}", name);

        if gpu.vendor == "NVIDIA" {
            if let Some(nvidia_gpu) = collect_nvidia_smi_gpu() {
                gpu = merge_gpu_info(gpu, nvidia_gpu);
            } else {
                gpu.source = format!("{}; nvidia-smi unavailable", gpu.source);
            }
        }
        return gpu;
    }

    gpu
}

#[cfg(not(target_os = "linux"))]
fn collect_gpu_info(components: &Components) -> GpuInfo {
    GpuInfo {
        temperature_c: gpu_temperature_from_components(components),
        ..GpuInfo::default()
    }
}

fn gpu_temperature_from_components(components: &Components) -> Option<f32> {
    components
        .iter()
        .find(|component| {
            let label = component.label().to_ascii_lowercase();
            label.contains("gpu") || label.contains("amdgpu") || label.contains("nvidia")
        })
        .map(|component| component.temperature())
        .filter(|temperature| temperature.is_finite())
}

#[cfg(target_os = "linux")]
fn gpu_model(device_path: &Path, vendor: &str, device_id: &str) -> String {
    read_trimmed(device_path.join("product_name"))
        .or_else(|| read_trimmed(device_path.join("model")))
        .or_else(|| {
            read_trimmed(device_path.join("uevent"))
                .and_then(|data| parse_uevent_slot(&data))
                .and_then(|slot| lspci_model(&slot))
        })
        .or_else(|| {
            read_trimmed(device_path.join("uevent")).and_then(|data| parse_uevent_name(&data))
        })
        .unwrap_or_else(|| {
            if device_id.is_empty() {
                vendor.to_owned()
            } else {
                format!("{} GPU {}", vendor, device_id)
            }
        })
}

#[cfg(target_os = "linux")]
fn parse_uevent_name(data: &str) -> Option<String> {
    data.lines()
        .find_map(|line| line.strip_prefix("PCI_ID="))
        .map(|id| format!("PCI {}", id))
}

#[cfg(target_os = "linux")]
fn parse_uevent_slot(data: &str) -> Option<String> {
    data.lines()
        .find_map(|line| line.strip_prefix("PCI_SLOT_NAME="))
        .map(str::to_owned)
}

#[cfg(target_os = "linux")]
fn lspci_model(slot: &str) -> Option<String> {
    let output = Command::new("lspci")
        .args(["-s", slot, "-nn"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .next()
        .and_then(|line| line.rsplit_once(": "))
        .map(|(_, model)| model.trim().to_owned())
        .filter(|model| !model.is_empty())
}

#[cfg(target_os = "linux")]
fn collect_nvidia_smi_gpu() -> Option<GpuInfo> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.used,temperature.gpu,clocks.current.graphics,clocks.current.memory",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let line = stdout.lines().find(|line| !line.trim().is_empty())?;
    let fields = line
        .split(',')
        .map(|field| field.trim())
        .collect::<Vec<_>>();
    if fields.len() < 6 {
        return None;
    }

    Some(GpuInfo {
        model: fields[0].to_owned(),
        vendor: "NVIDIA".to_owned(),
        vram_total_mb: parse_optional_u64(fields[1]),
        vram_used_mb: parse_optional_u64(fields[2]),
        temperature_c: parse_optional_f32(fields[3]),
        core_clock_mhz: parse_optional_u64(fields[4]),
        memory_clock_mhz: parse_optional_u64(fields[5]),
        source: "nvidia-smi".to_owned(),
    })
}

#[cfg(target_os = "linux")]
fn merge_gpu_info(mut base: GpuInfo, driver: GpuInfo) -> GpuInfo {
    if !driver.model.trim().is_empty() {
        base.model = driver.model;
    }
    base.vram_total_mb = driver.vram_total_mb.or(base.vram_total_mb);
    base.vram_used_mb = driver.vram_used_mb.or(base.vram_used_mb);
    base.temperature_c = driver.temperature_c.or(base.temperature_c);
    base.core_clock_mhz = driver.core_clock_mhz.or(base.core_clock_mhz);
    base.memory_clock_mhz = driver.memory_clock_mhz.or(base.memory_clock_mhz);
    base.source = driver.source;
    base
}

#[cfg(target_os = "linux")]
fn parse_optional_u64(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("n/a") || value.is_empty() {
        None
    } else {
        value.parse::<u64>().ok()
    }
}

#[cfg(target_os = "linux")]
fn parse_optional_f32(value: &str) -> Option<f32> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("n/a") || value.is_empty() {
        None
    } else {
        value.parse::<f32>().ok()
    }
}

#[cfg(target_os = "linux")]
fn gpu_vendor_name(vendor_id: &str) -> &'static str {
    match vendor_id.trim().to_ascii_lowercase().as_str() {
        "0x1002" => "AMD",
        "0x10de" => "NVIDIA",
        "0x8086" => "Intel",
        _ => "Unknown",
    }
}

#[cfg(target_os = "linux")]
fn read_hwmon_temperature(device_path: &Path) -> Option<f32> {
    let entries = std::fs::read_dir(device_path.join("hwmon")).ok()?;
    for entry in entries.flatten() {
        for index in 1..=8 {
            if let Some(value) = read_u64(entry.path().join(format!("temp{}_input", index))) {
                return Some(value as f32 / 1000.0);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn read_active_dpm_clock(path: impl AsRef<Path>) -> Option<u64> {
    let data = read_trimmed(path)?;
    data.lines()
        .find(|line| line.contains('*'))
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|part| part.parse::<u64>().ok())
        })
}

#[cfg(target_os = "linux")]
fn read_u64(path: impl AsRef<Path>) -> Option<u64> {
    read_trimmed(path)?.parse::<u64>().ok()
}

#[cfg(target_os = "linux")]
fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
