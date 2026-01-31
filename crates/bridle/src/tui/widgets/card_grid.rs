#![cfg(feature = "tui-cards")]

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::config::ProfileInfo;

use super::profile_card::{NewProfileCard, ProfileCard};

const CARD_WIDTH: u16 = 30;
const CARD_HEIGHT: u16 = 8;
const CARD_GAP: u16 = 1;

pub struct CardGridState {
    pub selected: usize,
    pub scroll_offset: usize,
}

impl CardGridState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            scroll_offset: 0,
        }
    }

    pub fn select_next(&mut self, total: usize) {
        if total > 0 && self.selected < total - 1 {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn select_down(&mut self, cols: usize, total: usize) {
        let next = self.selected + cols;
        if next < total {
            self.selected = next;
        }
    }

    pub fn select_up(&mut self, cols: usize) {
        if self.selected >= cols {
            self.selected -= cols;
        }
    }
}

impl Default for CardGridState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CardGrid<'a> {
    profiles: &'a [ProfileInfo],
    show_new_card: bool,
}

impl<'a> CardGrid<'a> {
    pub fn new(profiles: &'a [ProfileInfo]) -> Self {
        Self {
            profiles,
            show_new_card: true,
        }
    }

    pub fn show_new_card(mut self, show: bool) -> Self {
        self.show_new_card = show;
        self
    }

    fn calc_columns(&self, width: u16) -> usize {
        let usable = width.saturating_sub(CARD_GAP);
        let card_with_gap = CARD_WIDTH + CARD_GAP;
        (usable / card_with_gap).max(1) as usize
    }
}

impl StatefulWidget for CardGrid<'_> {
    type State = CardGridState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width < CARD_WIDTH || area.height < CARD_HEIGHT {
            return;
        }

        let cols = self.calc_columns(area.width);
        let total_items = self.profiles.len() + if self.show_new_card { 1 } else { 0 };

        if state.selected >= total_items && total_items > 0 {
            state.selected = total_items - 1;
        }

        let visible_rows = (area.height / (CARD_HEIGHT + CARD_GAP)) as usize;
        let selected_row = state.selected / cols;

        if selected_row < state.scroll_offset {
            state.scroll_offset = selected_row;
        } else if selected_row >= state.scroll_offset + visible_rows {
            state.scroll_offset = selected_row - visible_rows + 1;
        }

        for row in 0..visible_rows {
            let data_row = state.scroll_offset + row;
            for col in 0..cols {
                let idx = data_row * cols + col;
                if idx >= total_items {
                    break;
                }

                let x = area.x + (col as u16) * (CARD_WIDTH + CARD_GAP);
                let y = area.y + (row as u16) * (CARD_HEIGHT + CARD_GAP);

                if x + CARD_WIDTH > area.x + area.width || y + CARD_HEIGHT > area.y + area.height {
                    continue;
                }

                let card_area = Rect::new(x, y, CARD_WIDTH, CARD_HEIGHT);
                let is_selected = idx == state.selected;

                if idx < self.profiles.len() {
                    let profile = &self.profiles[idx];
                    ProfileCard::new(profile)
                        .selected(profile.is_active)
                        .focused(is_selected)
                        .render(card_area, buf);
                } else {
                    NewProfileCard::new()
                        .focused(is_selected)
                        .render(card_area, buf);
                }
            }
        }
    }
}
