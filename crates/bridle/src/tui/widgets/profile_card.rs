#![cfg(feature = "tui-cards")]

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::config::ProfileInfo;
use crate::tui::theme::Theme;

pub struct ProfileCard<'a> {
    profile: &'a ProfileInfo,
    selected: bool,
    focused: bool,
}

impl<'a> ProfileCard<'a> {
    pub fn new(profile: &'a ProfileInfo) -> Self {
        Self {
            profile,
            selected: false,
            focused: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Widget for ProfileCard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Theme::border_active()
        } else if self.selected {
            Theme::profile_active()
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(self.profile.name.clone());

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        let mut lines = Vec::new();

        if self.profile.is_active {
            lines.push(Line::from(Span::styled(
                "● Active",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
        }

        if let Some(model) = &self.profile.model {
            let display = if model.len() > inner.width as usize - 2 {
                format!("{}…", &model[..inner.width as usize - 3])
            } else {
                model.clone()
            };
            lines.push(Line::from(Span::styled(
                display,
                Style::default().fg(Color::Cyan),
            )));
        }

        let mcp_count = self.profile.mcp_servers.len();
        if mcp_count > 0 {
            lines.push(Line::from(Span::styled(
                format!("{} MCP servers", mcp_count),
                Style::default().fg(Color::Yellow),
            )));
        }

        if let Some(theme) = &self.profile.theme {
            lines.push(Line::from(Span::styled(
                format!("Theme: {}", theme),
                Style::default().fg(Color::Gray),
            )));
        }

        let para = Paragraph::new(lines);
        para.render(inner, buf);
    }
}

pub struct NewProfileCard {
    focused: bool,
}

impl NewProfileCard {
    pub fn new() -> Self {
        Self { focused: false }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }
}

impl Default for NewProfileCard {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for NewProfileCard {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Theme::border_active()
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("+ New Profile");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 5 {
            return;
        }

        let text = Paragraph::new(Line::from(Span::styled(
            "Create new profile",
            Style::default().fg(Color::DarkGray),
        )));
        text.render(inner, buf);
    }
}
