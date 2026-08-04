use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default)]
pub struct DockerEvent {
    pub timestamp: i64,
    pub kind: String,
    pub action: String,
    pub actor: String,
    pub attributes: String,
}

impl DockerEvent {
    pub fn time(&self) -> String {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(self.timestamp.max(0) as u64);
        if elapsed < 60 {
            format!("{elapsed}s ago")
        } else if elapsed < 3600 {
            format!("{}m ago", elapsed / 60)
        } else {
            format!("{}h ago", elapsed / 3600)
        }
    }
}

#[derive(Clone, Debug)]
pub enum LogStream {
    Stdout,
    Stderr,
    Console,
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub stream: LogStream,
    pub text: String,
}
