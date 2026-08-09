//! Raw `/proc` readers — pure string parsers + thin `std::fs` wrappers.
//!
//! Every parser is total: malformed input yields defaults, never a panic.
//! This module is the single source of truth for Linux host metrics.

use std::{fs, path::PathBuf};

use crate::docker::read_host_memory;

pub const SECTOR_SIZE: u64 = 512;
pub const PAGE_SIZE: u64 = 4096;

#[derive(Clone, Debug, Default)]
pub struct CoreRaw {
    /// Total CPU ticks across all states.
    pub total: u64,
    /// idle + iowait ticks.
    pub idle: u64,
}

#[derive(Clone, Debug, Default)]
pub struct DiskRaw {
    pub name: String,
    pub sectors_read: u64,
    pub sectors_written: u64,
}

#[derive(Clone, Debug, Default)]
pub struct NetRaw {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ProcRaw {
    pub pid: i32,
    pub name: String,
    pub state: char,
    pub utime: u64,
    pub stime: u64,
    pub rss_pages: u64,
    pub threads: u64,
}

#[derive(Clone, Debug, Default)]
pub struct HostRaw {
    pub cpu_agg: CoreRaw,
    pub cores: Vec<CoreRaw>,
    pub load_avg: [f64; 3],
    pub memory: crate::docker::HostMemory,
    pub disks: Vec<DiskRaw>,
    pub nets: Vec<NetRaw>,
    pub processes: Vec<ProcRaw>,
    pub num_cpus: usize,
}

/// Read every host counter from `/proc`. Never fails hard — a missing or
/// unreadable file leaves the corresponding section empty.
///
/// The process scan is NOT included here: it is the most expensive read (one
/// directory walk plus two files per pid) and runs on its own slower cadence.
/// Call `scan_processes()` separately and attach the result.
pub fn read_host_raw() -> HostRaw {
    let mut raw = HostRaw { memory: read_host_memory(), ..Default::default() };
    if let Ok(contents) = fs::read_to_string("/proc/stat") {
        let (cpu_agg, cores) = parse_stat(&contents);
        raw.cpu_agg = cpu_agg;
        raw.cores = cores;
    }
    raw.num_cpus = raw.cores.len().max(1);
    if let Ok(contents) = fs::read_to_string("/proc/loadavg") {
        if let Some(line) = contents.lines().next() {
            raw.load_avg = parse_loadavg(line);
        }
    }
    if let Ok(contents) = fs::read_to_string("/proc/diskstats") {
        raw.disks = parse_diskstats(&contents);
    }
    if let Ok(contents) = fs::read_to_string("/proc/net/dev") {
        raw.nets = parse_net_dev(&contents);
    }
    raw
}

/// Parse the aggregate `cpu` line and every per-core `cpuN` line.
fn parse_stat(contents: &str) -> (CoreRaw, Vec<CoreRaw>) {
    let mut aggregate = CoreRaw::default();
    let mut cores = Vec::new();
    for line in contents.lines() {
        if line.starts_with("cpu") {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.is_empty() {
                continue;
            }
            // Fields (after the name): user nice system idle iowait irq softirq steal
            // [guest guest_nice]. guest/guest_nice are NOT added to total — they are
            // already counted inside user, adding them double-counts busy time.
            let values: Vec<u64> =
                fields.iter().skip(1).take(8).map(|f| f.parse().unwrap_or(0)).collect();
            let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0); // idle + iowait
            let total: u64 = values.iter().sum();
            if fields[0] == "cpu" {
                aggregate = CoreRaw { total, idle };
            } else if fields[0].len() > 3 && fields[0][3..].chars().all(|c| c.is_ascii_digit()) {
                cores.push(CoreRaw { total, idle });
            }
        }
    }
    (aggregate, cores)
}

fn parse_loadavg(line: &str) -> [f64; 3] {
    let mut out = [0.0f64; 3];
    for (slot, value) in out.iter_mut().zip(line.split_whitespace()) {
        *slot = value.parse().unwrap_or(0.0);
    }
    out
}

/// Parse `/proc/diskstats`, keeping only whole physical disks.
/// Columns (0-based): 0 major, 1 minor, 2 name, 5 sectors_read, 9 sectors_written.
fn parse_diskstats(contents: &str) -> Vec<DiskRaw> {
    let mut disks = Vec::new();
    for line in contents.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }
        let name = fields[2];
        if !is_whole_disk(name) {
            continue;
        }
        let sectors_read = fields[5].parse().unwrap_or(0);
        let sectors_written = fields[9].parse().unwrap_or(0);
        disks.push(DiskRaw { name: name.to_owned(), sectors_read, sectors_written });
    }
    disks
}

