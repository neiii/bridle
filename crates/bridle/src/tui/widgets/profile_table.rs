use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, StatefulWidget, Table, TableState, Widget},
};

use super::EmptyState;
use crate::config::ProfileInfo;

pub struct ProfileTable<'a> {
    profiles: &'a [ProfileInfo],
    block: Option<Block<'a>>,
    focused: bool,
}

impl<'a> ProfileTable<'a> {
    pub fn new(profiles: &'a [ProfileInfo]) -> Self {
        Self {
            profiles,
            block: None,
            focused: false,
        }
    }

    #[allow(dead_code)]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    fn truncate_model(model: &str, max_len: usize) -> String {
        if model.len() <= max_len {
            model.to_string()
        } else {
            let parts: Vec<&str> = model.split('/').collect();
            if parts.len() > 1 {
                let model_name = parts.last().unwrap_or(&model);
                if model_name.len() <= max_len {
                    model_name.to_string()
                } else {
                    format!("{}...", &model_name[..max_len.saturating_sub(3)])
                }
            } else {
                format!("{}...", &model[..max_len.saturating_sub(3)])
            }
        }
    }
}

impl StatefulWidget for ProfileTable<'_> {
    type State = TableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if self.profiles.is_empty() {
            let lines = vec![
                "No profiles found".to_string(),
                String::new(),
                "Press 'n' to create a profile".to_string(),
            ];
            let widget = EmptyState::new("Profiles", lines).focused(self.focused);
            widget.render(area, buf);
            return;
        }

        let header_cells = ["", "Name", "Model", "MCP"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().add_modifier(Modifier::BOLD)));
        let header = Row::new(header_cells)
            .style(Style::default().fg(Color::Cyan))
            .height(1);

        let rows = self.profiles.iter().map(|profile| {
            let active = if profile.is_active { "●" } else { " " };
            let active_style = if profile.is_active {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            let model = profile
                .model
                .as_deref()
                .map(|m| Self::truncate_model(m, 22))
                .unwrap_or_else(|| "-".to_string());

            let mcp_count = profile.mcp_servers.len();
            let mcp = if mcp_count > 0 {
                format!("{}", mcp_count)
            } else {
                "-".to_string()
            };

            Row::new(vec![
                Cell::from(active).style(active_style),
                Cell::from(profile.name.as_str()),
                Cell::from(model).style(Style::default().add_modifier(Modifier::DIM)),
                Cell::from(mcp).style(Style::default().add_modifier(Modifier::DIM)),
            ])
        });

        let widths = [
            Constraint::Length(2),
            Constraint::Min(10),
            Constraint::Length(24),
            Constraint::Length(4),
        ];

        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let table = Table::new(rows, widths)
            .header(header)
            .row_highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(" Profiles "),
            );

        let table = if let Some(block) = self.block {
            table.block(block)
        } else {
            table
        };

        StatefulWidget::render(table, area, buf, state);
    }
}
