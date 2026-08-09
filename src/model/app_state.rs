use std::collections::HashMap;

use crate::docker::HostMemory;

use super::{
    BoundedLines, ContainerDetails, ContainerRow, DockerEvent, HostHistory, HostStats, ImageRow,
    NetworkRow, VolumeRow,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Home,
    // Docker mode
    Overview,
    Containers,
    Details,
    Logs,
    Events,
    Images,
    Volumes,
    Networks,
    // All mode
    System,
    Cpu,
    Memory,
    Disk,
    Network,
    Processes,
    // Shared
    Settings,
}

impl Screen {
    /// Tab cycle in docker mode.
    pub const PRIMARY_DOCKER: [Self; 7] = [
        Self::Overview,
        Self::Containers,
        Self::Events,
        Self::Images,
        Self::Volumes,
        Self::Networks,
        Self::Settings,
    ];

    /// Tab cycle in all mode — btop-style host screens plus Settings.
    pub const PRIMARY_ALL: [Self; 7] = [
        Self::System,
        Self::Cpu,
        Self::Memory,
        Self::Disk,
        Self::Network,
        Self::Processes,
        Self::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Overview => "Overview",
            Self::Containers => "Containers",
            Self::Details => "Details",
            Self::Logs => "Logs",
            Self::Events => "Events",
            Self::Images => "Images",
            Self::Volumes => "Volumes",
            Self::Networks => "Networks",
            Self::System => "System",
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Disk => "Disk",
            Self::Network => "Network",
            Self::Processes => "Processes",
            Self::Settings => "Settings",
        }
    }

    pub fn primary(mode: crate::config::Mode) -> [Self; 7] {
        match mode {
            crate::config::Mode::Docker => Self::PRIMARY_DOCKER,
            crate::config::Mode::All => Self::PRIMARY_ALL,
        }
    }

    pub fn next_primary(self, mode: crate::config::Mode) -> Self {
        let primary = Self::primary(mode);
        let current = primary.iter().position(|screen| *screen == self).unwrap_or(0);
        primary[(current + 1) % primary.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConnectionState {
    #[default]
    Connecting,
    Connected,
    Unavailable,
    PermissionDenied,
    Error,
}

impl ConnectionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Unavailable => "daemon unavailable",
            Self::PermissionDenied => "permission denied",
            Self::Error => "error",
        }
    }
}

#[derive(Debug)]
pub struct AppData {
    pub containers: Vec<ContainerRow>,
    pub details: Option<ContainerDetails>,
    pub images: Vec<ImageRow>,
    pub volumes: Vec<VolumeRow>,
    pub networks: Vec<NetworkRow>,
    pub events: BoundedLines<DockerEvent>,
    pub logs: BoundedLines<super::LogLine>,
    pub container_metrics: HashMap<String, super::Metrics>,
    pub host_memory: HostMemory,
    pub gpu: crate::docker::GpuInfo,
    pub history: super::History,
    /// Host-wide metrics for the "all" mode. Fresh every 500 ms, even when the
    /// Docker daemon is unreachable.
    pub host: HostStats,
    pub host_history: HostHistory,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            containers: Vec::new(),
            details: None,
            images: Vec::new(),
            volumes: Vec::new(),
            networks: Vec::new(),
            events: BoundedLines::new(200),
            logs: BoundedLines::new(5000),
            container_metrics: HashMap::new(),
            host_memory: HostMemory::default(),
            gpu: crate::docker::GpuInfo::none(),
            history: super::History::new(),
            host: HostStats::default(),
            host_history: HostHistory::default(),
        }
    }
}
