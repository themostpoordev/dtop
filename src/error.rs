use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DtopError {
    #[error("configuration file is invalid: {0}")]
    InvalidConfig(String),
    #[error("Docker socket path is invalid: {0}")]
    InvalidSocket(PathBuf),
    #[error("Docker permission denied for {0}")]
    PermissionDenied(String),
    #[error("Docker daemon is unavailable at {0}")]
    DaemonUnavailable(String),
    #[error("Docker API error: {0}")]
    DockerApi(String),
}
