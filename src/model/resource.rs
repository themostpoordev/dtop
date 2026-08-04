use std::collections::VecDeque;

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
