//! `HostSampler`: keeps the previous raw sample and computes deltas on each tick.
//! Fast files are read every tick; the process scan runs on a slower cadence so
//! a large `/proc` never stalls the 500 ms loop.

use std::time::{Duration, Instant};

use super::{
    proc::{read_host_raw, scan_processes, HostRaw},
    stats::host_stats_from_raw,
};
use crate::model::HostStats;

/// Process scan cadence — btop's default is ~2 s; scanning `/proc` on every
/// 500 ms tick would waste ~4/5 of the cost on unchanged data.
const PROC_SCAN_INTERVAL: Duration = Duration::from_millis(2000);

pub struct HostSampler {
    state: Option<HostSampleState>,
    last_proc_scan: Option<Instant>,
}

struct HostSampleState {
    current: HostRaw,
    sampled_at: Instant,
}

impl Default for HostSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl HostSampler {
    pub fn new() -> Self {
        Self { state: None, last_proc_scan: None }
    }

    /// Sample the host. The first tick returns zeros for every rate; results
    /// are meaningful from the second tick onward.
    pub fn tick(&mut self) -> HostStats {
        let now = Instant::now();
        let mut current = read_host_raw();

        // The process scan is by far the most expensive read. Run it only when
        // due and reuse the previous list otherwise — CPU% for the reused list
        // still comes from fresh `/proc/stat` deltas, so values stay accurate.
        let rescan = match self.last_proc_scan {
            Some(last) => now.duration_since(last) >= PROC_SCAN_INTERVAL,
            None => true,
        };
        if rescan {
            current.processes = scan_processes();
            self.last_proc_scan = Some(now);
        } else if let Some(state) = &self.state {
            current.processes = state.current.processes.clone();
        }

        // The stored sample *is* the previous sample for this tick.
        let previous = self.state.as_ref().map(|state| &state.current);
        let elapsed = self
            .state
            .as_ref()
            .map(|state| state.sampled_at.elapsed().as_secs_f64().max(0.001))
            .unwrap_or(0.0);
        let stats = host_stats_from_raw(&current, previous, elapsed);
        self.state = Some(HostSampleState { current, sampled_at: now });
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampler_first_tick_then_second() {
        let mut sampler = HostSampler::new();
        let first = sampler.tick();
        assert_eq!(first.cpu_total, 0.0);
        let second = sampler.tick();
        // Second tick has real data (or at least never panics with zeros).
        assert!(second.cpu_total >= 0.0 && second.cpu_total <= 100.0);
    }
}
