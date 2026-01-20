use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use super::EmptyState;
use crate::config::ProfileInfo;

pub fn render_profile_details(profile: &ProfileInfo) -> Vec<Line<'static>> {
    let nodes = crate::display::profile_to_nodes(profile);
    crate::display::nodes_to_lines(&nodes)
}

pub struct DetailPane<'a> {
    profile: Option<&'a ProfileInfo>,
    is_focused: bool,
    scroll_offset: u16,
}

impl<'a> DetailPane<'a> {
    pub fn new(profile: Option<&'a ProfileInfo>) -> Self {
        Self {
            profile,
            is_focused: false,
            scroll_offset: 0,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    pub fn scroll(mut self, offset: u16) -> Self {
        self.scroll_offset = offset;
        self
    }
}

impl Widget for DetailPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.profile {
            None => {
                let lines = vec![
                    "Select a profile to view details".to_string(),
                    String::new(),
                    "Tab to focus Profiles".to_string(),
                ];
                let widget = EmptyState::new("Details", lines).focused(self.is_focused);
                widget.render(area, buf);
            }
            Some(profile) => {
                let border_style = if self.is_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let block = Block::default()
                    .title(" Details ")
                    .borders(Borders::ALL)
                    .border_style(border_style);

                let content = render_profile_details(profile);

                Paragraph::new(content)
                    .block(block)
                    .scroll((self.scroll_offset, 0))
                    .render(area, buf);
            }
        }
    }
}
