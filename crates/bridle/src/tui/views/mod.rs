mod dashboard;

#[cfg(feature = "tui-cards")]
mod cards;

#[allow(unused_imports)]
pub use dashboard::DashboardView;

#[cfg(feature = "tui-cards")]
pub use cards::CardView;

use crate::config::ViewPreference;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Legacy,
    #[default]
    Dashboard,
    #[cfg(feature = "tui-cards")]
    Cards,
}

impl ViewMode {
    #[allow(dead_code)]
    pub fn from_config(pref: ViewPreference) -> Self {
        match pref {
            ViewPreference::Legacy => ViewMode::Legacy,
            ViewPreference::Dashboard => ViewMode::Dashboard,
            #[cfg(feature = "tui-cards")]
            ViewPreference::Cards => ViewMode::Cards,
        }
    }

    pub fn toggle(&mut self) {
        *self = match self {
            ViewMode::Legacy => ViewMode::Dashboard,
            #[cfg(feature = "tui-cards")]
            ViewMode::Dashboard => ViewMode::Cards,
            #[cfg(not(feature = "tui-cards"))]
            ViewMode::Dashboard => ViewMode::Legacy,
            #[cfg(feature = "tui-cards")]
            ViewMode::Cards => ViewMode::Legacy,
        };
    }

    pub fn name(&self) -> &'static str {
        match self {
            ViewMode::Legacy => "Legacy",
            ViewMode::Dashboard => "Dashboard",
            #[cfg(feature = "tui-cards")]
            ViewMode::Cards => "Cards",
        }
    }
}
