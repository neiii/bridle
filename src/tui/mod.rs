use std::io::{self, Stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use get_harness::{Harness, HarnessKind};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::config::{BridleConfig, ProfileInfo, ProfileManager, ProfileName};
use crate::error::Error;

type Tui = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Harnesses,
    Profiles,
}

#[derive(Debug)]
struct App {
    running: bool,
    active_pane: Pane,
    harnesses: Vec<HarnessKind>,
    harness_state: ListState,
    profiles: Vec<ProfileInfo>,
    profile_state: ListState,
    status_message: Option<String>,
    bridle_config: BridleConfig,
    manager: ProfileManager,
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
            status_message: Some("Press ? for help".to_string()),
            bridle_config,
            manager,
        };

        app.refresh_profiles();
        Ok(app)
    }

    fn selected_harness(&self) -> Option<HarnessKind> {
        self.harness_state.selected().map(|i| self.harnesses[i])
    }

    fn refresh_profiles(&mut self) {
        self.profiles.clear();
        self.profile_state.select(None);

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

    fn create_new_profile(&mut self) {
        self.status_message = Some("Use CLI: bridle profile create <harness> <name>".to_string());
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
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
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
                    self.switch_to_selected();
                }
            }
            KeyCode::Char('r') => {
                self.refresh_profiles();
                self.status_message = Some("Refreshed".to_string());
            }
            KeyCode::Char('n') => self.create_new_profile(),
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
            let installed = harness.is_installed();
            let style = if installed {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let suffix = if installed { "" } else { " (not installed)" };
            ListItem::new(format!("{}{}", harness.kind(), suffix)).style(style)
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

fn render_profile_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.active_pane == Pane::Profiles;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app
        .profiles
        .iter()
        .map(|profile| {
            let active_marker = if profile.is_active { "● " } else { "  " };
            let mcp_count = profile.mcp_servers.len();
            let mcp_info = if mcp_count > 0 {
                format!(" [{} MCPs]", mcp_count)
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

            ListItem::new(format!("{}{}{}", active_marker, profile.name, mcp_info)).style(style)
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

    frame.render_stateful_widget(list, area, &mut app.profile_state);
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
