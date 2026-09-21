//! Raw `/proc` readers — pure string parsers + thin `std::fs` wrappers.
//!
//! Every parser is total: malformed input yields defaults, never a panic.
//! This module is the single source of truth for Linux host metrics.
//!
//! Performance budget: the fast tick reads a handful of tiny `/proc` files
//! (each a single small read, ~µs). Mounts are the exception — on Docker
//! hosts `/proc/mounts` carries one line per container overlay — so the
//! sampler refreshes them on a slow 10 s TTL instead of every 500 ms tick.

use std::{collections::HashMap, fs, path::PathBuf};

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
    pub reads_completed: u64,
    pub writes_completed: u64,
    pub time_read_ms: u64,
    pub time_write_ms: u64,
    /// I/Os currently in flight (instantaneous gauge, not a counter).
    pub ios_in_progress: u64,
    /// Milliseconds spent doing I/O (counter — delta / elapsed = util%).
    pub io_time_ms: u64,
    pub weighted_ms: u64,
    /// Block-device capacity from `/proc/partitions`, 0 when unknown.
    pub capacity_bytes: u64,
}

#[derive(Clone, Debug, Default)]
pub struct NetRaw {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errs: u64,
    pub rx_drop: u64,
    pub tx_errs: u64,
    pub tx_drop: u64,
}

#[derive(Clone, Debug, Default)]
pub struct MountRaw {
    /// Source device as written in `/proc/mounts` (e.g. `/dev/sda3`).
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
    /// Partition capacity from `/proc/partitions`, 0 when unknown.
    pub size_bytes: u64,
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
    /// Block-backed filesystems. Filled by the sampler on a slow TTL —
    /// `read_host_raw` leaves it empty, `read_mounts` fills it.
    pub mounts: Vec<MountRaw>,
    pub processes: Vec<ProcRaw>,
    pub num_cpus: usize,
}

/// Read every fast-tick host counter from `/proc`. Never fails hard — a
/// missing or unreadable file leaves the corresponding section empty.
///
/// The process scan is NOT included here: it is the most expensive read (one
/// directory walk plus two files per pid) and runs on its own slower cadence.
/// Mounts are NOT included either (see `read_mounts`). Callers attach both.
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
    // One extra tiny file (`/proc/partitions`, ~hundreds of bytes) gives us
    // per-device capacity for the disk + filesystem panels at negligible cost.
    let capacities = read_partition_capacities();
    if let Ok(contents) = fs::read_to_string("/proc/diskstats") {
        raw.disks = parse_diskstats(&contents, &capacities);
    }
    if let Ok(contents) = fs::read_to_string("/proc/net/dev") {
        raw.nets = parse_net_dev(&contents);
    }
    raw
}

/// Read block-backed filesystems from `/proc/mounts`.
///
/// Kept out of the fast tick: on container hosts this file carries dozens of
/// overlay lines. The sampler calls this at most once per 10 s and reuses the
/// cached list between refreshes.
pub fn read_mounts() -> Vec<MountRaw> {
    let capacities = read_partition_capacities();
    fs::read_to_string("/proc/mounts")
        .map(|contents| parse_mounts(&contents, &capacities))
        .unwrap_or_default()
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

fn num(fields: &[&str], index: usize) -> u64 {
    fields.get(index).and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// Parse `/proc/diskstats` (full counters, not just sectors).
/// Columns (0-based): 0 major, 1 minor, 2 name, 3 reads_completed,
/// 4 reads_merged, 5 sectors_read, 6 time_read_ms, 7 writes_completed,
/// 8 writes_merged, 9 sectors_written, 10 time_write_ms, 11 in_flight,
/// 12 io_time_ms, 13 weighted_ms.
///
/// Device selection is a 3-tier fallback so the panel is never blank,
/// whatever the hardware is (SATA `sda`, virtio `vda`, NVMe `nvme0n1`,
/// `mmcblk0`, LVM `dm-0`, RAID `md0`, exotic/container setups):
/// 1. whole physical disks, 2. whole virtual devices, 3. busiest entries.
fn parse_diskstats(contents: &str, capacities: &HashMap<String, u64>) -> Vec<DiskRaw> {
    let mut all = Vec::new();
    for line in contents.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }
        let name = fields[2];
        if is_ignored_virtual(name) {
            continue;
        }
        all.push(DiskRaw {
            name: name.to_owned(),
            sectors_read: num(&fields, 5),
            sectors_written: num(&fields, 9),
            reads_completed: num(&fields, 3),
            writes_completed: num(&fields, 7),
            time_read_ms: num(&fields, 6),
            time_write_ms: num(&fields, 10),
            ios_in_progress: num(&fields, 11),
            io_time_ms: num(&fields, 12),
            weighted_ms: num(&fields, 13),
            capacity_bytes: capacities.get(name).copied().unwrap_or(0),
        });
    }
    // Tier 1: whole physical disks, in kernel order (stable across ticks).
    let physical: Vec<DiskRaw> =
        all.iter().filter(|d| is_physical_whole(&d.name)).cloned().collect();
    if !physical.is_empty() {
        return physical;
    }
    // Tier 2: whole virtual devices — LVM / RAID roots live here.
    let virtual_devs: Vec<DiskRaw> =
        all.iter().filter(|d| is_virtual_whole(&d.name)).cloned().collect();
    if !virtual_devs.is_empty() {
        return virtual_devs;
    }
    // Tier 3: whatever is busiest, so the panel always has something to show.
    let mut rest = all;
    rest.sort_by(|a, b| {
        (b.sectors_read.saturating_add(b.sectors_written))
            .cmp(&(a.sectors_read.saturating_add(a.sectors_written)))
    });
    rest.truncate(8);
    rest
}

