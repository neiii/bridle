use std::io::{self, Stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use harness_locate::{Harness, HarnessKind, InstallationStatus};

use crate::harness::HarnessConfig;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::config::{BridleConfig, ProfileInfo, ProfileManager, ProfileName};
use crate::error::Error;

type Tui = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Harnesses,
    Profiles,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    #[default]
    Normal,
    CreatingProfile,
}

#[derive(Debug)]
struct App {
    running: bool,
    active_pane: Pane,
    harnesses: Vec<HarnessKind>,
    harness_state: ListState,
    profiles: Vec<ProfileInfo>,
    profile_state: ListState,
    expanded_profile: Option<usize>,
    status_message: Option<String>,
    bridle_config: BridleConfig,
    manager: ProfileManager,
    show_help: bool,
    input_mode: InputMode,
    input_buffer: String,
}

impl App {
    fn new() -> Result<Self, Error> {
        let bridle_config = BridleConfig::load()?;
        let profiles_dir = BridleConfig::profiles_dir()?;
        let manager = ProfileManager::new(profiles_dir);
        let harnesses = HarnessKind::ALL.to_vec();

        for kind in &harnesses {
            let harness = Harness::new(*kind);
            let _ = manager.create_from_current_if_missing(&harness);
        }
        let mut harness_state = ListState::default();
        harness_state.select(Some(0));

        let mut app = Self {
            running: true,
            active_pane: Pane::Harnesses,
            harnesses,
            harness_state,
            profiles: Vec::new(),
            profile_state: ListState::default(),
            expanded_profile: None,
            status_message: Some("Press ? for help".to_string()),
            bridle_config,
            manager,
            show_help: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
        };

        app.refresh_profiles();
        Ok(app)
    }

    fn selected_harness(&self) -> Option<HarnessKind> {
        self.harness_state
            .selected()
            .and_then(|i| self.harnesses.get(i).copied())
    }

    fn harness_status_indicator(&self, harness: &Harness) -> char {
        let harness_id = harness.id();
        if self.bridle_config.active_profile_for(harness_id).is_some() {
            return '*';
        }

        match harness.installation_status() {
            Ok(InstallationStatus::FullyInstalled { .. }) => '+',
            Ok(InstallationStatus::ConfigOnly { .. }) => '+',
            Ok(InstallationStatus::BinaryOnly { .. }) => '-',
            _ => ' ',
        }
    }

