//! Hook types and parsing for plugin hooks.json files.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Hook event types that trigger hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[non_exhaustive]
pub enum HookEvent {
    /// Before a tool is used.
    PreToolUse,
    /// After a tool is used.
    PostToolUse,
    /// On notification events.
    Notification,
    /// When the agent stops.
    Stop,
    /// When a subagent stops.
    SubagentStop,
}

/// A hook action to execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum HookAction {
    /// Simple command string.
    Simple(String),
    /// Command with options.
    Extended {
        /// Command to execute.
        command: String,
        /// Timeout in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<u64>,
        /// Run in background.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background: Option<bool>,
    },
}

/// A group of hooks with optional matcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HookGroup {
    /// Optional matcher pattern (e.g., tool name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    /// Hooks to execute.
    pub hooks: Vec<HookAction>,
}

/// Parsed hooks.json file structure.
pub type HooksConfig = HashMap<HookEvent, Vec<HookGroup>>;

/// Parse a hooks.json file content into a HooksConfig.
pub fn parse_hooks_json(content: &str) -> Result<HooksConfig> {
    serde_json::from_str(content).map_err(Error::JsonParse)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_event_serde_roundtrip() {
        let event = HookEvent::PreToolUse;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""PreToolUse""#);
        let parsed: HookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn hook_action_simple_serde() {
        let action = HookAction::Simple("echo hello".to_string());
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, r#""echo hello""#);
        let parsed: HookAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn hook_action_extended_serde() {
        let action = HookAction::Extended {
            command: "npm test".to_string(),
            timeout: Some(30000),
            background: Some(true),
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: HookAction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, action);
    }

    #[test]
    fn hook_group_with_matcher() {
        let group = HookGroup {
            matcher: Some("Edit".to_string()),
            hooks: vec![HookAction::Simple("lint".to_string())],
        };
        let json = serde_json::to_string(&group).unwrap();
        let parsed: HookGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, group);
    }

    #[test]
    fn parse_full_hooks_json() {
        let content = r#"{
            "PreToolUse": [
                {
                    "matcher": "Edit",
                    "hooks": ["pre-edit-check"]
                }
            ],
            "PostToolUse": [
                {
                    "hooks": [
                        {"command": "npm test", "timeout": 30000}
                    ]
                }
            ]
        }"#;
        let config = parse_hooks_json(content).unwrap();
        assert!(config.contains_key(&HookEvent::PreToolUse));
        assert!(config.contains_key(&HookEvent::PostToolUse));
        assert_eq!(config[&HookEvent::PreToolUse].len(), 1);
    }

    #[test]
    fn parse_empty_hooks_json() {
        let content = "{}";
        let config = parse_hooks_json(content).unwrap();
        assert!(config.is_empty());
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let content = "not json";
        assert!(parse_hooks_json(content).is_err());
    }
}
