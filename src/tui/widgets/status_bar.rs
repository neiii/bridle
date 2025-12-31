use crate::tui::theme::Theme;
use crate::tui::views::ViewMode;
use harness_locate::InstallationStatus;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct StatusBar<'a> {
    view_mode: ViewMode,
    message: Option<&'a str>,
    harness_status: Option<&'a str>,
}

impl<'a> StatusBar<'a> {
    pub fn new(view_mode: ViewMode) -> Self {
        Self {
            view_mode,
            message: None,
            harness_status: None,
        }
    }

    pub fn message(mut self, msg: Option<&'a str>) -> Self {
        self.message = msg;
        self
    }

    pub fn harness_status(mut self, status: Option<&'a str>) -> Self {
        self.harness_status = status;
        self
    }

    pub fn installation_status_text(status: &InstallationStatus) -> &'static str {
        match status {
            InstallationStatus::FullyInstalled { .. } => "Installed",
            InstallationStatus::ConfigOnly { .. } => "Config only",
            InstallationStatus::BinaryOnly { .. } => "Binary only",
            InstallationStatus::NotInstalled => "Not installed",
            _ => "Unknown",
        }
    }

    fn keybindings(&self) -> &'static str {
        match self.view_mode {
            ViewMode::Dashboard => {
                "q:quit  ←/→:harness  ↑/↓:profile  Tab:focus  Enter:switch  n:new  d:del  e:edit  r:refresh  ?:help"
            }
            ViewMode::Legacy => {
                "q:quit  Tab:pane  ↑/↓:nav  Enter:switch  n:new  d:del  e:edit  r:refresh  ?:help"
            }
            #[cfg(feature = "tui-cards")]
            ViewMode::Cards => {
                "q:quit  ←/→:harness  ↑/↓:profile  Enter:switch  n:new  d:del  e:edit  r:refresh  ?:help"
            }
        }
    }

    fn mode_indicator(&self) -> &'static str {
        match self.view_mode {
            ViewMode::Dashboard => "[Dashboard]",
            ViewMode::Legacy => "[Legacy]",
            #[cfg(feature = "tui-cards")]
            ViewMode::Cards => "[Cards]",
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let msg = self.message.unwrap_or("");

        let mut spans = vec![
            Span::styled(self.mode_indicator(), Theme::tab_selected()),
            Span::raw(" "),
        ];

        if let Some(status) = self.harness_status {
            spans.push(Span::styled(format!("[{}]", status), Theme::text_muted()));
            spans.push(Span::raw(" "));
        }

        spans.push(Span::styled(self.keybindings(), Theme::text_muted()));
        spans.push(Span::raw("  "));
        spans.push(Span::styled(msg, Theme::text_warning()));

        let paragraph = Paragraph::new(Line::from(spans));
        paragraph.render(area, buf);
    }
}