    fn empty_state_message(&self) -> &'static str {
        let Some(kind) = self.selected_harness() else {
            return "No harness selected";
        };
        let harness = Harness::new(kind);
        match harness.installation_status() {
            Ok(InstallationStatus::NotInstalled) => "Harness not installed",
            Ok(InstallationStatus::BinaryOnly { .. }) => "Run harness once to generate config",
            Ok(InstallationStatus::ConfigOnly { .. }) => "No binary found - reinstall harness",
            Ok(InstallationStatus::FullyInstalled { .. }) => "Press 'n' to create a profile",
            Ok(_) => "Press 'n' to create a profile",
            Err(_) => "Unable to detect harness status",
        }
    }

    fn refresh_profiles(&mut self) {
        self.profiles.clear();
        self.profile_state.select(None);
        self.expanded_profile = None;

        if let Some(kind) = self.selected_harness() {
            let harness = Harness::new(kind);

            if let Ok(names) = self.manager.list_profiles(&harness) {
                for name in names {
                    if let Ok(info) = self.manager.show_profile(&harness, &name) {
                        self.profiles.push(info);
                    }
                }
            }

            if !self.profiles.is_empty() {
                self.profile_state.select(Some(0));
            }
        }
    }

    fn next_harness(&mut self) {
        let i = match self.harness_state.selected() {
            Some(i) => (i + 1) % self.harnesses.len(),
            None => 0,
        };
        self.harness_state.select(Some(i));
        self.refresh_profiles();
    }

    fn prev_harness(&mut self) {
        let i = match self.harness_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.harnesses.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.harness_state.select(Some(i));
        self.refresh_profiles();
    }

    fn next_profile(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        let i = match self.profile_state.selected() {
            Some(i) => (i + 1) % self.profiles.len(),
            None => 0,
        };
        self.profile_state.select(Some(i));
    }

    fn prev_profile(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        let i = match self.profile_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.profiles.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.profile_state.select(Some(i));
    }

    fn delete_selected(&mut self) {
        let Some(kind) = self.selected_harness() else {
            return;
        };
        let Some(idx) = self.profile_state.selected() else {
            self.status_message = Some("No profile selected".to_string());
            return;
        };
        let profile = &self.profiles[idx];
        let harness = Harness::new(kind);
        let Ok(profile_name) = ProfileName::new(&profile.name) else {
            self.status_message = Some("Invalid profile name".to_string());
            return;
        };

        match self.manager.delete_profile(&harness, &profile_name) {
            Ok(()) => {
                self.status_message = Some(format!("Deleted '{}'", profile.name));
                self.refresh_profiles();
            }
            Err(e) => {
                self.status_message = Some(format!("Delete failed: {}", e));
            }
        }
    }

    fn edit_selected(&mut self) {
        let Some(kind) = self.selected_harness() else {
            return;
        };
        let Some(idx) = self.profile_state.selected() else {
            self.status_message = Some("No profile selected".to_string());
            return;
        };
        let profile = &self.profiles[idx];
        let harness = Harness::new(kind);
        let Ok(profile_name) = ProfileName::new(&profile.name) else {
            self.status_message = Some("Invalid profile name".to_string());
            return;
        };

        let profile_path = self.manager.profile_path(&harness, &profile_name);
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        let _ = restore_terminal_for_editor();
        let status = std::process::Command::new(&editor)
            .arg(&profile_path)
            .status();
        let _ = reinit_terminal_after_editor();

        match status {
            Ok(s) if s.success() => {
                self.status_message = Some(format!("Edited '{}'", profile.name));
                self.refresh_profiles();
            }
            Ok(s) => self.status_message = Some(format!("Editor exited: {}", s)),
            Err(e) => self.status_message = Some(format!("Editor failed: {}", e)),
        }
    }

    fn toggle_expansion(&mut self) {
        let Some(idx) = self.profile_state.selected() else {
            return;
        };
        if self.expanded_profile == Some(idx) {
            self.expanded_profile = None;
            self.status_message = Some("Collapsed".to_string());
        } else {
            self.expanded_profile = Some(idx);
            self.status_message = Some(format!("Expanded profile {}", idx));
        }
    }

    fn is_selected_expanded(&self) -> bool {
        self.profile_state
            .selected()
            .is_some_and(|idx| self.expanded_profile == Some(idx))
    }

    fn switch_to_selected(&mut self) {
        let Some(kind) = self.selected_harness() else {
            return;
        };
        let Some(idx) = self.profile_state.selected() else {
            return;
        };
        let profile = &self.profiles[idx];

        if profile.is_active {
            self.status_message = Some(format!("'{}' is already active", profile.name));
            return;
        }

        let harness = Harness::new(kind);
        let Ok(profile_name) = ProfileName::new(&profile.name) else {
            self.status_message = Some("Invalid profile name".to_string());
            return;
        };

        if let Err(e) = self.manager.backup_current(&harness) {
            self.status_message = Some(format!("Backup failed: {}", e));
            return;
        }

        match self.manager.switch_profile(&harness, &profile_name) {
            Ok(_) => {
                self.bridle_config = BridleConfig::load().unwrap_or_default();
                self.status_message = Some(format!("Switched to '{}'", profile.name));
                self.refresh_profiles();
            }
            Err(e) => {
                self.status_message = Some(format!("Switch failed: {}", e));
            }
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        if self.show_help {
            match key {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return;
        }

        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::CreatingProfile => self.handle_input_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Tab => {
                self.active_pane = match self.active_pane {
                    Pane::Harnesses => Pane::Profiles,
                    Pane::Profiles => Pane::Harnesses,
                };
            }
            KeyCode::Up | KeyCode::Char('k') => match self.active_pane {
                Pane::Harnesses => self.prev_harness(),
                Pane::Profiles => self.prev_profile(),
            },
            KeyCode::Down | KeyCode::Char('j') => match self.active_pane {
                Pane::Harnesses => self.next_harness(),
                Pane::Profiles => self.next_profile(),
            },
            KeyCode::Enter => {
                if self.active_pane == Pane::Profiles {
                    if self.is_selected_expanded() {
                        self.switch_to_selected();
                    } else {
                        self.toggle_expansion();
                    }
                }
            }
            KeyCode::Char(' ') => {
                if self.active_pane == Pane::Profiles {
                    self.toggle_expansion();
                }
            }
            KeyCode::Char('r') => {
                self.refresh_profiles();
                self.status_message = Some("Refreshed".to_string());
            }
            KeyCode::Char('n') => {
                self.input_mode = InputMode::CreatingProfile;
                self.input_buffer.clear();
                self.status_message = Some("Enter profile name (Esc to cancel)".to_string());
            }
            KeyCode::Char('d') => {
                if self.active_pane == Pane::Profiles {
                    self.delete_selected();
                }
            }
            KeyCode::Char('e') => {
                if self.active_pane == Pane::Profiles {
                    self.edit_selected();
                }
            }
            _ => {}
        }
    }

    fn handle_input_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Enter => self.create_profile_from_input(),
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
                self.status_message = None;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
    }

    fn create_profile_from_input(&mut self) {
        let name = self.input_buffer.trim().to_string();
        if name.is_empty() {
            self.status_message = Some("Profile name cannot be empty".to_string());
            return;
        }

        let Some(kind) = self.selected_harness() else {
            self.status_message = Some("No harness selected".to_string());
            self.input_mode = InputMode::Normal;
            self.input_buffer.clear();
            return;
        };

        let harness = Harness::new(kind);
        let profile_name = match ProfileName::new(&name) {
            Ok(pn) => pn,
            Err(_) => {
                self.status_message = Some("Invalid profile name".to_string());
                return;
            }
        };

        match self.manager.create_from_current(&harness, &profile_name) {
            Ok(_) => {
                self.status_message = Some(format!("Created profile '{}'", name));
                self.refresh_profiles();
            }
            Err(e) => {
                self.status_message = Some(format!("Failed: {}", e));
            }
        }

        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }
}

