# Sentinel

Sentinel is a Linux-first operator dashboard for watching the services, ports, and host health signals that matter on a local machine.

The project is built backend-first: Rust collectors prove the data path first, then the UI renders those verified results.

## What it does

Sentinel currently reports:

- watched service health by named port
- grouped port/service views: `All`, `Web`, `Infra`, `Hermes`, `Game`
- listener discovery from `ss -H -ltnup`
- process name and PID for discovered listeners
- TCP/UDP health checks with latency
- host summary metrics:
  - CPU usage
  - memory usage
  - uptime
  - load averages
  - listener count
- thermal readings from `/sys/class/thermal`
- systemd service detail lookup for selected listener PIDs
- recent journal/status context where systemd can resolve the unit

## Interfaces

Sentinel has two runnable interfaces.

### 1. Headless terminal dashboard

This is the current preferred path for proving backend/service monitoring without browser or Tauri window issues.

```bash
cd src-tauri
cargo run --bin sentinel-tui
```

Controls:

```text
q / Esc              quit
r                    refresh
j / ↓                move down
k / ↑                move up
Tab / → / l          next service group
Shift+Tab / ← / h    previous service group
```

### 2. Tauri/Vite desktop shell

```bash
npm install --include=dev
npm run tauri dev
```

Vite is configured to bind to port `11500` with `strictPort: true`.

For a browser-only dev server:

```bash
npm run dev -- --host 0.0.0.0
```

## Custom watchlist

Override the default watched ports with `SENTINEL_WATCH_PORTS`.

Format:

```text
name:protocol:address:port[:expected]
```

- `protocol`: `tcp` or `udp`
- `expected`: optional; accepts `true`, `yes`, `up`, `expected`, `1`, `false`, `no`, `down`, `optional`, `0`

Example:

```bash
cd src-tauri
SENTINEL_WATCH_PORTS='api:tcp:127.0.0.1:8000,redis:tcp:127.0.0.1:6379:false' \
  cargo run --bin sentinel-tui
```

Custom entries currently appear under the `custom` category in backend data.

## Architecture

```text
src-tauri/src/
  lib.rs                 Tauri command bridge and backend entrypoints
  health.rs              TCP/UDP service health checks
  listeners.rs           listening socket collection and port classification
  metrics.rs             CPU, memory, uptime, and load collectors
  services.rs            process, cgroup, systemd, and journal lookup
  thermals.rs            thermal zone collection
  watchlist.rs           named service watchlist and env override parser
  models.rs              serialized dashboard data models
  bin/sentinel-tui.rs    Ratatui terminal dashboard

src/
  main.js                browser/Tauri frontend behavior
  styles.css             dashboard styling

vite.config.js           Vite dev-server/build config
src-tauri/Cargo.toml     Rust crate and binary config
```

Backend commands exposed to the Tauri frontend:

- `snapshot` — host summary, thermals, listeners, warnings
- `service_details` — command line, cgroup, systemd status, recent logs
- `service_health` — direct health probe for a listener
- `watched_services` — named watchlist status payload

## Development

Install JavaScript dependencies:

```bash
npm install --include=dev
```

Run the TUI:

```bash
cd src-tauri
cargo run --bin sentinel-tui
```

Run the Tauri app:

```bash
npm run tauri dev
```

Build frontend assets:

```bash
npm run build
```

Build the Rust TUI binary:

```bash
cargo build --manifest-path src-tauri/Cargo.toml --bin sentinel-tui
```

## Verification

Run before committing:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check --manifest-path src-tauri/Cargo.toml --bin sentinel-tui
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
npm run build
```

## Requirements

- Linux
- Rust stable toolchain
- Node.js/npm
- `ss` from `iproute2`
- systemd/journald for full service detail enrichment

Notes:

- Listener collection depends on `ss`.
- Thermal data is best-effort; not every host exposes the same thermal zones.
- Systemd unit mapping is best-effort and may be unavailable for some PIDs or containers.
- Sentinel is local-host scoped right now; it is not a remote monitoring agent yet.

## Status

MVP scaffold. Backend collectors and the terminal dashboard are the most reliable path today. The web/Tauri UI exists, but active iteration should keep proving functionality in Rust first.

## License

MIT. See `LICENSE`.
