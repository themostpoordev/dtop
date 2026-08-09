# dtop

A terminal monitor for the local Docker daemon. Shows your containers the way
`docker stats` always wanted to, with logs, events, images, volumes and
networks in one place.

Built in Rust with ratatui + bollard. It talks to Docker over the Unix socket
only — no registry calls, no telemetry, no remote hosts.

## Modes

- **docker** (default) — everything Docker: containers, images, volumes,
  networks, events
- **all** — btop-style host monitor: system overview, per-core CPU, memory
  (RAM/zram/swapfile), disk I/O, network interfaces and a top-process list.
  Host metrics are read straight from `/proc` and keep working even when the
  Docker daemon is down. Switch modes in Settings (or `mode = "all"` in the
  config).

## What it does

### docker mode

- **Overview** — total CPU with a scrolling history graph, per-service CPU and
  memory, and host memory split into RAM / zram / swapfile
- **Containers** — live CPU, memory, network and block I/O, uptime, restart
  count and health, with search (`/`) and sortable columns
- **Details** — inspect view of the selected container (ports, mounts,
  networks, command). Environment values are intentionally not shown
- **Logs** — stdout/stderr with follow mode and a bounded scrollback
- **Events** — the daemon's event stream, filtered by container
- **Images, volumes, networks** — plain lists with an empty state
- **Safe actions only** — start, stop, restart, pause, unpause. Stop/restart/
  pause ask for confirmation first. There is no delete, no prune, no exec

### all mode

- **System** — host CPU total + load average + history graph, memory summary,
  disk/net rate totals
- **CPU** — per-core utilization bars and a 60-second history sparkline
- **Memory** — RAM / zram / swapfile bars plus the top RSS processes
- **Disk** — per physical disk cumulative bytes and read/write rates
- **Network** — per interface rx/tx and live rate sparklines
- **Processes** — top 64 processes by CPU (sortable by cpu/memory/name, `/`
  to search)

Stats are sampled every 500 ms. That interval is fixed; it is not a setting.

## Install

### Binary (Linux x86_64)

Grab the latest release from
[GitHub Releases](https://github.com/themostpoordev/dtop/releases):

```sh
tar xzf dtop-v*.tar.gz
sudo install -m 0755 dtop /usr/local/bin/dtop
dtop
```

### From source

Requires Rust 1.88+.

```sh
cargo build --release
# binary at target/release/dtop
```

### cargo install

```sh
cargo install --git https://github.com/themostpoordev/dtop
```

## Usage

```sh
dtop                       # default socket: /var/run/docker.sock
dtop --socket /run/user/1000/docker.sock
dtop --config ~/.config/dtop/config.toml
```

You need access to the Docker socket — usually the `docker` group. If the
daemon is missing or the socket is not readable, dtop says so and keeps
running; it does not crash.

| Key | Action |
|-----|--------|
| `Tab` | next section |
| `Esc` | back to Home |
| `↑` / `↓` | move selection (container list scrolls with you) |
| `Enter` | open / confirm |
| `/` | filter current list |
| `d` | details |
| `l` | logs |
| `s` `x` `r` `p` `u` | start / stop / restart / pause / unpause |
| `Space` | toggle log follow |
| `c` / `C` | clear logs / events |
| `q` | quit |

## Configuration

`~/.config/dtop/config.toml` (or `$XDG_CONFIG_HOME/dtop/config.toml`). Settings
changed in the app are saved automatically. See `config/dtop.example.toml`.

```toml
docker_socket = "/var/run/docker.sock"
mode = "docker"            # docker | all
theme = "default"          # default | midnight | amber | mono
sort = "cpu"               # cpu | memory | uptime | name | status
show_stopped = true
follow_logs = true
density = "comfortable"    # comfortable | compact
show_hints = true
show_gpu = true            # GPU panel only appears when a GPU is detected
```

## Layout

```
src/
├── main.rs        # CLI, terminal lifecycle, event loop
├── app.rs         # UI state, navigation, filtering, actions
├── config.rs      # TOML config, validation, atomic save
├── docker/        # Docker client, adapter, stats, host memory, GPU probe
├── host/          # /proc readers, delta math, host sampler (daemon-free)
├── model/         # display models, bounded buffers, history
├── runtime/       # supervisor: polling, log/event streams, reconnect
├── ui/            # one module per screen + theme
└── terminal.rs    # raw mode / alternate screen guard
```

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
cargo test --all-targets
```

CI runs all four on every push and pull request.

## Why no refresh setting

A polling interval that can be lowered to "feels faster" just moves the
refresh cost onto the daemon. 500 ms keeps the UI responsive without turning
the socket into a hot loop, and the history graph gives you the recent trend
without needing a faster tick.

## License

MIT
