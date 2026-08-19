# dtop

A fast, local-first terminal monitor for Docker **and your host**. Watch
containers the way `docker stats` always wanted to — logs, events, images,
volumes and networks in one place — or flip to **all mode** for a btop-style
system monitor with CPU, memory, disk, network and process graphs.

Built in Rust with ratatui + bollard. Talks to Docker over the Unix socket
only — no registry calls, no telemetry, no remote hosts, no cloud.

![rust](https://img.shields.io/badge/rust-1.88+-orange.svg) ![license](https://img.shields.io/badge/license-MIT-blue.svg)

## Features

- **Two modes** — `docker` (containers & resources) and `all` (host system monitor)
- **Gradient graphs** — every history graph renders as 8-level gradient lines
  (`▁▂▃▄▅▆▇█`) with lerped colors; smooth, no hard joints, one cell per sample
- **Dense UI** — no wasted rows; every panel shows live data
- **Daemon-independent** — `all` mode reads `/proc` directly and keeps working
  even when the Docker daemon is down
- **Fast & light** — fast tick reads 5 small `/proc` files in ~112 µs; process
  scan runs on a slower 2 s cadence; zero new dependencies, zero unsafe code
- **Safe actions only** — start, stop, restart, pause, unpause with
  confirmation. No delete, no prune, no exec

## Modes

| Mode | Screens |
|------|---------|
| **docker** (default) | Overview · Containers · Events · Images · Volumes · Networks · Settings |
| **all** | System · CPU · Memory · Disk · Network · Processes · Settings |

Switch modes in **Settings** (first row) or set `mode = "all"` in the config.

### docker mode

- **Overview** — total CPU with gradient history, per-service CPU/memory, host
  memory split into RAM / zram / swapfile
- **Containers** — live CPU, memory, network and block I/O, uptime, restarts
  and health, with search (`/`) and sortable columns
- **Details** — inspect view (ports, mounts, networks, command). Environment
  values are intentionally not shown
- **Logs** — stdout/stderr with follow mode and bounded scrollback
- **Events** — the daemon's event stream, filterable
- **Images / Volumes / Networks** — plain lists with empty states

### all mode

- **System** — CPU total + load average + gradient history, memory bars
  (RAM/zram/swapfile), disk/net rate totals
- **CPU** — per-core bars, gradient history, and a top-by-CPU process list
- **Memory** — RAM / zram / swapfile bars plus the top RSS processes
- **Disk** — read/write gradient history plus per-disk cumulative bytes and rates
- **Network** — rx/tx gradient history plus per-interface rates
- **Processes** — top 64 processes, sortable (cpu / memory / name), `/` to search

Stats are sampled every 500 ms. That interval is fixed; it is not a setting.

## Install

### Binary (Linux x86_64)

Grab the latest release from
[GitHub Releases](https://github.com/themostpoordev/dtop/releases):

```sh
tar xzf dtop-v1.0.0.tar.gz
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

## Architecture

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

### Performance notes

- The host sampler runs on its **own task**, independent of the Docker select
  loop — it fires every 500 ms even when the daemon is down
- The fast tick reads only 5 small `/proc` files (~112 µs); the process scan
  (the expensive part, ~6 ms) runs every 2 s
- The UI polls the event channel on a fixed cadence instead of blocking on
  `recv()`, so snapshots can never be starved by a busy key stream
- Everything is bounded: 64 processes, 120-sample history, drop-on-full events

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
