use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct History {
    pub cpu: VecDeque<u64>,
    pub memory: VecDeque<u64>,
    cpu_smoothed: Option<f64>,
    memory_smoothed: Option<f64>,
}

impl History {
    pub fn new() -> Self {
        Self {
            cpu: VecDeque::with_capacity(120),
            memory: VecDeque::with_capacity(120),
            cpu_smoothed: None,
            memory_smoothed: None,
        }
    }
    pub fn push_cpu(&mut self, value: u64) {
        let smoothed = match self.cpu_smoothed {
            Some(prev) => (value as f64 * 0.4) + (prev * 0.6),
            None => value as f64,
        };
        self.cpu_smoothed = Some(smoothed);
        if self.cpu.len() == 120 {
            self.cpu.pop_front();
        }
        self.cpu.push_back(smoothed as u64);
    }
    pub fn push_memory(&mut self, value: u64) {
        let smoothed = match self.memory_smoothed {
            Some(prev) => (value as f64 * 0.4) + (prev * 0.6),
            None => value as f64,
        };
        self.memory_smoothed = Some(smoothed);
        if self.memory.len() == 120 {
            self.memory.pop_front();
        }
        self.memory.push_back(smoothed as u64);
    }
    pub fn as_slice_cpu(&self) -> Vec<u64> {
        self.cpu.iter().copied().collect()
    }
    pub fn as_slice_memory(&self) -> Vec<u64> {
        self.memory.iter().copied().collect()
    }
}

/// Bounded f64 time series with EMA smoothing — used for disk/net rates.
/// Same shape as `History` but without the u64 cast.
#[derive(Clone, Debug, Default)]
pub struct RateSeries {
    pub values: VecDeque<f64>,
    capacity: usize,
    smoothed: Option<f64>,
}

impl RateSeries {
    pub fn new() -> Self {
        Self { values: VecDeque::with_capacity(120), capacity: 120, smoothed: None }
    }
    pub fn push(&mut self, value: f64) {
        let smoothed = match self.smoothed {
            Some(prev) => (value * 0.4) + (prev * 0.6),
            None => value,
        };
        self.smoothed = Some(smoothed);
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(smoothed);
    }
    pub fn as_slice(&self) -> Vec<f64> {
        self.values.iter().copied().collect()
    }
}

/// Host-level history for the "all" mode sparklines.
#[derive(Clone, Debug, Default)]
pub struct HostHistory {
    pub cpu: History,
    pub disk_read: RateSeries,
    pub disk_write: RateSeries,
    pub net_rx: RateSeries,
    pub net_tx: RateSeries,
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub memory_limit: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub pids: u64,
}

impl Metrics {
    pub fn memory_percent(&self) -> f64 {
        if self.memory_limit == 0 {
            0.0
        } else {
            (self.memory_bytes as f64 / self.memory_limit as f64) * 100.0
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ResourceDelta {
    pub network_rx_rate: f64,
    pub network_tx_rate: f64,
    pub block_read_rate: f64,
    pub block_write_rate: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ImageRow {
    pub id: String,
    pub tags: Vec<String>,
    pub size_bytes: u64,
    pub created: i64,
}

#[derive(Clone, Debug, Default)]
pub struct VolumeRow {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub scope: String,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkRow {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub containers: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ContainerDetails {
    pub id: String,
    pub name: String,
    pub image: String,
    pub command: String,
    pub created: String,
    pub started: String,
    pub status: String,
    pub health: String,
    pub restart_count: i64,
    pub ports: Vec<String>,
    pub mounts: Vec<String>,
    pub networks: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct BoundedLines<T> {
    pub items: VecDeque<T>,
    pub capacity: usize,
}

impl<T> BoundedLines<T> {
    pub fn new(capacity: usize) -> Self {
        Self { items: VecDeque::with_capacity(capacity.min(1024)), capacity }
    }

    pub fn push(&mut self, item: T) {
        if self.capacity == 0 {
            return;
        }
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}
