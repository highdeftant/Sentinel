use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
};
use sentinel_lib::watched_services_snapshot;

const COLOR_BG: Color = Color::Rgb(24, 28, 34);
const COLOR_SURFACE: Color = Color::Rgb(31, 37, 46);
const COLOR_BORDER: Color = Color::Rgb(72, 82, 96);
const COLOR_TEXT: Color = Color::Rgb(214, 221, 230);
const COLOR_MUTED: Color = Color::Rgb(148, 163, 184);
const COLOR_OK: Color = Color::Rgb(134, 239, 172);
const COLOR_WARNING: Color = Color::Rgb(252, 211, 77);
const COLOR_DANGER: Color = Color::Rgb(248, 113, 113);
const COLOR_ACCENT: Color = Color::Rgb(147, 197, 253);

const CATEGORIES: [(&str, &str); 5] = [
    ("all", "All"),
    ("web", "Web"),
    ("infra", "Infra"),
    ("hermes", "Hermes"),
    ("game", "Game"),
];

struct AppState {
    services: Vec<ServiceRow>,
    warnings: Vec<String>,
    selected: usize,
    active_category: usize,
    should_quit: bool,
}

struct ServiceRow {
    name: String,
    category: String,
    port: String,
    endpoint: String,
    status: String,
    probe: String,
    latency: String,
    listener: String,
    process: String,
    pid: String,
    message: String,
}

impl AppState {
    fn new() -> Self {
        let mut state = Self {
            services: Vec::new(),
            warnings: Vec::new(),
            selected: 0,
            active_category: 0,
            should_quit: false,
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        let snapshot = watched_services_snapshot();
        self.services = snapshot
            .services
            .into_iter()
            .map(|service| {
                let process = service
                    .listener
                    .as_ref()
                    .and_then(|listener| listener.process.clone())
                    .unwrap_or_else(|| "—".to_string());
                let pid = service
                    .listener
                    .as_ref()
                    .and_then(|listener| listener.pid)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "—".to_string());
                let listener = service
                    .listener
                    .as_ref()
                    .map(|listener| format!("{}:{}", listener.local_address, listener.port))
                    .unwrap_or_else(|| "not-listening".to_string());
                let latency = service
                    .health
                    .latency_ms
                    .map(|value| format!("{value}ms"))
                    .unwrap_or_else(|| "—".to_string());

                ServiceRow {
                    name: service.name,
                    category: service.category,
                    port: service.port.to_string(),
                    endpoint: format!(
                        "{}://{}:{}",
                        service.protocol, service.address, service.port
                    ),
                    status: service.health.status,
                    probe: service.health.check_kind,
                    latency,
                    listener,
                    process,
                    pid,
                    message: service.health.message,
                }
            })
            .collect();
        self.warnings = snapshot.warnings;
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        let visible_count = self.visible_services().len();
        if visible_count == 0 {
            self.selected = 0;
            return;
        }

        if self.selected >= visible_count {
            self.selected = visible_count.saturating_sub(1);
        }
    }