fn init_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn restore_terminal_for_editor() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn reinit_terminal_after_editor() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[0]);

    render_harness_pane(frame, app, main_chunks[0]);
    render_profile_pane(frame, app, main_chunks[1]);
    render_status_bar(frame, app, chunks[1]);

    if app.show_help {
        render_help_modal(frame, frame.area());
    }
}

fn render_harness_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.active_pane == Pane::Harnesses;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app
        .harnesses
        .iter()
        .map(|kind| {
            let harness = Harness::new(*kind);
            let indicator = app.harness_status_indicator(&harness);
            let installed = harness.is_installed();
            let style = if installed {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let suffix = if installed { "" } else { " (not installed)" };
            ListItem::new(format!("{} {}{}", indicator, harness.kind(), suffix)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Harnesses ")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.harness_state);
}

fn render_profile_compact(profile: &ProfileInfo) -> Line<'static> {
    let active_marker = if profile.is_active { "● " } else { "  " };
    let mcp_count = profile.mcp_servers.len();
    let mcp_info = if mcp_count > 0 {
        format!(" ({} MCP)", mcp_count)
    } else {
        String::new()
    };
    let style = if profile.is_active {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::styled(
        format!("{}{}{}", active_marker, profile.name, mcp_info),
        style,
    )
}

fn render_profile_expanded(profile: &ProfileInfo) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let active_marker = if profile.is_active { "● " } else { "  " };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{}{}", active_marker, profile.name),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " ─────────────────────────",
            Style::default().fg(Color::Gray),
        ),
    ]));

    let mut theme_model = Vec::new();
    if let Some(ref theme) = profile.theme {
        theme_model.push(format!("Theme: {}", theme));
    }
    if let Some(ref model) = profile.model {
        theme_model.push(format!("Model: {}", model));
    }
    if !theme_model.is_empty() {
        lines.push(Line::styled(
            format!("  │ {}", theme_model.join("  ")),
            Style::default().fg(Color::Gray),
        ));
    }

    if !profile.mcp_servers.is_empty() {
        let mut spans: Vec<Span> =
            vec![Span::styled("  │ MCP: ", Style::default().fg(Color::Gray))];
        for (i, server) in profile.mcp_servers.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            let (marker, color) = if server.enabled {
                ("✓", Color::Green)
            } else {
                ("✗", Color::Red)
            };
            spans.push(Span::styled(
                format!("{}{}", marker, server.name),
                Style::default().fg(color),
            ));
        }
        lines.push(Line::from(spans));
    }

    let mut counts = vec![
        format!("Skills: {}", profile.skills.items.len()),
        format!("Commands: {}", profile.commands.items.len()),
    ];
    if let Some(ref plugins) = profile.plugins {
        counts.push(format!("Plugins: {}", plugins.items.len()));
    }
    if let Some(ref agents) = profile.agents {
        counts.push(format!("Agents: {}", agents.items.len()));
    }
    lines.push(Line::styled(
        format!("  │ {}", counts.join("  ")),
        Style::default().fg(Color::Gray),
    ));

    if let Some(ref rules) = profile.rules_file
        && let Some(filename) = rules.file_name()
    {
        lines.push(Line::styled(
            format!("  │ Rules: {}", filename.to_string_lossy()),
            Style::default().fg(Color::Gray),
        ));
    }

    for error in &profile.extraction_errors {
        lines.push(Line::styled(
            format!("  │ ⚠ {}", error),
            Style::default().fg(Color::Yellow),
        ));
    }

    lines.push(Line::styled(
        "  └─────────────────────────────",
        Style::default().fg(Color::Gray),
    ));

    lines
}

