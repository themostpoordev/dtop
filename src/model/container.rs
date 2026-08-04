use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{Metrics, ResourceDelta};

#[derive(Clone, Debug, Default)]
pub struct ContainerMeta {
    pub started: i64,
    pub restart_count: i64,
    pub health: String,
}

#[derive(Clone, Debug, Default)]
pub struct ContainerRow {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub health: String,
    pub created: i64,
    pub started: i64,
    pub restart_count: i64,
    pub ports: String,
    pub metrics: Metrics,
    pub delta: ResourceDelta,
}

impl ContainerRow {
    pub fn uptime(&self) -> String {
        if self.started <= 0 || self.state != "running" {
            return "—".to_owned();
        }
        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs() as i64;
        format_duration(now.saturating_sub(self.started) as u64)
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_ascii_lowercase();
        self.name.to_ascii_lowercase().contains(&query)
            || self.image.to_ascii_lowercase().contains(&query)
            || self.state.to_ascii_lowercase().contains(&query)
            || self.status.to_ascii_lowercase().contains(&query)
    }
}

pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h {:02}m", minutes % 60);
    }
    format!("{}d {:02}h", hours / 24, hours % 24)
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_rate(bytes_per_second: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_second.max(0.0) as u64))
}