    fn active_category_key(&self) -> &'static str {
        CATEGORIES
            .get(self.active_category)
            .map(|category| category.0)
            .unwrap_or("all")
    }

    fn active_category_label(&self) -> &'static str {
        CATEGORIES
            .get(self.active_category)
            .map(|category| category.1)
            .unwrap_or("All")
    }

    fn visible_services(&self) -> Vec<&ServiceRow> {
        let key = self.active_category_key();
        self.services
            .iter()
            .filter(|service| key == "all" || service.category == key)
            .collect()
    }

    fn selected_detail(&self) -> Vec<Line<'static>> {
        self.services
            .iter()
            .filter(|service| {
                self.active_category_key() == "all"
                    || service.category == self.active_category_key()
            })
            .nth(self.selected)
            .map(|service| {
                vec![
                    Line::from(vec![
                        Span::styled("service: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.name.clone(), Style::default().fg(COLOR_TEXT)),
                    ]),
                    Line::from(vec![
                        Span::styled("port: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.port.clone(), Style::default().fg(COLOR_TEXT)),
                        Span::styled("  endpoint: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.endpoint.clone(), Style::default().fg(COLOR_TEXT)),
                    ]),
                    Line::from(vec![
                        Span::styled("listener: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.listener.clone(), Style::default().fg(COLOR_TEXT)),
                        Span::styled("  process: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.process.clone(), Style::default().fg(COLOR_TEXT)),
                        Span::styled("  pid: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.pid.clone(), Style::default().fg(COLOR_TEXT)),
                    ]),
                    Line::from(vec![
                        Span::styled("probe: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.probe.clone(), Style::default().fg(COLOR_TEXT)),
                        Span::styled("  message: ", Style::default().fg(COLOR_MUTED)),
                        Span::styled(service.message.clone(), Style::default().fg(COLOR_TEXT)),
                    ]),
                ]
            })
            .unwrap_or_else(|| vec![Line::from("no service selected")])
    }

    fn move_down(&mut self) {
        if self.selected + 1 < self.visible_services().len() {
            self.selected += 1;
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn next_category(&mut self) {
        self.active_category = (self.active_category + 1) % CATEGORIES.len();
        self.selected = 0;
        self.clamp_selection();
    }

    fn previous_category(&mut self) {
        self.active_category = self
            .active_category
            .checked_sub(1)
            .unwrap_or(CATEGORIES.len().saturating_sub(1));
        self.selected = 0;
        self.clamp_selection();
    }
}

fn main() -> Result<(), String> {
    let mut stdout = io::stdout();
    enable_raw_mode().map_err(|error| format!("enable raw mode failed: {error}"))?;
    execute!(stdout, EnterAlternateScreen)
        .map_err(|error| format!("enter alternate screen failed: {error}"))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|error| format!("terminal init failed: {error}"))?;

    let run_result = run_app(&mut terminal);
    let cleanup_result = cleanup_terminal(&mut terminal);

    match (run_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(())) => Err(run_error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(run_error), Err(cleanup_error)) => Err(format!(
            "{run_error}; terminal cleanup failed: {cleanup_error}"
        )),
    }
}

fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
    disable_raw_mode().map_err(|error| format!("disable raw mode failed: {error}"))?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|error| format!("leave alternate screen failed: {error}"))?;
    terminal
        .show_cursor()
        .map_err(|error| format!("show cursor failed: {error}"))
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
    let mut state = AppState::new();

    while !state.should_quit {
        terminal
            .draw(|frame| render(frame, &state))
            .map_err(|error| format!("draw failed: {error}"))?;

        if event::poll(Duration::from_millis(1000))
            .map_err(|error| format!("event poll failed: {error}"))?
        {
            let event = event::read().map_err(|error| format!("event read failed: {error}"))?;
            handle_event(event, &mut state);
        } else {
            state.refresh();
        }
    }

    Ok(())
}

fn handle_event(event: Event, state: &mut AppState) {
    if let Event::Key(key) = event {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
            KeyCode::Char('r') => state.refresh(),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => state.next_category(),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => state.previous_category(),
            KeyCode::Down | KeyCode::Char('j') => state.move_down(),
            KeyCode::Up | KeyCode::Char('k') => state.move_up(),
            _ => {}
        }
    }
}

fn render(frame: &mut Frame<'_>, state: &AppState) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(COLOR_BG)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, chunks[0], state);
    render_tabs(frame, chunks[1], state);
    render_services(frame, chunks[2], state);
    render_detail(frame, chunks[3], state);
    render_footer(frame, chunks[4]);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let visible_services = state.visible_services();
    let ok = visible_services
        .iter()
        .filter(|service| service.status == "ok")
        .count();
    let warning = visible_services
        .iter()
        .filter(|service| service.status == "warning")
        .count();
    let danger = visible_services
        .iter()
        .filter(|service| service.status == "danger")
        .count();

    let line = Line::from(vec![
        Span::styled(
            " SENTINEL ",
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" watched services ", Style::default().fg(COLOR_TEXT)),
        Span::styled(
            format!(" tab:{} ", state.active_category_label()),
            Style::default().fg(COLOR_ACCENT),
        ),
        Span::styled(
            format!(
                " visible:{}/{} ",
                visible_services.len(),
                state.services.len()
            ),
            Style::default().fg(COLOR_MUTED),
        ),
        Span::styled(format!(" ok:{ok} "), Style::default().fg(COLOR_OK)),
        Span::styled(
            format!(" warn:{warning} "),
            Style::default().fg(COLOR_WARNING),
        ),
        Span::styled(
            format!(" down:{danger} "),
            Style::default().fg(COLOR_DANGER),
        ),
    ]);

    frame.render_widget(Paragraph::new(line).block(panel_block("HOST")), area);
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let titles = CATEGORIES.iter().map(|(key, label)| {
        let count = if *key == "all" {
            state.services.len()
        } else {
            state
                .services
                .iter()
                .filter(|service| service.category == *key)
                .count()
        };
        Line::from(format!(" {label} {count} "))
    });

    let tabs = Tabs::new(titles)
        .select(state.active_category)
        .block(panel_block("PORT GROUPS"))
        .style(Style::default().fg(COLOR_MUTED))
        .highlight_style(
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

fn render_services(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let header = Row::new([
        "Service label",
        "Port",
        "Endpoint",
        "Status",
        "Latency",
        "Listener",
        "Process",
    ])
    .style(
        Style::default()
            .fg(COLOR_MUTED)
            .add_modifier(Modifier::BOLD),
    );

    let visible_services = state.visible_services();
    let rows = visible_services.iter().enumerate().map(|(index, service)| {
        let status_style = status_style(&service.status);
        let row_style = if index == state.selected {
            Style::default().bg(COLOR_SURFACE).fg(COLOR_TEXT)
        } else {
            Style::default().fg(COLOR_TEXT)
        };

        Row::new(vec![
            Cell::from(service.name.clone()),
            Cell::from(service.port.clone()),
            Cell::from(service.endpoint.clone()),
            Cell::from(service.status.clone()).style(status_style),
            Cell::from(service.latency.clone()),
            Cell::from(service.listener.clone()),
            Cell::from(service.process.clone()),
        ])
        .style(row_style)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Length(6),
            Constraint::Length(26),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(18),
            Constraint::Min(10),
        ],
    )
    .header(header)
    .block(panel_block("WATCHLIST"))
    .row_highlight_style(Style::default().bg(COLOR_SURFACE));

    frame.render_widget(table, area);
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let mut lines = state.selected_detail();

    if !state.warnings.is_empty() {
        lines.push(Line::from(""));
        lines.extend(state.warnings.iter().map(|warning| {
            Line::from(Span::styled(
                warning.clone(),
                Style::default().fg(COLOR_WARNING),
            ))
        }));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel_block("DETAIL"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new(
            " q/Esc quit  r refresh  Tab/→/l next group  Shift+Tab/←/h prev group  j/k move ",
        )
        .style(Style::default().fg(COLOR_MUTED).bg(COLOR_BG)),
        area,
    );
}

fn panel_block(title: &'static str) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_BORDER))
        .style(Style::default().bg(COLOR_BG).fg(COLOR_TEXT))
}

fn status_style(status: &str) -> Style {
    match status {
        "ok" => Style::default().fg(COLOR_OK).add_modifier(Modifier::BOLD),
        "warning" => Style::default()
            .fg(COLOR_WARNING)
            .add_modifier(Modifier::BOLD),
        "danger" => Style::default()
            .fg(COLOR_DANGER)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(COLOR_MUTED),
    }
}
