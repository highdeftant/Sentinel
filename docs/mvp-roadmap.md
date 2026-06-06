# Sentinel MVP Roadmap

## Phase 1 — Local host inventory

- [x] Tauri shell scaffold
- [x] Vite frontend scaffold
- [x] Local snapshot command
- [x] Host summary cards
- [x] Thermal cards
- [x] Listener table
- [x] Warning surface
- [x] Unit tests for proc parsers
- [x] systemd unit mapping
- [x] service detail pane
- [x] listener filters/search

## Phase 2 — Operator actions

- [x] `systemctl status` integration
- [ ] restart/stop/start actions
- [x] journal tail panel
- [ ] health checks per service
- [x] exposure severity badges

## Phase 3 — Metrics backend

- [ ] Prometheus setup docs
- [ ] node_exporter integration
- [ ] custom Sentinel exporter or embedded metrics endpoint
- [ ] Prometheus query-backed trend widgets
- [ ] Grafana dashboard JSON

## Phase 4 — Multi-host

- [ ] remote host registration model
- [ ] secure transport/auth
- [ ] host switcher UI
- [ ] per-host alert state
