use ratatui::style::{Color, Modifier, Style};

/// Theme constants for consistent styling across the TUI.
pub struct Theme;

impl Theme {
    #[cfg(feature = "tui-cards")]
    pub fn profile_active() -> Style {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    }

    // Harness states
    pub fn harness_installed() -> Style {
        Style::default()
    }

    pub fn harness_not_installed() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    // Text styles
    pub fn text_muted() -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    pub fn text_warning() -> Style {
        Style::default().fg(Color::Yellow)
    }

    // Tab styles
    pub fn tab_selected() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }
}