/// Whole physical disks only: skip loop/ram/zram/fd/sr/dm- devices and partitions.
fn is_whole_disk(name: &str) -> bool {
    for prefix in ["loop", "ram", "zram", "fd", "sr", "dm-"] {
        if name.starts_with(prefix) {
            return false;
        }
    }
    if name.ends_with("p") && (name.starts_with("nvme") || name.starts_with("mmcblk")) {
        return false; // nvme0n1p / mmcblk0p — a partition suffix without a number
    }
    // Partition heuristics:
    // - nvme/mmcblk: partition iff it ends with "p<digits>" (e.g. nvme0n1p1)
    // - everything else: partition iff it ends with a digit (e.g. sda1, vda2)
    if name.starts_with("nvme") || name.starts_with("mmcblk") {
        let Some(p) = name.rfind('p') else { return true };
        let suffix = &name[p + 1..];
        !suffix.is_empty() && !suffix.chars().all(|c| c.is_ascii_digit())
    } else {
        !name.ends_with(|c: char| c.is_ascii_digit())
    }
}

/// Parse `/proc/net/dev`. Skip the two header lines; split on `:` — left is the
/// interface, right is 16 numeric fields. rx_bytes = field 0, tx_bytes = field 8.
fn parse_net_dev(contents: &str) -> Vec<NetRaw> {
    let mut nets = Vec::new();
    for line in contents.lines() {
        let Some((left, right)) = line.split_once(':') else { continue };
        let name = left.trim();
        if name.is_empty() || name == "lo" {
            continue;
        }
        let fields: Vec<u64> = right.split_whitespace().map(|f| f.parse().unwrap_or(0)).collect();
        if fields.len() < 16 {
            continue;
        }
        nets.push(NetRaw { name: name.to_owned(), rx_bytes: fields[0], tx_bytes: fields[8] });
    }
    nets.sort_by(|a, b| a.name.cmp(&b.name));
    nets
}

/// Scan `/proc` for process-group leaders. Skips threads (pid != tgid) so RSS and
/// CPU are never double-counted, and silently skips processes that vanish mid-scan.
///
/// This is the most expensive host read (one directory walk plus two file reads
/// per pid). Callers should run it on a slower cadence than the fast tick.
pub fn scan_processes() -> Vec<ProcRaw> {
    let Ok(entries) = fs::read_dir("/proc") else { return Vec::new() };
    let mut processes = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|name| name.parse::<i32>().ok()) else {
            continue;
        };
        let Some(raw) = read_proc(pid) else { continue };
        if raw.pid != raw.tgid {
            continue; // thread, not a process-group leader
        }
        processes.push(ProcRaw {
            pid,
            name: raw.name,
            state: raw.state,
            utime: raw.utime,
            stime: raw.stime,
            rss_pages: raw.rss_pages,
            threads: raw.threads,
        });
    }
    processes
}

struct ProcInfo {
    pid: i32,
    tgid: i32,
    name: String,
    state: char,
    utime: u64,
    stime: u64,
    rss_pages: u64,
    threads: u64,
}

fn read_proc(pid: i32) -> Option<ProcInfo> {
    let dir = PathBuf::from("/proc").join(pid.to_string());
    let stat = fs::read_to_string(dir.join("stat")).ok()?;
    let status = fs::read_to_string(dir.join("status")).ok()?;
    let stat_parsed = parse_proc_stat_line(&stat)?;
    Some(ProcInfo {
        pid,
        tgid: parse_tgid(&status),
        name: stat_parsed.name,
        state: stat_parsed.state,
        utime: stat_parsed.utime,
        stime: stat_parsed.stime,
        rss_pages: stat_parsed.rss_pages,
        threads: stat_parsed.threads,
    })
}

