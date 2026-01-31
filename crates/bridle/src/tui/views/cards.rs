#![cfg(feature = "tui-cards")]

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::StatefulWidget,
};

use crate::config::ProfileInfo;
use crate::tui::widgets::{CardGrid, CardGridState};

pub struct CardViewState {
    pub grid_state: CardGridState,
}

impl CardViewState {
    pub fn new() -> Self {
        Self {
            grid_state: CardGridState::new(),
        }
    }
}

impl Default for CardViewState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CardView<'a> {
    profiles: &'a [ProfileInfo],
}

impl<'a> CardView<'a> {
    pub fn new(profiles: &'a [ProfileInfo]) -> Self {
        Self { profiles }
    }
}

impl StatefulWidget for CardView<'_> {
    type State = CardViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::vertical([Constraint::Min(0)]).split(area);

        CardGrid::new(self.profiles).show_new_card(true).render(
            chunks[0],
            buf,
            &mut state.grid_state,
        );
    }
}
