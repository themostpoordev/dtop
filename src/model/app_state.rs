use std::collections::HashMap;

use super::{
    BoundedLines, ContainerDetails, ContainerRow, DockerEvent, ImageRow, NetworkRow, VolumeRow,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Home,
    Overview,
    Containers,
    Details,
    Logs,
    Events,
    Images,
    Volumes,
    Networks,
    Settings,
}

impl Screen {
    pub const PRIMARY: [Self; 7] = [
        Self::Overview,
        Self::Containers,
        Self::Events,
        Self::Images,
        Self::Volumes,
        Self::Networks,
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
            Self::Settings => "Settings",
        }
    }

    pub fn next_primary(self) -> Self {
        let current = Self::PRIMARY.iter().position(|screen| *screen == self).unwrap_or(0);
        Self::PRIMARY[(current + 1) % Self::PRIMARY.len()]
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
        }
    }
}