/// Devices that are never storage: loop/ram/zram backing, floppies,
/// optical drives, the `nullb` test driver.
fn is_ignored_virtual(name: &str) -> bool {
    ["loop", "ram", "zram", "fd", "sr", "nullb"].iter().any(|p| name.starts_with(p))
}

/// Whole physical disks (partitions excluded), across naming schemes:
/// `sda`/`vda`/`hda`/`xvda` (trailing digit = partition), NVMe / MMC / NBD
/// (`p<digits>` suffix = partition), `dasda` (s390, digit rule works).
/// `dm-*` / `md*` are virtual — tier 2 handles them.
fn is_physical_whole(name: &str) -> bool {
    if name.starts_with("dm-") {
        return false;
    }
    if let Some(rest) = name.strip_prefix("md") {
        // md0, md127 — whole, but virtual (tier 2)
        if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    if name.starts_with("nvme") || name.starts_with("mmcblk") || name.starts_with("nbd") {
        if name.contains("boot") || name.contains("rpmb") {
            return false; // mmcblk0boot0 / rpmb — sub-areas, not the disk
        }
        // Partition iff the name ends with "p<digits>" (nvme0n1p1, mmcblk0p2).
        if let Some(p) = name.rfind('p') {
            let suffix = &name[p + 1..];
            if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
        }
        if name.ends_with('p') {
            return false; // nvme0n1p — truncated/partial name, not a disk
        }
        return true;
    }
    // Everything else: a trailing digit means partition (sda1, vda2, dasda1).
    !name.ends_with(|c: char| c.is_ascii_digit())
}

/// Whole virtual devices: `dm-0` (LVM), `md0`/`md127` (RAID).
fn is_virtual_whole(name: &str) -> bool {
    if let Some(rest) = name.strip_prefix("dm-") {
        return !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit());
    }
    if let Some(rest) = name.strip_prefix("md") {
        return !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit());
    }
    false
}

/// Parse `/proc/net/dev` (full counters). Skips the two header lines (no
/// colon) and loopback. Keeps discovery order — activity sorting happens in
/// `stats.rs` so the busiest interface lands on top on any machine
/// (eth*, en*, wl*, tailscale, wg, docker bridges, veths, …).
/// Right-hand fields: 0 rx_bytes, 1 rx_packets, 2 rx_errs, 3 rx_drop,
/// 8 tx_bytes, 9 tx_packets, 10 tx_errs, 11 tx_drop.
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
        nets.push(NetRaw {
            name: name.to_owned(),
            rx_bytes: fields[0],
            rx_packets: fields[1],
            rx_errs: fields[2],
            rx_drop: fields[3],
            tx_bytes: fields[8],
            tx_packets: fields[9],
            tx_errs: fields[10],
            tx_drop: fields[11],
        });
    }
    nets
}

