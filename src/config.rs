use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const MIN_REFRESH_MS: u64 = 50;
const DEFAULT_SOCKET: &str = "/var/run/docker.sock";

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    #[default]
    Default,
    Midnight,
    Amber,
    Mono,
}

impl ThemeName {
    pub const ALL: [Self; 4] = [Self::Default, Self::Midnight, Self::Amber, Self::Mono];
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Midnight => "midnight",
            Self::Amber => "amber",
            Self::Mono => "mono",
        }
    }
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Name,
    #[default]
    Cpu,
    Memory,
    Uptime,
    Status,
}
impl SortOrder {
    pub const ALL: [Self; 5] = [Self::Cpu, Self::Memory, Self::Uptime, Self::Name, Self::Status];
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Uptime => "uptime",
            Self::Status => "status",
        }
    }
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}
impl Density {
    pub fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }
    pub fn toggle(self) -> Self {
        match self {
            Self::Comfortable => Self::Compact,
            Self::Compact => Self::Comfortable,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub docker_socket: String,
    pub refresh_ms: u64,
    pub theme: ThemeName,
    pub sort: SortOrder,
    pub show_stopped: bool,
    pub follow_logs: bool,
    pub density: Density,
    pub show_hints: bool,
    #[serde(default = "default_true")]
    pub show_gpu: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            docker_socket: DEFAULT_SOCKET.into(),
            refresh_ms: 2000,
            theme: ThemeName::Default,
            sort: SortOrder::Cpu,
            show_stopped: true,
            follow_logs: true,
            density: Density::Comfortable,
            show_hints: true,
            show_gpu: true,
        }
    }
}

impl Config {
    pub fn validate(mut self) -> Result<Self> {
        if self.docker_socket.trim().is_empty() || !Path::new(&self.docker_socket).is_absolute() {
            anyhow::bail!("docker_socket must be an absolute Unix socket path");
        }
        self.refresh_ms = self.refresh_ms.max(MIN_REFRESH_MS);
        Ok(self)
    }

    pub fn path_from_arg(path: Option<PathBuf>) -> Result<PathBuf> {
        if let Some(path) = path {
            return Ok(path);
        }
        if let Some(base) = env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(base).join("dtop/config.toml"));
        }
        let home = env::var_os("HOME").context("HOME is not set; pass --config explicitly")?;
        Ok(PathBuf::from(home).join(".config/dtop/config.toml"))
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        toml::from_str::<Self>(&raw)
            .with_context(|| format!("parse config {}", path.display()))?
            .validate()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let config = self.clone().validate()?;
        let contents = toml::to_string_pretty(&config).context("serialize config")?;
        let parent = path.parent().context("config path has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("create config directory {}", parent.display()))?;
        let temp = parent.join(format!(".dtop-config-{}.tmp", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .with_context(|| format!("create temporary config {}", temp.display()))?;
        let result = (|| -> Result<()> {
            file.write_all(contents.as_bytes()).context("write config")?;
            file.sync_all().context("flush config")?;
            fs::rename(&temp, path).context("replace config")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}
