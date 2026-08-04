use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct GpuInfo {
    pub present: bool,
    pub vendor: String,
    pub gpus: Vec<GpuDevice>,
}

#[derive(Clone, Debug)]
pub struct GpuDevice {
    pub index: usize,
    pub name: String,
    pub utilization: u64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
}

impl GpuInfo {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_present(&self) -> bool {
        self.present
    }

    pub fn memory_summary(&self) -> String {
        let used: u64 = self.gpus.iter().map(|gpu| gpu.memory_used_mb).sum();
        let total: u64 = self.gpus.iter().map(|gpu| gpu.memory_total_mb).sum();
        if total == 0 {
            "—".to_owned()
        } else {
            format!("{used} MiB / {total} MiB")
        }
    }
}

pub fn detect_gpus() -> GpuInfo {
    if let Some(info) = detect_nvidia() {
        return info;
    }
    if let Some(info) = detect_amd() {
        return info;
    }
    GpuInfo::none()
}

fn detect_nvidia() -> Option<GpuInfo> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,utilization.gpu,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut gpus = Vec::new();
    for (index, line) in text.lines().filter(|line| !line.trim().is_empty()).enumerate() {
        let fields: Vec<&str> = line.split(',').map(|field| field.trim()).collect();
        if fields.len() < 5 {
            continue;
        }
        gpus.push(GpuDevice {
            index,
            name: fields[1].to_owned(),
            utilization: fields[2].parse().unwrap_or(0),
            memory_used_mb: fields[3].parse().unwrap_or(0),
            memory_total_mb: fields[4].parse().unwrap_or(0),
        });
    }
    if gpus.is_empty() {
        return None;
    }
    Some(GpuInfo { present: true, vendor: "NVIDIA".to_owned(), gpus })
}

fn detect_amd() -> Option<GpuInfo> {
    let output =
        Command::new("rocm-smi").args(["--showuse", "--showmeminfo", "vram"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut gpus = Vec::new();
    let mut current: Option<GpuDevice> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("GPU[") {
            if let Some(device) = current.take() {
                gpus.push(device);
            }
            let name = line.split(']').nth(1).unwrap_or("").trim().to_owned();
            current = Some(GpuDevice {
                index: gpus.len(),
                name,
                utilization: 0,
                memory_used_mb: 0,
                memory_total_mb: 0,
            });
        } else if let Some(device) = current.as_mut() {
            if line.contains("GPU use") {
                device.utilization = line
                    .split(':')
                    .nth(1)
                    .and_then(|v| v.trim().trim_end_matches('%').parse().ok())
                    .unwrap_or(0);
            } else if line.contains("vram") && line.contains("Used") {
                device.memory_used_mb = parse_mb(line);
            } else if line.contains("vram") && line.contains("Total") {
                device.memory_total_mb = parse_mb(line);
            }
        }
    }
    if let Some(device) = current.take() {
        gpus.push(device);
    }
    if gpus.is_empty() {
        return None;
    }
    Some(GpuInfo { present: true, vendor: "AMD".to_owned(), gpus })
}

fn parse_mb(line: &str) -> u64 {
    line.split_whitespace().nth(1).and_then(|value| value.parse::<u64>().ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn none_when_no_gpu() {
        let info = GpuInfo::none();
        assert!(!info.is_present());
        assert_eq!(info.memory_summary(), "—");
    }
}
