# dtop

A fast, local-first terminal monitor for Docker. dtop brings the feel of btop to
your containers: live CPU, memory, network and block-I/O statistics, logs,
events, images, volumes and networks, with a clean keyboard-driven interface.

dtop is designed for both small personal hosts and large production-like
servers. It talks **only** to the local Docker Engine through a Unix socket,
never to a registry or the internet, and contains **no telemetry**.

## Features

- Overview dashboard with running/stopped/paused counts and per-container CPU
- Container table with live CPU, memory, network, block I/O, uptime, restart
  count and health
- Searchable and sortable containers (name, CPU, memory, uptime, status)
- Container details from Docker inspection (secrets are never shown)
- Live logs with stdout/stderr markers, bounded scrollback, follow and clear
- Real-time Docker events with a bounded buffer
- Images, volumes and networks screens
- Safe container actions only: start, stop, restart, pause, unpause
  (stop/restart/pause require confirmation)
- Switchable themes, configurable refresh rate, density and hint settings
- Graceful handling of missing daemon, permission errors and empty states
- Local-only: one Unix socket, no telemetry, no remote Docker

## Requirements

- Linux (or macOS) with Docker Engine
- A user with access to `/var/run/docker.sock` (usually the `docker` group)
- Rust 1.88+ to build from source

## Build and install

```sh
cargo build --release
# binary at target/release/dtop
```

## Run

```sh
dtop
```

By default dtop connects to `/var/run/docker.sock`. Override the socket with
`--socket`, or use a custom config file with `--config`.

```sh
dtop --socket /run/user/1000/docker.sock
dtop --config ~/.config/dtop/config.toml
```

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` | Switch main section |
| `Esc` | Return to Home |
| `↑ ↓` | Move selection |
| `Enter` | Open selection / confirm |
| `/` | Filter current list (containers, events, logs) |
| `d` | Open container details |
| `l` | Open container logs |
| `s` / `x` / `r` / `p` / `u` | start / stop / restart / pause / unpause |
| `Space` | Toggle log follow |
| `c` / `C` | Clear logs / events |
| `?` or `F1` | Keybinding help |
| `q` or `Ctrl-C` | Quit |

## Configuration

dtop reads `$XDG_CONFIG_HOME/dtop/config.toml` (default
`~/.config/dtop/config.toml`). Settings can be edited in-app under the
**Settings** tab and are saved on exit. See `config/dtop.example.toml` for a
complete example.

```toml
docker_socket = "/var/run/docker.sock"
refresh_ms = 1000
theme = "default"
sort = "cpu"
show_stopped = true
follow_logs = true
density = "comfortable"
show_hints = true
```

- `refresh_ms` controls how often container stats are sampled. The minimum is
  **50 ms**. Lower values feel more live but cost more CPU; choose a higher
  value on large hosts.
- `theme`: `default`, `midnight`, `amber`, `mono`
- `sort`: `cpu`, `memory`, `uptime`, `name`, `status`
- `show_stopped`: include exited/created containers in the list
- `follow_logs`: automatically follow the latest log lines
- `density`: `comfortable` or `compact`
- `show_hints`: show keybinding hints in the footer

## Safety

dtop only performs safe container actions: start, stop, restart, pause and
unpause. Stopping, restarting and pausing require an explicit confirmation.
There are intentionally no delete, prune, exec or registry operations.

Container environment values are never displayed, to avoid leaking secrets.

## Performance

- Fast stats path samples a bounded, rotating subset of running containers per
  refresh; every container is sampled regularly regardless of how many exist.
- Images, volumes and networks are refreshed on a slower cadence, so a 50 ms
  refresh interval does not trigger expensive full inventory scans.
- Logs and events use bounded ring buffers, so memory stays flat even for
  noisy containers.
- Concurrent API calls are bounded and non-blocking; the UI never blocks on
  the Docker daemon.

## Troubleshooting

- **Permission denied**: the current user cannot access the Docker socket.
  Ensure the user is in the `docker` group or run with an accessible socket.
- **Daemon unavailable**: confirm the Docker Engine is running and that the
  socket path in Settings (or `--socket`) exists.
- **No containers listed**: an empty daemon is expected and shown as an empty
  state; enable `show_stopped` to see exited containers.

## License

MIT
