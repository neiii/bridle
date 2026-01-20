use crate::harness::HarnessConfig;
use crate::tui::theme::Theme;
use harness_locate::{Harness, HarnessKind, InstallationStatus};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs, Widget},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HarnessStatus {
    Active,
    Installed,
    BinaryOnly,
    NotInstalled,
}

impl HarnessStatus {
    pub fn indicator(self) -> char {
        match self {
            Self::Active => '●',
            Self::Installed => '+',
            Self::BinaryOnly => '-',
            Self::NotInstalled => '○',
        }
    }

    pub fn style(self) -> Style {
        match self {
            Self::Active => Style::default().fg(Color::Green),
            Self::Installed => Theme::harness_installed(),
            Self::BinaryOnly | Self::NotInstalled => Theme::harness_not_installed(),
        }
    }
}

pub struct HarnessTabs<'a> {
    harnesses: &'a [HarnessKind],
    selected: usize,
    statuses: Vec<HarnessStatus>,
}

impl<'a> HarnessTabs<'a> {
    pub fn new(harnesses: &'a [HarnessKind], selected: usize) -> Self {
        let statuses = harnesses
            .iter()
            .map(|kind| {
                let harness = Harness::new(*kind);
                match harness.installation_status() {
                    Ok(InstallationStatus::FullyInstalled { .. })
                    | Ok(InstallationStatus::ConfigOnly { .. }) => HarnessStatus::Installed,
                    Ok(InstallationStatus::BinaryOnly { .. }) => HarnessStatus::BinaryOnly,
                    _ => HarnessStatus::NotInstalled,
                }
            })
            .collect();

        Self {
            harnesses,
            selected,
            statuses,
        }
    }

    pub fn with_active_indicator(mut self, harness_id: &str) -> Self {
        for (i, kind) in self.harnesses.iter().enumerate() {
            let h = Harness::new(*kind);
            if h.id() == harness_id {
                self.statuses[i] = HarnessStatus::Active;
            }
        }
        self
    }
}

impl Widget for HarnessTabs<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<Line> = self
            .harnesses
            .iter()
            .zip(self.statuses.iter())
            .map(|(kind, status)| {
                let harness = Harness::new(*kind);
                let name = harness.kind().to_string();
                let style = status.style();
                Line::from(vec![
                    Span::styled(format!("{} ", status.indicator()), style),
                    Span::styled(name, style),
                ])
            })
            .collect();

        let border_style = Style::default().fg(Color::DarkGray);

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .title(" Harnesses ")
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .select(self.selected)
            .style(Style::default())
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::raw(" │ "));

        tabs.render(area, buf);
    }
}
