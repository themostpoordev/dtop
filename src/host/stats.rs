//! Delta math: turn two `HostRaw` samples into a bounded `HostStats`.
//! All arithmetic is saturating; the first sample yields zeros.

use std::collections::HashMap;

use super::proc::{CoreRaw, DiskRaw, HostRaw, NetRaw, ProcRaw, PAGE_SIZE, SECTOR_SIZE};
use crate::model::{DiskIo, FsRow, HostStats, NetIo, ProcessRow};

/// Upper bound on the emitted process list. The raw list stays in sampler state
/// so next-tick deltas stay correct — only what the UI sees is truncated.
pub const MAX_PROCESSES: usize = 64;

/// Exponential smoothing factor for per-process CPU% — tames the 0%↔400%
/// jumps that raw 500 ms deltas produce. 0.4 new / 0.6 previous.
pub const PROC_CPU_SMOOTHING: f64 = 0.4;

/// Smooth per-process CPU% across ticks. `previous` is the last displayed
/// value for that pid (None for a brand-new process → raw value).
pub fn smooth_process_cpu(raw: f64, previous: Option<f64>, total_delta: u64, elapsed: f64) -> f64 {
    // If the host was idle (no ticks consumed), a process that woke up mid-gap
    // would look like 400%. Scale by the expected sample length so the number
    // stays sane even on the first sample after a stall.
    let scaled = if total_delta == 0 {
        0.0
    } else {
        let expected_samples = (elapsed / 0.5).clamp(1.0, 4.0);
        raw * (1.0 / expected_samples)
    };
    match previous {
        Some(prev) => (scaled * PROC_CPU_SMOOTHING) + (prev * (1.0 - PROC_CPU_SMOOTHING)),
        None => scaled,
    }
}

pub fn host_stats_from_raw(
    current: &HostRaw,
    previous: Option<&HostRaw>,
    elapsed: f64,
    proc_cpu_smooth: &mut HashMap<i32, f64>,
) -> HostStats {
    let elapsed = elapsed.max(0.001);
    // First sample: treat the previous sample as identical to the current one,
    // so every delta is zero (rates and CPU% start at 0) instead of computing
    // against a zero baseline.
    let prev = previous.unwrap_or(current);
    let num_cpus = current.num_cpus.max(1);

    let cpu_total = cpu_percent(&prev.cpu_agg, &current.cpu_agg);
    let cores = current
        .cores
        .iter()
        .zip(prev.cores.iter())
        .map(|(now, then)| cpu_percent(then, now))
        .collect::<Vec<_>>();

    let mut disks = disk_rows(&current.disks, &prev.disks, elapsed);
    // Busiest first: on docker hosts the interesting device is `sda`/`vda`/
    // `nvme0n1`, on LVM roots it is `dm-0` — sorting puts it on top on any
    // machine instead of relying on kernel discovery order.
    disks.sort_by(|a, b| (b.read_rate + b.write_rate).total_cmp(&(a.read_rate + a.write_rate)));
    let mut nets = net_rows(&current.nets, &prev.nets, elapsed);
    // Same idea: eth0/wlan0/tailscale first, idle veths last.
    nets.sort_by(|a, b| (b.rx_rate + b.tx_rate).total_cmp(&(a.rx_rate + a.tx_rate)));

    // Process CPU% uses the SAME sample pair's aggregate totals, so a process
    // can never exceed 100% of the host per-thread time.
    let total_delta = current.cpu_agg.total.saturating_sub(prev.cpu_agg.total);
    let ram_total = current.memory.ram_total.max(1);
    let mut processes = process_rows(
        &current.processes,
        &prev.processes,
        total_delta,
        num_cpus,
        elapsed,
        ram_total,
        proc_cpu_smooth,
    );
    processes.sort_by(|a, b| {
        b.cpu_percent.total_cmp(&a.cpu_percent).then_with(|| b.rss_bytes.cmp(&a.rss_bytes))
    });
    processes.truncate(MAX_PROCESSES);

    HostStats {
        cpu_total,
        cores,
        load_avg: current.load_avg,
        memory: current.memory.clone(),
        disks,
        nets,
        mounts: current
            .mounts
            .iter()
            .map(|m| FsRow {
                device: m.device.clone(),
                mountpoint: m.mountpoint.clone(),
                fstype: m.fstype.clone(),
                size_bytes: m.size_bytes,
            })
            .collect(),
        processes,
        num_cpus,
    }
}

