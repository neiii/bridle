//! Terminal user interface for bridle.
//!
//! Provides an interactive TUI for browsing harnesses, profiles, and their configurations.

mod theme;
mod views;
mod widgets;

use std::io::{self, Stdout};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEvent,
        MouseEventKind,
    },
    execute,
    terminal::{
        ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    },
};
use harness_locate::{Harness, HarnessKind, InstallationStatus};

use crate::harness::HarnessConfig;
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Alignment, CrosstermBackend},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, TableState},
};

use crate::config::{BridleConfig, ProfileInfo, ProfileManager, ProfileName};
use crate::error::Error;
use views::ViewMode;
use widgets::{DetailPane, HarnessTabs, ProfileTable, StatusBar};

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn harness_id(kind: &HarnessKind) -> &'static str {
    match kind {
        HarnessKind::ClaudeCode => "claude-code",
        HarnessKind::OpenCode => "opencode",
        HarnessKind::Goose => "goose",
        HarnessKind::AmpCode => "amp-code",
        _ => "unknown",
    }
}

fn harness_name(kind: &HarnessKind) -> &'static str {
    match kind {
        HarnessKind::ClaudeCode => "Claude Code",
        HarnessKind::OpenCode => "OpenCode",
        HarnessKind::Goose => "Goose",
        HarnessKind::AmpCode => "AMP Code",
        _ => "Unknown",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Harnesses,
    Profiles,
    Details,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    #[default]
    Normal,
    CreatingProfile,
    ConfirmingDelete,
}

#[derive(Debug)]
struct App {
    running: bool,
    view_mode: ViewMode,
    active_pane: Pane,
    harnesses: Vec<HarnessKind>,
    harness_state: ListState,
    profiles: Vec<ProfileInfo>,
    profile_state: ListState,
    profile_table_state: TableState,
    expanded_profile: Option<usize>,
    status_message: Option<String>,
    bridle_config: BridleConfig,
    manager: ProfileManager,
    show_help: bool,
    input_mode: InputMode,
    input_buffer: String,
    needs_full_redraw: bool,
    detail_scroll: u16,
    detail_content_height: u16,
    harness_area: Option<Rect>,
    profile_area: Option<Rect>,
    detail_area: Option<Rect>,
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
        let default_idx = bridle_config
            .default_harness()
            .and_then(|id| harnesses.iter().position(|h| harness_id(h) == id))
            .unwrap_or(0);
        harness_state.select(Some(default_idx));