/// Parse `/proc/mounts`, keeping real block-backed filesystems only:
/// source under `/dev/` with a physical fstype. This skips pseudo-fs
/// (proc/sysfs/cgroup/tmpfs/…) and the per-container overlay flood, so the
/// result is a handful of rows on any distro (btrfs/ext4/xfs/vfat/…).
fn parse_mounts(contents: &str, capacities: &HashMap<String, u64>) -> Vec<MountRaw> {
    let mut out = Vec::new();
    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        let (Some(device), Some(mountpoint), Some(fstype)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !device.starts_with("/dev/") || !is_real_fstype(fstype) {
            continue;
        }
        if out.iter().any(|m: &MountRaw| m.device == device && m.mountpoint == mountpoint) {
            continue;
        }
        let base = device.rsplit('/').next().unwrap_or(device);
        out.push(MountRaw {
            device: device.to_owned(),
            mountpoint: unescape_mount(mountpoint),
            fstype: fstype.to_owned(),
            size_bytes: capacities.get(base).copied().unwrap_or(0),
        });
    }
    out.sort_by(|a, b| a.mountpoint.cmp(&b.mountpoint));
    out
}

fn is_real_fstype(fstype: &str) -> bool {
    matches!(
        fstype,
        "btrfs"
            | "ext2"
            | "ext3"
            | "ext4"
            | "xfs"
            | "vfat"
            | "exfat"
            | "ntfs"
            | "ntfs3"
            | "f2fs"
            | "jfs"
            | "reiserfs"
            | "zfs"
            | "bcachefs"
            | "hfsplus"
            | "apfs"
            | "nilfs2"
            | "ufs"
    )
}

/// `/proc/mounts` escapes spaces as `\040` (and tabs/newlines similarly).
fn unescape_mount(value: &str) -> String {
    value.replace("\\040", " ").replace("\\011", "\t").replace("\\012", "\n").replace("\\\\", "\\")
}

/// Parse `/proc/partitions` into name → capacity bytes.
/// Lines are `major minor #blocks name`; `#blocks` are 1 KiB units.
fn parse_partitions(contents: &str) -> HashMap<String, u64> {
    let mut map = HashMap::new();
    for line in contents.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 {
            continue;
        }
        let blocks: u64 = fields[2].parse().unwrap_or(0);
        if blocks == 0 {
            continue; // header line or empty entry
        }
        map.insert(fields[3].to_owned(), blocks.saturating_mul(1024));
    }
    map
}

