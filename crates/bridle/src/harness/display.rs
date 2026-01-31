//! Display information extraction from harness configs.

/// Information for displaying harness status.
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// Harness name.
    pub name: String,

    /// Whether the harness is detected/installed.
    pub installed: bool,

    /// Current status description.
    pub status: String,
}

impl DisplayInfo {
    /// Create new display info.
    pub fn new(name: impl Into<String>, installed: bool, status: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            installed,
            status: status.into(),
        }
    }
}
