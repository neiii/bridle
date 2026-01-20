use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Margin, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

pub struct EmptyState<'a> {
    title: &'a str,
    lines: Vec<String>,
    is_focused: bool,
}

impl<'a> EmptyState<'a> {
    pub fn new(title: &'a str, lines: Vec<String>) -> Self {
        Self {
            title,
            lines,
            is_focused: false,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    fn count_wrapped_lines(&self, text: &str, width: usize) -> usize {
        if width == 0 {
            return 0;
        }
        textwrap::wrap(text, width).len()
    }
}

impl Widget for EmptyState<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        let padded = inner.inner(Margin::new(4, 1));

        if padded.width == 0 || padded.height == 0 {
            return;
        }

        let total_wrapped_lines: usize = self
            .lines
            .iter()
            .map(|line| self.count_wrapped_lines(line, padded.width as usize))
            .sum();

        let vertical_offset = if total_wrapped_lines < padded.height as usize {
            (padded.height as usize - total_wrapped_lines) / 2
        } else {
            0
        };

        let content_lines: Vec<Line> = self
            .lines
            .into_iter()
            .map(|line| Line::from(line).style(Style::default().add_modifier(Modifier::DIM)))
            .collect();

        let content_area = Rect {
            x: padded.x,
            y: padded.y + vertical_offset as u16,
            width: padded.width,
            height: padded.height.saturating_sub(vertical_offset as u16),
        };

        Paragraph::new(content_lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .render(content_area, buf);
    }
}