fn parse_tgid(status: &str) -> i32 {
    status
        .lines()
        .find(|line| line.starts_with("Tgid:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(-1)
}

/// Parse a `/proc/{pid}/stat` line. The comm field is parenthesized and may
/// contain spaces and `)` — split at the *last* `)` and index the remainder.
/// Remainder indices (0-based): 0 state, 11 utime, 12 stime, 17 threads, 21 rss.
fn parse_proc_stat_line(line: &str) -> Option<ProcRaw> {
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    if close <= open {
        return None;
    }
    let name = line[open + 1..close].to_owned();
    let rest: Vec<&str> = line[close + 1..].split_whitespace().collect();
    Some(ProcRaw {
        pid: 0, // filled by caller
        name,
        state: rest.first().and_then(|s| s.chars().next()).unwrap_or('?'),
        utime: rest.get(11).and_then(|s| s.parse().ok()).unwrap_or(0),
        stime: rest.get(12).and_then(|s| s.parse().ok()).unwrap_or(0),
        threads: rest.get(17).and_then(|s| s.parse().ok()).unwrap_or(0),
        rss_pages: rest.get(21).and_then(|s| s.parse().ok()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stat_aggregate_and_cores() {
        let contents = "\
cpu  100 0 50 800 100 0 10 0 0 0
cpu0 50 0 20 400 50 0 5 0 0 0
cpu1 50 0 30 400 50 0 5 0 0 0
intr 12345
ctxt 67890
";
        let (agg, cores) = parse_stat(contents);
        // total = user+nice+system+idle+iowait+irq+softirq+steal
        assert_eq!(agg.total, 100 + 50 + 800 + 100 + 10);
        assert_eq!(agg.idle, 800 + 100); // idle + iowait
        assert_eq!(cores.len(), 2);
        assert_eq!(cores[0].total, 50 + 20 + 400 + 50 + 5);
        assert_eq!(cores[1].idle, 400 + 50);
    }

    #[test]
    fn parse_stat_handles_short_lines() {
        let (agg, cores) = parse_stat("cpu  100 0 50 800\ncpu0 1 2 3 4\n");
        assert_eq!(agg.total, 950);
        assert_eq!(cores.len(), 1);
    }

    #[test]
    fn parse_loadavg_three_fields() {
        assert_eq!(parse_loadavg("0.52 0.31 0.25 1/234 5678"), [0.52, 0.31, 0.25]);
        assert_eq!(parse_loadavg("garbage"), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn parse_diskstats_filters_partitions_and_virtual() {
        let contents = "\
  8       0 sda 100 2 3000 40 50 3 6000 60 0 100 0 100
  8       1 sda1 100 0 3000 40 0 0 0 0 0 0 0 0 0
 253       0 dm-0 0 0 0 0 0 0 0 0 0 0 0 0 0
   7       0 loop0 0 0 0 0 0 0 0 0 0 0 0 0 0
259       0 nvme0n1 200 0 4000 10 20 0 8000 20 0 0 0 0
259       1 nvme0n1p1 200 0 4000 10 0 0 0 0 0 0 0 0 0
";
        let disks = parse_diskstats(contents);
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].name, "sda");
        assert_eq!(disks[0].sectors_read, 3000);
        assert_eq!(disks[0].sectors_written, 6000);
        assert_eq!(disks[1].name, "nvme0n1");
    }

    #[test]
    fn parse_net_dev_skips_lo_and_headers() {
        let contents = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo: 1000 10 0 0 0 0 0 0 1000 10 0 0 0 0 0 0
  eth0: 5000 20 0 0 0 0 0 0 8000 30 0 0 0 0 0 0
 docker0: 111 1 0 0 0 0 0 0 222 2 0 0 0 0 0 0
";
        let nets = parse_net_dev(contents);
        assert_eq!(nets.len(), 2);
        assert_eq!(nets[0].name, "docker0");
        assert_eq!(nets[0].rx_bytes, 111);
        assert_eq!(nets[0].tx_bytes, 222);
        assert_eq!(nets[1].name, "eth0");
        assert_eq!(nets[1].rx_bytes, 5000);
        assert_eq!(nets[1].tx_bytes, 8000);
    }

    #[test]
    fn parse_proc_stat_line_comm_with_spaces_and_parens() {
        // comm "(some) weird (name)" — must split at the last ')'
        let line = "1234 (some) weird (name) S 100 200 300 400 500 600 700 800 900 1000 1100 1200 \
             1300 1400 1500 1600 1700 1800 1900 2000 2100 2200 2300 2400 2500";
        let raw = parse_proc_stat_line(line).unwrap();
        assert_eq!(raw.name, "some) weird (name");
        assert_eq!(raw.state, 'S');
        assert_eq!(raw.utime, 1100);
        assert_eq!(raw.stime, 1200);
        assert_eq!(raw.threads, 1700);
        assert_eq!(raw.rss_pages, 2100);
    }

    #[test]
    fn is_whole_disk_heuristics() {
        assert!(is_whole_disk("sda"));
        assert!(is_whole_disk("vda"));
        assert!(is_whole_disk("nvme0n1"));
        assert!(is_whole_disk("mmcblk0"));
        assert!(!is_whole_disk("sda1"));
        assert!(!is_whole_disk("vda2"));
        assert!(!is_whole_disk("nvme0n1p1"));
        assert!(!is_whole_disk("mmcblk0p2"));
        assert!(!is_whole_disk("loop0"));
        assert!(!is_whole_disk("zram0"));
        assert!(!is_whole_disk("dm-0"));
        assert!(!is_whole_disk("sr0"));
    }
}