fn cpu_percent(prev: &CoreRaw, current: &CoreRaw) -> f64 {
    let d_total = current.total.saturating_sub(prev.total);
    if d_total == 0 {
        return 0.0;
    }
    let d_idle = current.idle.saturating_sub(prev.idle);
    ((d_total - d_idle) as f64 / d_total as f64 * 100.0).clamp(0.0, 100.0)
}

fn disk_rows(current: &[DiskRaw], previous: &[DiskRaw], elapsed: f64) -> Vec<DiskIo> {
    let prev_by_name = previous.iter().map(|d| (d.name.as_str(), d)).collect::<HashMap<_, _>>();
    let elapsed_ms = (elapsed * 1000.0).max(1.0);
    current
        .iter()
        .map(|d| {
            let prev = prev_by_name.get(d.name.as_str()).copied();
            let read_delta = d.sectors_read.saturating_sub(prev.map_or(0, |p| p.sectors_read));
            let write_delta =
                d.sectors_written.saturating_sub(prev.map_or(0, |p| p.sectors_written));
            let read_op_delta =
                d.reads_completed.saturating_sub(prev.map_or(0, |p| p.reads_completed));
            let write_op_delta =
                d.writes_completed.saturating_sub(prev.map_or(0, |p| p.writes_completed));
            // Utilization: ms spent doing I/O per wall-clock ms, clamped —
            // parallel queues can otherwise report >100%.
            let io_delta_ms = d.io_time_ms.saturating_sub(prev.map_or(0, |p| p.io_time_ms)) as f64;
            DiskIo {
                name: d.name.clone(),
                read_bytes: d.sectors_read * SECTOR_SIZE,
                write_bytes: d.sectors_written * SECTOR_SIZE,
                read_rate: read_delta as f64 * SECTOR_SIZE as f64 / elapsed,
                write_rate: write_delta as f64 * SECTOR_SIZE as f64 / elapsed,
                read_ops: d.reads_completed,
                write_ops: d.writes_completed,
                read_iops: read_op_delta as f64 / elapsed,
                write_iops: write_op_delta as f64 / elapsed,
                util: (io_delta_ms / elapsed_ms * 100.0).clamp(0.0, 100.0),
                capacity_bytes: d.capacity_bytes,
            }
        })
        .collect()
}

fn net_rows(current: &[NetRaw], previous: &[NetRaw], elapsed: f64) -> Vec<NetIo> {
    let prev_by_name = previous.iter().map(|n| (n.name.as_str(), n)).collect::<HashMap<_, _>>();
    current
        .iter()
        .map(|n| {
            let prev = prev_by_name.get(n.name.as_str()).copied();
            let rx_delta = n.rx_bytes.saturating_sub(prev.map_or(0, |p| p.rx_bytes));
            let tx_delta = n.tx_bytes.saturating_sub(prev.map_or(0, |p| p.tx_bytes));
            let rx_pkt_delta = n.rx_packets.saturating_sub(prev.map_or(0, |p| p.rx_packets));
            let tx_pkt_delta = n.tx_packets.saturating_sub(prev.map_or(0, |p| p.tx_packets));
            NetIo {
                name: n.name.clone(),
                rx_bytes: n.rx_bytes,
                tx_bytes: n.tx_bytes,
                rx_rate: rx_delta as f64 / elapsed,
                tx_rate: tx_delta as f64 / elapsed,
                rx_packets: n.rx_packets,
                tx_packets: n.tx_packets,
                rx_pps: rx_pkt_delta as f64 / elapsed,
                tx_pps: tx_pkt_delta as f64 / elapsed,
                err_total: n
                    .rx_errs
                    .saturating_add(n.rx_drop)
                    .saturating_add(n.tx_errs)
                    .saturating_add(n.tx_drop),
            }
        })
        .collect()
}

