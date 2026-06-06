use std::{fs, thread, time::Duration};

use crate::models::SystemSummary;

#[derive(Debug)]
struct ProcStatSample {
    idle: u64,
    total: u64,
}

pub fn collect_summary(listener_count: usize, warnings: &mut Vec<String>) -> SystemSummary {
    let cpu_usage_percent = read_cpu_usage(warnings);
    let (memory_total_bytes, memory_available_bytes) = read_memory_bytes(warnings);
    let memory_used_bytes = memory_total_bytes.saturating_sub(memory_available_bytes);
    let load_averages = read_load_averages(warnings);
    let uptime_seconds = read_uptime_seconds(warnings);

    SystemSummary {
        cpu_usage_percent,
        memory_total_bytes,
        memory_used_bytes,
        memory_available_bytes,
        load_averages,
        uptime_seconds,
        listener_count,
    }
}

fn read_cpu_usage(warnings: &mut Vec<String>) -> Option<f32> {
    let first = match read_proc_stat_sample() {
        Ok(sample) => sample,
        Err(error) => {
            warnings.push(format!("cpu collector: {error}"));
            return None;
        }
    };

    thread::sleep(Duration::from_millis(180));

    let second = match read_proc_stat_sample() {
        Ok(sample) => sample,
        Err(error) => {
            warnings.push(format!("cpu collector second sample: {error}"));
            return None;
        }
    };

    let total_delta = second.total.saturating_sub(first.total);
    if total_delta == 0 {
        return None;
    }

    let idle_delta = second.idle.saturating_sub(first.idle);
    let busy_delta = total_delta.saturating_sub(idle_delta);
    Some((busy_delta as f32 / total_delta as f32) * 100.0)
}

fn read_proc_stat_sample() -> Result<ProcStatSample, String> {
    let content = fs::read_to_string("/proc/stat").map_err(|error| error.to_string())?;
    let cpu_line = content
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or_else(|| "missing aggregate cpu line".to_string())?;

    let mut parts = cpu_line.split_whitespace().skip(1);
    let user = next_u64(&mut parts, "user")?;
    let nice = next_u64(&mut parts, "nice")?;
    let system = next_u64(&mut parts, "system")?;
    let idle = next_u64(&mut parts, "idle")?;
    let iowait = next_u64(&mut parts, "iowait")?;
    let irq = next_u64(&mut parts, "irq")?;
    let softirq = next_u64(&mut parts, "softirq")?;
    let steal = next_u64(&mut parts, "steal")?;

    let idle_total = idle.saturating_add(iowait);
    let total = user
        .saturating_add(nice)
        .saturating_add(system)
        .saturating_add(idle)
        .saturating_add(iowait)
        .saturating_add(irq)
        .saturating_add(softirq)
        .saturating_add(steal);

    Ok(ProcStatSample {
        idle: idle_total,
        total,
    })
}

fn next_u64<'a>(parts: &mut impl Iterator<Item = &'a str>, label: &str) -> Result<u64, String> {
    let raw = parts
        .next()
        .ok_or_else(|| format!("missing proc stat field: {label}"))?;
    raw.parse::<u64>()
        .map_err(|error| format!("invalid proc stat field {label}: {error}"))
}

fn read_memory_bytes(warnings: &mut Vec<String>) -> (u64, u64) {
    match fs::read_to_string("/proc/meminfo") {
        Ok(content) => {
            let total_kib = extract_meminfo_value(&content, "MemTotal").unwrap_or_else(|error| {
                warnings.push(format!("memory collector total: {error}"));
                0
            });
            let available_kib =
                extract_meminfo_value(&content, "MemAvailable").unwrap_or_else(|error| {
                    warnings.push(format!("memory collector available: {error}"));
                    0
                });

            (
                total_kib.saturating_mul(1024),
                available_kib.saturating_mul(1024),
            )
        }
        Err(error) => {
            warnings.push(format!("memory collector: {error}"));
            (0, 0)
        }
    }
}

fn extract_meminfo_value(content: &str, key: &str) -> Result<u64, String> {
    let line = content
        .lines()
        .find(|line| line.starts_with(key))
        .ok_or_else(|| format!("{key} missing from /proc/meminfo"))?;

    let raw = line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| format!("{key} missing numeric field"))?;

    raw.parse::<u64>()
        .map_err(|error| format!("invalid meminfo value for {key}: {error}"))
}

fn read_load_averages(warnings: &mut Vec<String>) -> [f32; 3] {
    match fs::read_to_string("/proc/loadavg") {
        Ok(content) => {
            let values = content
                .split_whitespace()
                .take(3)
                .map(|raw| raw.parse::<f32>())
                .collect::<Result<Vec<_>, _>>();

            match values {
                Ok(values) if values.len() == 3 => [values[0], values[1], values[2]],
                Ok(_) => {
                    warnings.push("load collector: expected three load averages".to_string());
                    [0.0, 0.0, 0.0]
                }
                Err(error) => {
                    warnings.push(format!("load collector parse error: {error}"));
                    [0.0, 0.0, 0.0]
                }
            }
        }
        Err(error) => {
            warnings.push(format!("load collector: {error}"));
            [0.0, 0.0, 0.0]
        }
    }
}

fn read_uptime_seconds(warnings: &mut Vec<String>) -> u64 {
    match fs::read_to_string("/proc/uptime") {
        Ok(content) => {
            let raw = content.split_whitespace().next().unwrap_or("0");
            let seconds = raw.parse::<f64>().unwrap_or(0.0);
            seconds.max(0.0) as u64
        }
        Err(error) => {
            warnings.push(format!("uptime collector: {error}"));
            0
        }
    }
}
