use crate::docker::HostMemory;

/// Host-wide metrics snapshot consumed by the UI. Bounded and fully computed —
/// never raw counters. Produced by `crate::host::HostSampler`.
#[derive(Clone, Debug, Default)]
pub struct HostStats {
    /// Total CPU utilization 0–100 (normalized across all cores).
    pub cpu_total: f64,
    /// Per-core utilization, index i = cpu{i}.
    pub cores: Vec<f64>,
    /// Load average 1 / 5 / 15 minutes.
    pub load_avg: [f64; 3],
    /// RAM + zram + swapfile usage (bytes).
    pub memory: HostMemory,
    /// Block devices, busiest first (physical whole disks preferred,
    /// virtual / busiest fallback so the list is never empty without reason).
    pub disks: Vec<DiskIo>,
    /// Network interfaces (loopback excluded), busiest first.
    pub nets: Vec<NetIo>,
    /// Block-backed filesystems from `/proc/mounts`, sorted by mountpoint.
    /// Refreshed on a slow TTL — cheap by design, never per-tick parsing.
    pub mounts: Vec<FsRow>,
    /// Top-N processes by CPU.
    pub processes: Vec<ProcessRow>,
    pub num_cpus: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DiskIo {
    pub name: String,
    /// Cumulative bytes since boot.
    pub read_bytes: u64,
    pub write_bytes: u64,
    /// Bytes per second since the previous sample.
    pub read_rate: f64,
    pub write_rate: f64,
    /// Cumulative completed I/O operations since boot.
    pub read_ops: u64,
    pub write_ops: u64,
    /// Operations per second since the previous sample.
    pub read_iops: f64,
    pub write_iops: f64,
    /// Device utilization 0–100 (io_time delta / elapsed), clamped.
    pub util: f64,
    /// Block-device capacity from `/proc/partitions`, 0 when unknown.
    pub capacity_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct NetIo {
    pub name: String,
    /// Cumulative bytes since boot.
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    /// Bytes per second since the previous sample.
    pub rx_rate: f64,
    pub tx_rate: f64,
    /// Cumulative packets since boot.
    pub rx_packets: u64,
    pub tx_packets: u64,
    /// Packets per second since the previous sample.
    pub rx_pps: f64,
    pub tx_pps: f64,
    /// Cumulative errors + drops (rx + tx) since boot. Zero on healthy links.
    pub err_total: u64,
}

#[derive(Clone, Debug, Default)]
pub struct FsRow {
    /// Source device as written in `/proc/mounts` (e.g. `/dev/sda3`).
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
    /// Partition capacity from `/proc/partitions`, 0 when unknown.
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ProcessRow {
    pub pid: i32,
    pub name: String,
    pub state: char,
    /// Multi-threaded processes legitimately exceed 100.
    pub cpu_percent: f64,
    pub mem_percent: f64,
    pub rss_bytes: u64,
    pub threads: u64,
}

impl ProcessRow {
    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_ascii_lowercase();
        self.name.to_ascii_lowercase().contains(&query)
            || self.pid.to_string().contains(&query)
            || (query.len() == 1 && query.starts_with(self.state.to_ascii_lowercase()))
    }
}