fn render_profile_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.active_pane == Pane::Profiles;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let (list_area, input_area) = if app.input_mode == InputMode::CreatingProfile {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    if app.profiles.is_empty() && app.input_mode != InputMode::CreatingProfile {
        let message = app.empty_state_message();
        let block = Block::default()
            .title(match app.selected_harness() {
                Some(kind) => format!(" Profiles ({:?}) ", kind),
                None => " Profiles ".to_string(),
            })
            .borders(Borders::ALL)
            .border_style(border_style);
        frame.render_widget(block, area);

        let inner = area.inner(ratatui::layout::Margin::new(2, 2));
        let paragraph = Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, inner);
        return;
    }

    let items: Vec<ListItem> = app
        .profiles
        .iter()
        .enumerate()
        .map(|(idx, profile)| {
            let is_expanded = app.expanded_profile == Some(idx);
            if is_expanded {
                ListItem::new(Text::from(render_profile_expanded(profile)))
            } else {
                ListItem::new(render_profile_compact(profile))
            }
        })
        .collect();

    let title = match app.selected_harness() {
        Some(kind) => format!(" Profiles ({:?}) ", kind),
        None => " Profiles ".to_string(),
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, list_area, &mut app.profile_state);

    if let Some(input_area) = input_area {
        let input_text = format!("{}█", app.input_buffer);
        let input = Paragraph::new(input_text)
            .block(
                Block::default()
                    .title(" Profile name: ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().fg(Color::White));
        frame.render_widget(input, input_area);
    }
}

fn render_help_modal(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  j / ↓     Move down"),
        Line::from("  k / ↑     Move up"),
        Line::from("  Tab       Switch pane"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Enter     Switch to profile"),
        Line::from("  n         New profile"),
        Line::from("  d         Delete profile"),
        Line::from("  e         Edit profile"),
        Line::from("  r         Refresh"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Harness Status",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  *         Tracked (active profile)"),
        Line::from("  +         Has config (not tracked)"),
        Line::from("  -         Binary only (no config)"),
        Line::from("            Not installed"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ?         Toggle help"),
        Line::from("  q / Esc   Quit"),
    ];

    let width = 40;
    let height = help_text.len() as u16 + 4;
    let x = area.width.saturating_sub(width) / 2;
    let y = area.height.saturating_sub(height) / 2;
    let modal_area = Rect::new(x, y, width.min(area.width), height.min(area.height));

    frame.render_widget(Clear, modal_area);

    let help_block = Block::default()
        .title(" Help ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    let help_paragraph = Paragraph::new(help_text).block(help_block);
    frame.render_widget(help_paragraph, modal_area);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let help = "q:quit  Tab:pane  j/k:nav  Enter:switch  n:new  d:del  e:edit  r:refresh";
    let msg = app.status_message.as_deref().unwrap_or("");

    let spans = vec![
        Span::styled(help, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(msg, Style::default().fg(Color::Yellow)),
    ];

    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

pub fn run() -> Result<(), Error> {
    let mut terminal = init_terminal().map_err(Error::Io)?;

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        hook(info);
    }));

    let mut app = App::new()?;

    while app.running {
        terminal
            .draw(|frame| ui(frame, &mut app))
            .map_err(Error::Io)?;

        if event::poll(std::time::Duration::from_millis(100)).map_err(Error::Io)?
            && let Event::Key(key) = event::read().map_err(Error::Io)?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key.code);
        }
    }

    restore_terminal(&mut terminal).map_err(Error::Io)?;
    Ok(())
}
