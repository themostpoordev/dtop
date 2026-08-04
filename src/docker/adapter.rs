use std::collections::HashMap;

use bollard::container::LogOutput;
use bollard::models::{
    ContainerInspectResponse, ContainerStatsResponse, ContainerSummary, EventMessage, ImageSummary,
    Network, Volume,
};

use crate::{
    docker::{metrics_from_raw, RawStats},
    model::{
        ContainerDetails, ContainerMeta, ContainerRow, DockerEvent, ImageRow, LogLine, LogStream,
        Metrics, NetworkRow, VolumeRow,
    },
};

pub fn container_row(
    summary: &ContainerSummary,
    stats: Option<(&RawStats, Option<&RawStats>, f64)>,
    meta: Option<&ContainerMeta>,
) -> ContainerRow {
    let id = summary.id.clone().unwrap_or_default();
    let name = summary
        .names
        .as_ref()
        .and_then(|names| names.first())
        .map(|name| name.trim_start_matches('/').to_owned())
        .unwrap_or_else(|| id.chars().take(12).collect());
    let state = summary
        .state
        .as_ref()
        .map(|s| format!("{s:?}").to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".to_owned());
    let mut row = ContainerRow {
        id,
        name,
        image: summary.image.clone().unwrap_or_else(|| "unknown".into()),
        state,
        status: summary.status.clone().unwrap_or_default(),
        health: meta.map(|m| m.health.clone()).unwrap_or_else(|| "none".into()),
        created: summary.created.unwrap_or_default(),
        started: meta.map(|m| m.started).unwrap_or(0),
        restart_count: meta.map(|m| m.restart_count).unwrap_or(0),
        ports: format_ports(summary),
        metrics: Metrics::default(),
        delta: Default::default(),
    };
    if let Some((current, previous, elapsed)) = stats {
        let (metrics, delta) = metrics_from_raw(current, previous, elapsed);
        row.metrics = metrics;
        row.delta = delta;
    }
    row
}

pub fn container_meta(inspect: &ContainerInspectResponse) -> ContainerMeta {
    ContainerMeta {
        started: inspect
            .state
            .as_ref()
            .and_then(|state| state.started_at.as_ref())
            .and_then(|value| parse_timestamp(value))
            .unwrap_or(0),
        restart_count: inspect.restart_count.unwrap_or_default(),
        health: inspect
            .state
            .as_ref()
            .and_then(|state| state.health.as_ref())
            .and_then(|health| health.status.as_ref())
            .map(|value| format!("{value:?}").to_ascii_lowercase())
            .unwrap_or_else(|| "none".into()),
    }
}

pub fn raw_stats(stats: &ContainerStatsResponse) -> RawStats {
    let cpu = stats.cpu_stats.as_ref();
    let memory = stats.memory_stats.as_ref();
    let networks = stats
        .networks
        .as_ref()
        .map(|value| {
            value.values().fold((0, 0), |(rx, tx), network| {
                (rx + network.rx_bytes.unwrap_or(0), tx + network.tx_bytes.unwrap_or(0))
            })
        })
        .unwrap_or_default();
    let block = stats
        .blkio_stats
        .as_ref()
        .and_then(|blk| blk.io_service_bytes_recursive.as_ref())
        .map(|entries| {
            entries.iter().fold((0, 0), |(read, write), entry| {
                match entry.op.as_deref().unwrap_or_default().to_ascii_lowercase().as_str() {
                    "read" => (read + entry.value.unwrap_or(0), write),
                    "write" => (read, write + entry.value.unwrap_or(0)),
                    _ => (read, write),
                }
            })
        })
        .unwrap_or_default();
    RawStats {
        cpu_total: cpu
            .and_then(|value| value.cpu_usage.as_ref())
            .and_then(|usage| usage.total_usage)
            .unwrap_or(0),
        system_cpu: cpu.and_then(|value| value.system_cpu_usage).unwrap_or(0),
        online_cpus: cpu.and_then(|value| value.online_cpus).unwrap_or(1) as u64,
        memory: memory.and_then(|value| value.usage).unwrap_or(0),
        memory_limit: memory.and_then(|value| value.limit).unwrap_or(0),
        network_rx: networks.0,
        network_tx: networks.1,
        block_read: block.0,
        block_write: block.1,
        pids: stats.pids_stats.as_ref().and_then(|value| value.current).unwrap_or(0),
    }
}

pub fn details(value: ContainerInspectResponse) -> ContainerDetails {
    let state = value.state.as_ref();
    let config = value.config.as_ref();
    let network = value.network_settings.as_ref();
    let ports = config
        .and(network)
        .and_then(|settings| settings.ports.as_ref())
        .map(|ports| {
            ports
                .iter()
                .flat_map(|(private, bindings)| {
                    bindings
                        .as_ref()
                        .map(move |items| {
                            items.iter().map(move |item| {
                                format!(
                                    "{} -> {}:{}",
                                    private,
                                    item.host_ip.clone().unwrap_or_default(),
                                    item.host_port.clone().unwrap_or_default()
                                )
                            })
                        })
                        .into_iter()
                        .flatten()
                })
                .collect()
        })
        .unwrap_or_default();
    ContainerDetails {
        id: value.id.unwrap_or_default(),
        name: value.name.unwrap_or_default().trim_start_matches('/').into(),
        image: config.and_then(|c| c.image.clone()).unwrap_or_default(),
        command: config.and_then(|c| c.cmd.clone()).map(|cmd| cmd.join(" ")).unwrap_or_default(),
        created: value.created.unwrap_or_default(),
        started: state.and_then(|s| s.started_at.clone()).unwrap_or_default(),
        status: state.and_then(|s| s.status.as_ref()).map(|v| format!("{v:?}")).unwrap_or_default(),
        health: state
            .and_then(|s| s.health.as_ref())
            .and_then(|h| h.status.as_ref())
            .map(|v| format!("{v:?}"))
            .unwrap_or_else(|| "none".into()),
        restart_count: value.restart_count.unwrap_or_default(),
        ports,
        mounts: value
            .mounts
            .unwrap_or_default()
            .into_iter()
            .map(|mount| {
                format!(
                    "{} -> {}",
                    mount.source.unwrap_or_default(),
                    mount.destination.unwrap_or_default()
                )
            })
            .collect(),
        networks: network
            .and_then(|n| n.networks.as_ref())
            .map(|n| n.keys().cloned().collect())
            .unwrap_or_default(),
    }
}

pub fn image(value: ImageSummary) -> ImageRow {
    ImageRow {
        id: value.id,
        tags: value.repo_tags,
        size_bytes: value.size.max(0) as u64,
        created: value.created,
    }
}
pub fn volume(value: Volume) -> VolumeRow {
    VolumeRow {
        name: value.name,
        driver: value.driver,
        mountpoint: value.mountpoint,
        scope: value.scope.map(|scope| scope.to_string()).unwrap_or_default(),
    }
}
pub fn network(value: Network) -> NetworkRow {
    NetworkRow {
        id: value.id.unwrap_or_default(),
        name: value.name.unwrap_or_default(),
        driver: value.driver.unwrap_or_default(),
        scope: value.scope.unwrap_or_default(),
        containers: 0,
    }
}
pub fn event(value: EventMessage) -> DockerEvent {
    let actor = value.actor.as_ref();
    DockerEvent {
        timestamp: value.time.unwrap_or(0),
        kind: value.typ.map(|v| format!("{v:?}")).unwrap_or_default(),
        action: value.action.unwrap_or_default(),
        actor: actor.and_then(|a| a.id.clone()).unwrap_or_default(),
        attributes: actor
            .and_then(|a| a.attributes.clone())
            .map(|a| format_attributes(&a))
            .unwrap_or_default(),
    }
}

const MAX_LOG_LINE_CHARS: usize = 4096;

pub fn log_output(value: LogOutput) -> Vec<LogLine> {
    let (stream, bytes) = match value {
        LogOutput::StdErr { message } => (LogStream::Stderr, message),
        LogOutput::StdOut { message } => (LogStream::Stdout, message),
        other => (LogStream::Console, other.into_bytes()),
    };
    if bytes.is_empty() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    text.split('\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let text: String = line.chars().take(MAX_LOG_LINE_CHARS).collect();
            LogLine { stream: stream.clone(), text }
        })
        .collect()
}

