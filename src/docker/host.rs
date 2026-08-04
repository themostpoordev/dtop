use std::fs;

#[derive(Clone, Debug, Default)]
pub struct HostMemory {
    pub ram_total: u64,
    pub ram_used: u64,
    pub zram_total: u64,
    pub zram_used: u64,
    pub swapfile_total: u64,
    pub swapfile_used: u64,
}

impl HostMemory {
    pub fn total(&self) -> u64 {
        self.ram_total.saturating_add(self.zram_total).saturating_add(self.swapfile_total)
    }
    pub fn used(&self) -> u64 {
        self.ram_used.saturating_add(self.zram_used).saturating_add(self.swapfile_used)
    }
}

pub fn read_host_memory() -> HostMemory {
    let mut memory = HostMemory::default();
    if let Ok(contents) = fs::read_to_string("/proc/meminfo") {
        let mut total_kb = 0u64;
        let mut available_kb = 0u64;
        for line in contents.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = parse_kb(line);
            } else if line.starts_with("MemAvailable:") {
                available_kb = parse_kb(line);
            }
        }
        memory.ram_total = total_kb * 1024;
        memory.ram_used = total_kb.saturating_sub(available_kb) * 1024;
    }
    if let Ok(contents) = fs::read_to_string("/proc/swaps") {
        for line in contents.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 5 {
                continue;
            }
            let filename = fields[0];
            let total_kb: u64 = fields[2].parse().unwrap_or(0);
            let used_kb: u64 = fields[3].parse().unwrap_or(0);
            if filename.contains("zram") {
                memory.zram_total = total_kb * 1024;
                memory.zram_used = used_kb * 1024;
            } else if filename.starts_with('/') {
                memory.swapfile_total = total_kb * 1024;
                memory.swapfile_used = used_kb * 1024;
            }
        }
    }
    memory
}

fn parse_kb(line: &str) -> u64 {
    line.split_whitespace().nth(1).and_then(|value| value.parse().ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_kb_handles_meminfo_line() {
        assert_eq!(parse_kb("MemTotal:       16261224 kB"), 16_261_224);
        assert_eq!(parse_kb("garbage"), 0);
    }
}