fn process_rows(
    current: &[ProcRaw],
    previous: &[ProcRaw],
    total_delta: u64,
    num_cpus: usize,
    elapsed: f64,
    ram_total: u64,
    proc_cpu_smooth: &mut HashMap<i32, f64>,
) -> Vec<ProcessRow> {
    let prev_by_pid = previous.iter().map(|p| (p.pid, p)).collect::<HashMap<_, _>>();
    let rows = current
        .iter()
        .map(|p| {
            let prev = prev_by_pid.get(&p.pid).copied();
            let d_proc = p
                .utime
                .saturating_add(p.stime)
                .saturating_sub(prev.map_or(0, |prev| prev.utime.saturating_add(prev.stime)));
            let raw_cpu = if total_delta == 0 {
                0.0
            } else {
                d_proc as f64 / total_delta as f64 * num_cpus as f64 * 100.0
            };
            // EMA-smooth per-process CPU% so the list doesn't flicker between
            // 0% and 400% on short 500 ms deltas.
            let previous_smooth = proc_cpu_smooth.get(&p.pid).copied();
            let cpu_percent = smooth_process_cpu(raw_cpu, previous_smooth, total_delta, elapsed);
            proc_cpu_smooth.insert(p.pid, cpu_percent);
            let rss_bytes = p.rss_pages.saturating_mul(PAGE_SIZE);
            let mem_percent = (rss_bytes as f64 / ram_total as f64) * 100.0;
            ProcessRow {
                pid: p.pid,
                name: p.name.clone(),
                state: p.state,
                cpu_percent,
                mem_percent,
                rss_bytes,
                threads: p.threads,
            }
        })
        .collect();
    // Drop smoothing state for processes that no longer exist.
    proc_cpu_smooth.retain(|pid, _| current.iter().any(|p| p.pid == *pid));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::HostMemory;

    fn raw_with(cpu_agg: CoreRaw, cores: Vec<CoreRaw>) -> HostRaw {
        let num_cpus = cores.len().max(1);
        HostRaw { cpu_agg, cores, num_cpus, ..Default::default() }
    }

    #[test]
    fn first_sample_is_zero() {
        let current =
            raw_with(CoreRaw { total: 1000, idle: 800 }, vec![CoreRaw { total: 500, idle: 400 }]);
        let stats = host_stats_from_raw(&current, None, 0.5, &mut HashMap::new());
        assert_eq!(stats.cpu_total, 0.0);
        assert_eq!(stats.cores, vec![0.0]);
        assert!(stats.disks.is_empty());
        assert!(stats.nets.is_empty());
        assert!(stats.processes.is_empty());
    }

    #[test]
    fn cpu_percent_from_two_samples() {
        let prev =
            raw_with(CoreRaw { total: 1000, idle: 800 }, vec![CoreRaw { total: 500, idle: 400 }]);
        let current = raw_with(
            CoreRaw { total: 2000, idle: 1200 }, // busy: 600 of 1000 ticks = 60%
            vec![CoreRaw { total: 1000, idle: 600 }], // busy: 300 of 500 ticks = 60%
        );
        let stats = host_stats_from_raw(&current, Some(&prev), 0.5, &mut HashMap::new());
        assert_eq!(stats.cpu_total, 60.0);
        assert_eq!(stats.cores[0], 60.0);
    }

    #[test]
    fn cpu_percent_no_divide_by_zero() {
        let prev = raw_with(CoreRaw { total: 100, idle: 80 }, vec![]);
        let current = raw_with(CoreRaw { total: 100, idle: 80 }, vec![]);
        let stats = host_stats_from_raw(&current, Some(&prev), 0.5, &mut HashMap::new());
        assert_eq!(stats.cpu_total, 0.0);
    }

    #[test]
    fn disk_rates_use_sectors_times_512() {
        let mut prev = raw_with(CoreRaw::default(), vec![]);
        prev.disks = vec![DiskRaw {
            name: "sda".into(),
            sectors_read: 100,
            sectors_written: 50,
            reads_completed: 10,
            writes_completed: 5,
            io_time_ms: 20,
            ..Default::default()
        }];
        let mut current = raw_with(CoreRaw::default(), vec![]);
        current.disks = vec![DiskRaw {
            name: "sda".into(),
            sectors_read: 300,
            sectors_written: 150,
            reads_completed: 30,
            writes_completed: 15,
            io_time_ms: 220,
            ..Default::default()
        }];
        let stats = host_stats_from_raw(&current, Some(&prev), 2.0, &mut HashMap::new());
        assert_eq!(stats.disks.len(), 1);
        // (300-100)*512 / 2 = 51200 B/s
        assert_eq!(stats.disks[0].read_rate, 51200.0);
        assert_eq!(stats.disks[0].write_rate, 25600.0);
        // IOPS: (30-10)/2 = 10, (15-5)/2 = 5
        assert_eq!(stats.disks[0].read_iops, 10.0);
        assert_eq!(stats.disks[0].write_iops, 5.0);
        // util: 200 ms of I/O over 2000 ms = 10%
        assert_eq!(stats.disks[0].util, 10.0);
    }

    #[test]
    fn net_rates_from_two_samples() {
        let mut prev = raw_with(CoreRaw::default(), vec![]);
        prev.nets = vec![NetRaw {
            name: "eth0".into(),
            rx_bytes: 1000,
            tx_bytes: 2000,
            rx_packets: 10,
            tx_packets: 20,
            ..Default::default()
        }];
        let mut current = raw_with(CoreRaw::default(), vec![]);
        current.nets = vec![NetRaw {
            name: "eth0".into(),
            rx_bytes: 3000,
            tx_bytes: 2000,
            rx_packets: 30,
            tx_packets: 20,
            ..Default::default()
        }];
        let stats = host_stats_from_raw(&current, Some(&prev), 1.0, &mut HashMap::new());
        assert_eq!(stats.nets[0].rx_rate, 2000.0);
        assert_eq!(stats.nets[0].tx_rate, 0.0);
        assert_eq!(stats.nets[0].rx_pps, 20.0);
        assert_eq!(stats.nets[0].tx_pps, 0.0);
    }

    #[test]
    fn disks_and_nets_sort_busiest_first() {
        let mut prev = raw_with(CoreRaw::default(), vec![]);
        prev.disks = vec![
            DiskRaw { name: "sda".into(), ..Default::default() },
            DiskRaw { name: "sdb".into(), ..Default::default() },
        ];
        prev.nets = vec![
            NetRaw { name: "eth0".into(), ..Default::default() },
            NetRaw { name: "vethX".into(), ..Default::default() },
        ];
        let mut current = raw_with(CoreRaw::default(), vec![]);
        current.disks = vec![
            DiskRaw { name: "sda".into(), sectors_read: 10, ..Default::default() },
            DiskRaw { name: "sdb".into(), sectors_read: 1000, ..Default::default() },
        ];
        current.nets = vec![
            NetRaw { name: "eth0".into(), rx_bytes: 10, ..Default::default() },
            NetRaw { name: "vethX".into(), rx_bytes: 9000, ..Default::default() },
        ];
        let stats = host_stats_from_raw(&current, Some(&prev), 1.0, &mut HashMap::new());
        assert_eq!(stats.disks[0].name, "sdb");
        assert_eq!(stats.nets[0].name, "vethX");
    }

    #[test]
    fn process_cpu_uses_host_total_delta() {
        let mut prev = raw_with(CoreRaw { total: 1000, idle: 0 }, vec![]);
        prev.processes = vec![ProcRaw {
            pid: 1,
            name: "a".into(),
            state: 'R',
            utime: 100,
            stime: 50,
            rss_pages: 1024,
            threads: 1,
        }];
        let mut current = raw_with(CoreRaw { total: 2000, idle: 0 }, vec![]);
        current.processes = vec![ProcRaw {
            pid: 1,
            name: "a".into(),
            state: 'R',
            utime: 200,
            stime: 100,
            rss_pages: 1024,
            threads: 1,
        }];
        current.memory = HostMemory { ram_total: 1024 * 1024, ..Default::default() };
        let stats = host_stats_from_raw(&current, Some(&prev), 0.5, &mut HashMap::new());
        assert_eq!(stats.processes.len(), 1);
        // d_proc=150, d_total=1000, 1 cpu → 15%
        assert_eq!(stats.processes[0].cpu_percent, 15.0);
        // rss 1024 pages * 4096 = 4 MiB of 1 MiB ram → 400%
        assert_eq!(stats.processes[0].mem_percent, 400.0);
        assert_eq!(stats.processes[0].rss_bytes, 4 * 1024 * 1024);
    }

    #[test]
    fn processes_bounded_to_max() {
        let mut prev = raw_with(CoreRaw { total: 1000, idle: 0 }, vec![]);
        let mut current = raw_with(CoreRaw { total: 2000, idle: 0 }, vec![]);
        for i in 0..100 {
            prev.processes.push(ProcRaw {
                pid: i,
                name: "p".into(),
                state: 'R',
                utime: 0,
                stime: 0,
                rss_pages: 1,
                threads: 1,
            });
            current.processes.push(ProcRaw {
                pid: i,
                name: "p".into(),
                state: 'R',
                utime: 10,
                stime: 5,
                rss_pages: 1,
                threads: 1,
            });
        }
        let stats = host_stats_from_raw(&current, Some(&prev), 0.5, &mut HashMap::new());
        assert_eq!(stats.processes.len(), MAX_PROCESSES);
    }
}