fn format_ports(summary: &ContainerSummary) -> String {
    summary
        .ports
        .as_ref()
        .map(|ports| {
            ports
                .iter()
                .map(|port| {
                    let private = port.private_port;
                    let kind = port
                        .typ
                        .as_ref()
                        .map(|v| format!("{v:?}").to_ascii_lowercase())
                        .unwrap_or_default();
                    format!(
                        "{}:{}/{}",
                        port.public_port.map(|v| v.to_string()).unwrap_or_else(|| "-".into()),
                        private,
                        kind
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}
fn parse_timestamp(value: &str) -> Option<i64> {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::parse(value, &Rfc3339).ok().map(|parsed| parsed.unix_timestamp().max(0))
}
fn format_attributes(attributes: &HashMap<String, String>) -> String {
    attributes
        .iter()
        .take(3)
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc3339_timestamp() {
        assert_eq!(parse_timestamp("2020-01-01T00:00:00Z"), Some(1_577_836_800));
        assert_eq!(parse_timestamp("2020-01-01T00:00:00.123456789Z"), Some(1_577_836_800));
    }

    #[test]
    fn parses_unknown_timestamp_without_panicking() {
        assert_eq!(parse_timestamp("not-a-time"), None);
    }

    #[test]
    fn metrics_are_safe_for_missing_stats() {
        let row = container_row(&ContainerSummary::default(), None, None);
        assert_eq!(row.metrics.cpu_percent, 0.0);
    }

    #[test]
    fn log_output_splits_lines_and_caps_length() {
        let mut output = log_output(LogOutput::StdOut { message: "hello\r\nworld\n".into() });
        assert_eq!(output.len(), 2);
        assert_eq!(output.remove(0).text, "hello");
        assert_eq!(output.remove(0).text, "world");
    }
}