        let mut app = Self {
            running: true,
            view_mode: ViewMode::default(),
            active_pane: Pane::Profiles,
            harnesses,
            harness_state,
            profiles: Vec::new(),
            profile_state: ListState::default(),
            profile_table_state: TableState::default(),
            expanded_profile: None,
            status_message: None,
            bridle_config,
            manager,
            show_help: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            needs_full_redraw: false,
            detail_scroll: 0,
            detail_content_height: 0,
            harness_area: None,
            profile_area: None,
            detail_area: None,
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

    fn sync_active_profiles(&mut self) {
        for &kind in &self.harnesses {
            let harness = Harness::new(kind);
            let harness_id = harness.id();
            if let Some(active_name) = self.bridle_config.active_profile_for(harness_id)
                && let Ok(profile_name) = ProfileName::new(active_name)
            {
                let _ = self
                    .manager
                    .save_to_profile(&harness, Some(&harness), &profile_name);
            }
        }
    }

    fn refresh_profiles(&mut self) {
        self.sync_active_profiles();
        self.profiles.clear();
        self.profile_state.select(None);
        self.profile_table_state.select(None);
        self.expanded_profile = None;
        self.detail_scroll = 0;

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
                self.profile_table_state.select(Some(0));
                self.update_detail_content_height();
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
        self.profile_table_state.select(Some(i));
        self.detail_scroll = 0;
        self.update_detail_content_height();
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
        self.profile_table_state.select(Some(i));
        self.detail_scroll = 0;
        self.update_detail_content_height();
    }

    fn update_detail_content_height(&mut self) {
        self.detail_content_height = if let Some(idx) = self.profile_state.selected() {
            let profile = &self.profiles[idx];
            let lines = widgets::render_profile_details(profile);
            lines.len() as u16
        } else {
            0
        };
    }

    fn scroll_detail_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    fn scroll_detail_down(&mut self) {
        let viewport_height = self
            .detail_area
            .map(|a| a.height.saturating_sub(2))
            .unwrap_or(10);
        let max_scroll = self.detail_content_height.saturating_sub(viewport_height);
        if self.detail_scroll < max_scroll {
            self.detail_scroll = self.detail_scroll.saturating_add(1);
        }
    }

    fn handle_mouse(&mut self, event: MouseEvent) {
        let pos = ratatui::layout::Position::new(event.column, event.row);

        match event.kind {
            MouseEventKind::Down(_) => {
                if self.harness_area.is_some_and(|a| a.contains(pos)) {
                    let area = self.harness_area.unwrap();

                    if self.view_mode == ViewMode::Dashboard {
                        let inner_x = event.column.saturating_sub(area.x).saturating_sub(2);
                        let tab_width = 15;
                        let idx = (inner_x / tab_width) as usize;
                        if idx < self.harnesses.len() {
                            self.harness_state.select(Some(idx));
                            self.refresh_profiles();
                        }
                    } else {
                        let inner_y = event.row.saturating_sub(area.y).saturating_sub(1);
                        let idx = inner_y as usize;
                        if idx < self.harnesses.len() {
                            self.harness_state.select(Some(idx));
                            self.refresh_profiles();
                        }
                    }
                } else if self.profile_area.is_some_and(|a| a.contains(pos)) {
                    self.active_pane = Pane::Profiles;
                    let area = self.profile_area.unwrap();
                    let inner_y = event.row.saturating_sub(area.y).saturating_sub(2);
                    let idx = inner_y as usize;
                    if idx < self.profiles.len() {
                        self.profile_state.select(Some(idx));
                        self.profile_table_state.select(Some(idx));
                        self.detail_scroll = 0;
                    }
                } else if self.detail_area.is_some_and(|a| a.contains(pos)) {
                    self.active_pane = Pane::Details;
                }
            }
            MouseEventKind::ScrollUp => {
                if self.detail_area.is_some_and(|a| a.contains(pos)) {
                    self.scroll_detail_up();
                } else if self.profile_area.is_some_and(|a| a.contains(pos)) {
                    self.prev_profile();
                } else if self.harness_area.is_some_and(|a| a.contains(pos)) {
                    self.prev_harness();
                }
            }
            MouseEventKind::ScrollDown => {
                if self.detail_area.is_some_and(|a| a.contains(pos)) {
                    self.scroll_detail_down();
                } else if self.profile_area.is_some_and(|a| a.contains(pos)) {
                    self.next_profile();
                } else if self.harness_area.is_some_and(|a| a.contains(pos)) {
                    self.next_harness();
                }
            }
            _ => {}
        }
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
        let (program, args) = self.bridle_config.editor_command();

        let _ = restore_terminal_for_editor();

        // Clear screen and show message while editor is open
        print!("\x1B[2J\x1B[H"); // Clear screen, move cursor to top-left
        println!("Editing profile: {}", profile.name);
        println!("Close the editor to return to bridle.\n");
        let _ = std::io::Write::flush(&mut std::io::stdout());

        // On Windows, use cmd /c to invoke the editor so that .cmd/.bat wrappers
        // (like VS Code's `code.cmd`) are resolved correctly.
        #[cfg(windows)]
        let status = std::process::Command::new("cmd")
            .arg("/c")
            .arg(&program)
            .args(&args)
            .arg(&profile_path)
            .status();

        #[cfg(not(windows))]
        let status = std::process::Command::new(&program)
            .args(&args)
            .arg(&profile_path)
            .status();
        let _ = reinit_terminal_after_editor();
        self.needs_full_redraw = true;

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

        match self
            .manager
            .switch_profile_with_resources(&harness, Some(&harness), &profile_name)
        {
            Ok(_) => {
                self.bridle_config = BridleConfig::load().unwrap_or_default();
                self.status_message = Some(format!("Switched to '{}'", profile.name));
                let selected_idx = self.profile_state.selected();
                self.refresh_profiles();
                if let Some(idx) = selected_idx {
                    self.profile_state.select(Some(idx));
                    self.profile_table_state.select(Some(idx));
                }
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
            InputMode::ConfirmingDelete => self.handle_confirm_delete_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::F(2) => {
                self.view_mode.toggle();
                self.status_message = Some(format!("View: {}", self.view_mode.name()));
            }
            KeyCode::Tab => {
                self.active_pane = match self.active_pane {
                    Pane::Harnesses | Pane::Profiles => Pane::Details,
                    Pane::Details => Pane::Profiles,
                };
            }
            KeyCode::Up | KeyCode::Char('k') => match self.view_mode {
                ViewMode::Dashboard => {
                    if self.active_pane == Pane::Details {
                        self.scroll_detail_up();
                    } else {
                        self.prev_profile();
                    }
                }
                ViewMode::Legacy => match self.active_pane {
                    Pane::Harnesses => self.prev_harness(),
                    Pane::Profiles => self.prev_profile(),
                    Pane::Details => self.scroll_detail_up(),
                },
                #[cfg(feature = "tui-cards")]
                ViewMode::Cards => self.prev_profile(),
            },
            KeyCode::Down | KeyCode::Char('j') => match self.view_mode {
                ViewMode::Dashboard => {
                    if self.active_pane == Pane::Details {
                        self.scroll_detail_down();
                    } else {
                        self.next_profile();
                    }
                }
                ViewMode::Legacy => match self.active_pane {
                    Pane::Harnesses => self.next_harness(),
                    Pane::Profiles => self.next_profile(),
                    Pane::Details => self.scroll_detail_down(),
                },
                #[cfg(feature = "tui-cards")]
                ViewMode::Cards => self.next_profile(),
            },
            KeyCode::Left | KeyCode::Char('h') => {
                if self.view_mode == ViewMode::Dashboard {
                    self.prev_harness();
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.view_mode == ViewMode::Dashboard {
                    self.next_harness();
                }
            }
            KeyCode::Enter => match self.view_mode {
                ViewMode::Dashboard => {
                    self.switch_to_selected();
                }
                ViewMode::Legacy => {
                    if self.active_pane == Pane::Profiles {
                        if self.is_selected_expanded() {
                            self.switch_to_selected();
                        } else {
                            self.toggle_expansion();
                        }
                    }
                }
                #[cfg(feature = "tui-cards")]
                ViewMode::Cards => {
                    self.switch_to_selected();
                }
            },
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
                if (matches!(self.view_mode, ViewMode::Dashboard)
                    || self.active_pane == Pane::Profiles)
                    && let Some(idx) = self.profile_state.selected()
                    && let Some(profile) = self.profiles.get(idx)
                {
                    self.input_buffer = profile.name.clone();
                    self.input_mode = InputMode::ConfirmingDelete;
                }
            }
            KeyCode::Char('e') => {
                if matches!(self.view_mode, ViewMode::Dashboard)
                    || self.active_pane == Pane::Profiles
                {
                    self.edit_selected();
                }
            }
            KeyCode::Char('f') => {
                if let Some(harness_kind) = self.selected_harness() {
                    let id = harness_id(&harness_kind);
                    self.bridle_config.set_default_harness(Some(id));
                    if let Err(e) = self.bridle_config.save() {
                        self.status_message = Some(format!("Failed to save: {}", e));
                    } else {
                        self.status_message = Some(format!(
                            "Set {} as default harness",
                            harness_name(&harness_kind)
                        ));
                    }
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

    fn handle_confirm_delete_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.delete_selected();
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_buffer.clear();
                self.status_message = Some("Delete cancelled".to_string());
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

        match self.manager.create_from_current_with_resources(
            &harness,
            Some(&harness),
            &profile_name,
        ) {
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn restore_terminal_for_editor() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}

fn reinit_terminal_after_editor() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        crossterm::terminal::Clear(ClearType::All),
        EnableMouseCapture
    )?;
    Ok(())
}

fn ui(frame: &mut Frame, app: &mut App) {
    match app.view_mode {
        ViewMode::Legacy => render_legacy_view(frame, app),
        ViewMode::Dashboard => render_dashboard_view(frame, app),
        #[cfg(feature = "tui-cards")]
        ViewMode::Cards => render_dashboard_view(frame, app), // TODO: implement cards view
    }

    if app.show_help {
        render_help_modal(frame, frame.area(), app.view_mode);
    }
}

fn render_legacy_view(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[0]);

    app.harness_area = Some(main_chunks[0]);
    app.profile_area = Some(main_chunks[1]);
    app.detail_area = None;
    render_harness_pane(frame, app, main_chunks[0]);
    render_profile_pane(frame, app, main_chunks[1]);
    render_status_bar(frame, app, chunks[1]);
}

fn render_dashboard_view(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    app.harness_area = Some(chunks[0]);
    render_harness_tabs(frame, app, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    app.profile_area = Some(content_chunks[0]);
    app.detail_area = Some(content_chunks[1]);
    render_profile_table(frame, app, content_chunks[0]);
    render_detail_pane(frame, app, content_chunks[1]);

    render_status_bar(frame, app, chunks[2]);

    if app.input_mode == InputMode::CreatingProfile {
        render_input_popup(frame, app);
    }
    if app.input_mode == InputMode::ConfirmingDelete {
        render_confirm_delete_popup(frame, app);
    }
}

fn render_confirm_delete_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 3;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let confirm_text = format!("Delete '{}'? (y/n)", app.input_buffer);
    let confirm = Paragraph::new(confirm_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title(" Confirm Delete "),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(confirm, popup_area);
}

fn render_input_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 3;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let input_text = format!("{}█", app.input_buffer);
    let input = Paragraph::new(input_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(" New Profile Name (Enter to create, Esc to cancel) "),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(input, popup_area);
}

fn render_profile_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let table = ProfileTable::new(&app.profiles).focused(app.active_pane == Pane::Profiles);
    frame.render_stateful_widget(table, area, &mut app.profile_table_state);
}

fn render_detail_pane(frame: &mut Frame, app: &App, area: Rect) {
    let selected_profile = app
        .profile_table_state
        .selected()
        .and_then(|i| app.profiles.get(i));

    let detail = DetailPane::new(selected_profile)
        .focused(app.active_pane == Pane::Details)
        .scroll(app.detail_scroll);
    frame.render_widget(detail, area);
}

fn render_harness_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let mut tabs = HarnessTabs::new(&app.harnesses, app.harness_state.selected().unwrap_or(0));

    for kind in &app.harnesses {
        let harness = Harness::new(*kind);
        if app.bridle_config.active_profile_for(harness.id()).is_some() {
            tabs = tabs.with_active_indicator(harness.id());
        }
    }

    frame.render_widget(tabs, area);
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

    let mut summary_parts = Vec::new();
    if let Some(model) = &profile.model {
        let short_model = model
            .split('/')
            .next_back()
            .unwrap_or(model)
            .chars()
            .take(25)
            .collect::<String>();
        summary_parts.push(short_model);
    }
    let mcp_count = profile.mcp_servers.len();
    if mcp_count > 0 {
        summary_parts.push(format!("{} MCP", mcp_count));
    }

    let summary = if summary_parts.is_empty() {
        String::new()
    } else {
        format!(" — {}", summary_parts.join(", "))
    };

    let style = if profile.is_active {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::styled(
        format!("{}{}{}", active_marker, profile.name, summary),
        style,
    )
}

fn render_profile_expanded(profile: &ProfileInfo) -> Vec<Line<'static>> {
    let nodes = crate::display::profile_to_nodes(profile);
    crate::display::nodes_to_lines(&nodes)
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

fn render_help_modal(frame: &mut Frame, area: Rect, view_mode: views::ViewMode) {
    let mut help_text = vec![Line::from(vec![Span::styled(
        "Navigation",
        Style::default().add_modifier(Modifier::BOLD),
    )])];

    #[cfg(feature = "tui-cards")]
    if matches!(view_mode, views::ViewMode::Cards) {
        help_text.extend([
            Line::from("  ←/→       Move left/right"),
            Line::from("  ↑/↓       Move up/down"),
            Line::from("  Space     View details"),
        ]);
    } else {
        help_text.extend([
            Line::from("  j / ↓     Move down"),
            Line::from("  k / ↑     Move up"),
            Line::from("  Tab       Switch pane"),
        ]);
    }

    #[cfg(not(feature = "tui-cards"))]
    {
        let _ = view_mode;
        help_text.extend([
            Line::from("  j / ↓     Move down / scroll"),
            Line::from("  k / ↑     Move up / scroll"),
            Line::from("  Tab       Cycle panes"),
        ]);
    }

    help_text.extend([
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Enter     Switch to profile"),
        Line::from("  n         New profile"),
        Line::from("  d         Delete profile"),
        Line::from("  e         Edit profile"),
        Line::from("  f         Set default harness"),
        Line::from("  r         Refresh"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Harness Status",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ●         Tracked (active profile)"),
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
    ]);

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
    let harness_status = app.selected_harness().map(|kind| {
        let harness = Harness::new(kind);
        match harness.installation_status() {
            Ok(status) => StatusBar::installation_status_text(&status),
            Err(_) => "Unknown",
        }
    });

    let status_bar = StatusBar::new(app.view_mode)
        .message(app.status_message.as_deref())
        .harness_status(harness_status);
    frame.render_widget(status_bar, area);
}

pub fn run() -> Result<(), Error> {
    let mut terminal = init_terminal().map_err(Error::Io)?;

    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        hook(info);
    }));

    let mut app = App::new()?;

    while app.running {
        if app.needs_full_redraw {
            terminal.clear().map_err(Error::Io)?;
            app.needs_full_redraw = false;
        }
        terminal
            .draw(|frame| ui(frame, &mut app))
            .map_err(Error::Io)?;

        if event::poll(std::time::Duration::from_millis(100)).map_err(Error::Io)? {
            match event::read().map_err(Error::Io)? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key.code);
                }
                Event::Mouse(mouse) => {
                    app.handle_mouse(mouse);
                }
                _ => {}
            }
        }
    }

    restore_terminal(&mut terminal).map_err(Error::Io)?;
    Ok(())
}
