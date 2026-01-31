use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::TableState,
};

use crate::config::ProfileInfo;
use crate::tui::widgets::{DetailPane, ProfileTable};

#[allow(dead_code)]
pub struct DashboardView;

impl DashboardView {
    #[allow(dead_code)]
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        profiles: &[ProfileInfo],
        table_state: &mut TableState,
        detail_focused: bool,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let profile_table = ProfileTable::new(profiles);
        frame.render_stateful_widget(profile_table, chunks[0], table_state);

        let selected_profile = table_state.selected().and_then(|idx| profiles.get(idx));

        let detail_pane = DetailPane::new(selected_profile).focused(detail_focused);
        frame.render_widget(detail_pane, chunks[1]);
    }
}
