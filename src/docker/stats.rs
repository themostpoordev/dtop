use std::collections::HashMap;

use crate::model::{Metrics, ResourceDelta};

#[derive(Clone, Debug, Default)]
pub struct RawStats {
    pub cpu_total: u64,
    pub system_cpu: u64,
    pub online_cpus: u64,
    pub memory: u64,
    pub memory_limit: u64,
    pub network_rx: u64,
    pub network_tx: u64,
    pub block_read: u64,
    pub block_write: u64,
    pub pids: u64,
}

pub fn metrics_from_raw(
    raw: &RawStats,
    previous: Option<&RawStats>,
    elapsed_seconds: f64,
) -> (Metrics, ResourceDelta) {
    let cpu_percent = previous
        .and_then(|old| {
            let cpu_delta = raw.cpu_total.checked_sub(old.cpu_total)? as f64;
            let system_delta = raw.system_cpu.checked_sub(old.system_cpu)? as f64;
            if system_delta == 0.0 {
                None
            } else {
                Some((cpu_delta / system_delta) * raw.online_cpus.max(1) as f64 * 100.0)
            }
        })
        .unwrap_or(0.0)
        .max(0.0);
    let rate = |current: u64, old: Option<u64>| {
        old.and_then(|value| current.checked_sub(value)).unwrap_or(0) as f64
            / elapsed_seconds.max(0.001)
    };
    (
        Metrics {
            cpu_percent,
            memory_bytes: raw.memory,
            memory_limit: raw.memory_limit,
            network_rx_bytes: raw.network_rx,
            network_tx_bytes: raw.network_tx,
            block_read_bytes: raw.block_read,
            block_write_bytes: raw.block_write,
            pids: raw.pids,
        },
        ResourceDelta {
            network_rx_rate: rate(raw.network_rx, previous.map(|v| v.network_rx)),
            network_tx_rate: rate(raw.network_tx, previous.map(|v| v.network_tx)),
            block_read_rate: rate(raw.block_read, previous.map(|v| v.block_read)),
            block_write_rate: rate(raw.block_write, previous.map(|v| v.block_write)),
        },
    )
}

pub fn sum_metrics(metrics: impl Iterator<Item = Metrics>) -> Metrics {
    metrics.fold(Metrics::default(), |mut total, item| {
        total.cpu_percent += item.cpu_percent;
        total.memory_bytes += item.memory_bytes;
        total.memory_limit += item.memory_limit;
        total.network_rx_bytes += item.network_rx_bytes;
        total.network_tx_bytes += item.network_tx_bytes;
        total.block_read_bytes += item.block_read_bytes;
        total.block_write_bytes += item.block_write_bytes;
        total.pids += item.pids;
        total
    })
}

pub type StatsCache = HashMap<String, RawStats>;