pub fn read_partition_capacities() -> HashMap<String, u64> {
    fs::read_to_string("/proc/partitions").map(|c| parse_partitions(&c)).unwrap_or_default()
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

    fn capacities() -> HashMap<String, u64> {
        HashMap::new()
    }

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
        let disks = parse_diskstats(contents, &capacities());
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].name, "sda");
        assert_eq!(disks[0].sectors_read, 3000);
        assert_eq!(disks[0].sectors_written, 6000);
        assert_eq!(disks[1].name, "nvme0n1");
    }

    #[test]
    fn parse_diskstats_full_counters() {
        let contents = "   8       0 sda 100 2 3000 40 50 3 6000 60 1 700 800\n";
        let disks = parse_diskstats(contents, &capacities());
        assert_eq!(disks.len(), 1);
        let d = &disks[0];
        assert_eq!(d.reads_completed, 100);
        assert_eq!(d.writes_completed, 50);
        assert_eq!(d.time_read_ms, 40);
        assert_eq!(d.time_write_ms, 60);
        assert_eq!(d.ios_in_progress, 1);
        assert_eq!(d.io_time_ms, 700);
        assert_eq!(d.weighted_ms, 800);
    }

    #[test]
    fn parse_diskstats_falls_back_to_dm_when_no_physical() {
        let contents = "\
 253       0 dm-0 100 0 3000 40 50 0 6000 60 0 100 200 300
   7       0 loop0 0 0 0 0 0 0 0 0 0 0 0 0 0
";
        let disks = parse_diskstats(contents, &capacities());
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].name, "dm-0");
    }

    #[test]
    fn parse_diskstats_falls_back_to_busiest_when_only_partitions() {
        let contents = "\
   8       1 sda1 100 0 3000 40 0 0 0 0 0 0 0 0 0
   8       2 sda2 10 0 100 4 0 0 0 0 0 0 0 0 0
";
        let disks = parse_diskstats(contents, &capacities());
        // No whole disk and no dm/md: busiest entries so the panel stays alive.
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].name, "sda1");
    }

    #[test]
    fn parse_diskstats_attaches_capacity() {
        let contents = "   8       0 sda 100 2 3000 40 50 3 6000 60 0 100 0 100\n";
        let mut caps = HashMap::new();
        caps.insert("sda".to_owned(), 150 * 1024 * 1024 * 1024);
        let disks = parse_diskstats(contents, &caps);
        assert_eq!(disks[0].capacity_bytes, 150 * 1024 * 1024 * 1024);
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
        assert_eq!(nets[0].name, "eth0");
        assert_eq!(nets[0].rx_bytes, 5000);
        assert_eq!(nets[0].tx_bytes, 8000);
        assert_eq!(nets[1].name, "docker0");
        assert_eq!(nets[1].rx_bytes, 111);
        assert_eq!(nets[1].tx_bytes, 222);
    }

    #[test]
    fn parse_net_dev_full_counters() {
        let contents = " eth0: 5000 20 1 2 0 0 0 0 8000 30 3 4 0 0 0 0\n";
        let nets = parse_net_dev(contents);
        assert_eq!(nets.len(), 1);
        let n = &nets[0];
        assert_eq!((n.rx_packets, n.tx_packets), (20, 30));
        assert_eq!((n.rx_errs, n.rx_drop), (1, 2));
        assert_eq!((n.tx_errs, n.tx_drop), (3, 4));
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
        assert!(is_physical_whole("sda"));
        assert!(is_physical_whole("vda"));
        assert!(is_physical_whole("nvme0n1"));
        assert!(is_physical_whole("mmcblk0"));
        assert!(is_physical_whole("nbd0"));
        assert!(is_physical_whole("dasda"));
        assert!(!is_physical_whole("sda1"));
        assert!(!is_physical_whole("vda2"));
        assert!(!is_physical_whole("nvme0n1p1"));
        assert!(!is_physical_whole("mmcblk0p2"));
        assert!(!is_physical_whole("nbd0p1"));
        assert!(!is_physical_whole("dasda1"));
        assert!(!is_physical_whole("dm-0"));
        assert!(!is_physical_whole("md0"));
        assert!(!is_physical_whole("mmcblk0boot0"));
        assert!(is_virtual_whole("dm-0"));
        assert!(is_virtual_whole("md0"));
        assert!(is_virtual_whole("md127"));
        assert!(!is_virtual_whole("sda"));
        assert!(is_ignored_virtual("loop0"));
        assert!(is_ignored_virtual("zram0"));
        assert!(is_ignored_virtual("sr0"));
        assert!(is_ignored_virtual("ram0"));
        assert!(!is_ignored_virtual("sda"));
    }

    #[test]
    fn parse_mounts_keeps_block_filesystems_only() {
        let contents = "\
/dev/sda3 / btrfs rw,noatime 0 0
/dev/sda2 /efi vfat rw 0 0
proc /proc proc rw 0 0
tmpfs /tmp tmpfs rw 0 0
overlay /var/lib/docker/overlay overlay rw 0 0
/dev/sda3 / btrfs rw,noatime 0 0
";
        let mut caps = HashMap::new();
        caps.insert("sda3".to_owned(), 100 * 1024 * 1024 * 1024);
        let mounts = parse_mounts(contents, &caps);
        // Duplicate (device, mountpoint) pair collapses; pseudo-fs skipped.
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[0].mountpoint, "/");
        assert_eq!(mounts[0].size_bytes, 100 * 1024 * 1024 * 1024);
        assert_eq!(mounts[1].mountpoint, "/efi");
    }

    #[test]
    fn parse_partitions_blocks_to_bytes() {
        let contents = "major minor  #blocks  name\n\n   8        0  157286400 sda\n   8        3  156977135 sda3\n";
        let map = parse_partitions(contents);
        assert_eq!(map["sda"], 157286400 * 1024);
        assert_eq!(map["sda3"], 156977135 * 1024);
    }
}
