# Sentinel

Sentinel is a Rust/Tauri operator dashboard for Linux hosts: services, ports, thermal state, and core host health.

## Current scaffold

This repo now contains the first local-only MVP shell:

- **Tauri 2** desktop shell
- **Plain Vite frontend** with bordered operator panels
- **Rust collectors** for:
  - listening sockets via `ss -H -ltnup`
  - memory via `/proc/meminfo`
  - CPU usage via sampled `/proc/stat`
  - uptime via `/proc/uptime`
  - load averages via `/proc/loadavg`
  - temperatures via `/sys/class/thermal`

The current UI shows:

- host summary cards
- thermal readings
- listener table with:
  - systemd unit mapping
  - exposure severity badges (`ok` / `warning` / `danger`)
  - standard vs nonstandard port classification
  - client-side filters
- listener detail pane with:
  - resolved unit + scope
  - unit state
  - `systemctl status` output
  - cgroup path
  - command line
  - recent journal lines
- collector warnings

## Architecture

### Frontend

- `index.html`
- `src/main.js`
- `src/styles.css`
- `vite.config.js`

The frontend is intentionally framework-light. No React/Svelte yet.

### Native shell

- `src-tauri/src/main.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tauri.conf.json`

The Rust side currently exposes two commands:

- `snapshot` → returns a single host snapshot payload for the dashboard
- `service_details` → returns command line, resolved unit, `systemctl status`, and recent journal output for a selected PID

## Local development

```bash
npm install --include=dev
npm run tauri dev
```

## Verification

```bash
npm install --include=dev
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
npm run tauri info
```

## Next steps

1. Add per-service health checks.
2. Add service actions: restart/stop/start.
3. Add Prometheus integration for historical graphs.
4. Add Grafana-backed metrics panes and alert plumbing.
5. Add event history/diffing for listener changes.

## Notes

- Listener collection depends on `ss` being available.
- Temperatures are best effort; some hosts expose fewer thermal zones.
- This MVP is Linux-first and intentionally local-host scoped.
- `npm run tauri dev` expects port `11500` to be free because Vite is configured with `strictPort: true`.
