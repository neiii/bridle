//! Core type definitions for skills discovery.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Source location for a plugin.
///
/// Plugins can be sourced from GitHub repositories, direct URLs,
/// or relative paths within a marketplace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum PluginSource {
    /// GitHub repository reference.
    GitHub {
        /// GitHub URL or owner/repo shorthand.
        #[serde(alias = "repo")]
        github: String,
    },
    /// Direct URL to plugin.
    Url {
        /// Full URL to the plugin location.
        url: String,
    },
    /// Relative path within a marketplace repository.
    Relative(String),
}

/// Plugin descriptor containing metadata and skills.
///
/// Represents a plugin as discovered from a repository,
/// including its name, description, and contained skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PluginDescriptor {
    /// Plugin name.
    pub name: String,

    /// Path where plugin was discovered (e.g., "plugins/code-review").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Optional description of the plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Skills contained in this plugin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<SkillDescriptor>,

    /// Commands contained in this plugin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<crate::component::CommandDescriptor>,

    /// Agents contained in this plugin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<crate::component::AgentDescriptor>,

    /// Hooks configuration from hooks.json.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<crate::component::HooksConfig>,

    /// MCP server descriptors from .mcp.json, keyed by server name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mcp_servers: HashMap<String, crate::component::McpServer>,
}

/// Skill metadata descriptor.
///
/// Contains metadata extracted from SKILL.md frontmatter,
/// without the full skill body content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkillDescriptor {
    /// Skill name (required).
    pub name: String,

    /// Optional description of the skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Trigger patterns that invoke this skill.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<String>,
}

/// Result of plugin discovery with both grouped and flat access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DiscoveryResult {
    /// All discovered plugins, grouped with their components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginDescriptor>,

    /// Flat list of all skills across all plugins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_skills: Vec<SkillDescriptor>,

    /// Flat list of all commands across all plugins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_commands: Vec<crate::component::CommandDescriptor>,

    /// Flat list of all agents across all plugins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub all_agents: Vec<crate::component::AgentDescriptor>,

    /// Flat list of all MCP servers across all plugins, keyed by server name.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub all_mcp_servers: HashMap<String, crate::component::McpServer>,
}

impl DiscoveryResult {
    /// Create from a list of plugins, populating flat lists.
    #[must_use]
    pub fn from_plugins(plugins: Vec<PluginDescriptor>) -> Self {
        let all_skills = plugins.iter().flat_map(|p| p.skills.clone()).collect();
        let all_commands = plugins.iter().flat_map(|p| p.commands.clone()).collect();
        let all_agents = plugins.iter().flat_map(|p| p.agents.clone()).collect();
        let all_mcp_servers = plugins.iter().flat_map(|p| p.mcp_servers.clone()).collect();

        Self {
            plugins,
            all_skills,
            all_commands,
            all_agents,
            all_mcp_servers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_source_github_serde_roundtrip() {
        let source = PluginSource::GitHub {
            github: "anthropics/claude-code".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, r#"{"github":"anthropics/claude-code"}"#);
        let parsed: PluginSource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, source);
    }

    #[test]
    fn plugin_source_github_deserializes_from_repo_alias() {
        let json = r#"{"repo":"owner/repo"}"#;
        let parsed: PluginSource = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed,
            PluginSource::GitHub {
                github: "owner/repo".to_string()
            }
        );
    }

    #[test]
    fn plugin_source_url_serde_roundtrip() {
        let source = PluginSource::Url {
            url: "https://example.com/plugin".to_string(),
        };
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, r#"{"url":"https://example.com/plugin"}"#);
        let parsed: PluginSource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, source);
    }

    #[test]
    fn plugin_source_relative_serde_roundtrip() {
        let source = PluginSource::Relative("./plugins/my-plugin".to_string());
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, r#""./plugins/my-plugin""#);
        let parsed: PluginSource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, source);
    }

    #[test]
    fn plugin_descriptor_full_serde_roundtrip() {
        let plugin = PluginDescriptor {
            name: "test-plugin".to_string(),
            path: Some("plugins/test".to_string()),
            description: Some("A test plugin".to_string()),
            skills: vec![SkillDescriptor {
                name: "test-skill".to_string(),
                description: Some("A test skill".to_string()),
                triggers: vec!["/test".to_string()],
            }],
            commands: vec![],
            agents: vec![],
            hooks: None,
            mcp_servers: HashMap::new(),
        };
        let json = serde_json::to_string(&plugin).unwrap();
        let parsed: PluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, plugin);
    }

    #[test]
    fn plugin_descriptor_minimal_serde_roundtrip() {
        let plugin = PluginDescriptor {
            name: "minimal".to_string(),
            path: None,
            description: None,
            skills: vec![],
            commands: vec![],
            agents: vec![],
            hooks: None,
            mcp_servers: HashMap::new(),
        };
        let json = serde_json::to_string(&plugin).unwrap();
        let parsed: PluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, plugin);
    }

    #[test]
    fn plugin_descriptor_serde_omits_optional_fields() {
        let plugin = PluginDescriptor {
            name: "minimal".to_string(),
            path: None,
            description: None,
            skills: vec![],
            commands: vec![],
            agents: vec![],
            hooks: None,
            mcp_servers: HashMap::new(),
        };
        let json = serde_json::to_string(&plugin).unwrap();
        assert_eq!(json, r#"{"name":"minimal"}"#);
        let parsed: PluginDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, plugin);
    }

    #[test]
    fn skill_descriptor_full_serde_roundtrip() {
        let skill = SkillDescriptor {
            name: "code-review".to_string(),
            description: Some("Reviews code for issues".to_string()),
            triggers: vec!["/review".to_string(), "/cr".to_string()],
        };
        let json = serde_json::to_string(&skill).unwrap();
        let parsed: SkillDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, skill);
    }

    #[test]
    fn skill_descriptor_minimal_serde_roundtrip() {
        let skill = SkillDescriptor {
            name: "minimal-skill".to_string(),
            description: None,
            triggers: vec![],
        };
        let json = serde_json::to_string(&skill).unwrap();
        assert_eq!(json, r#"{"name":"minimal-skill"}"#);
        let parsed: SkillDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, skill);
    }

    #[test]
    fn plugin_descriptor_deserialize_with_defaults() {
        // JSON with only required field
        let json = r#"{"name":"test"}"#;
        let plugin: PluginDescriptor = serde_json::from_str(json).unwrap();
        assert_eq!(plugin.name, "test");
        assert_eq!(plugin.description, None);
        assert!(plugin.skills.is_empty());
    }

    #[test]
    fn skill_descriptor_deserialize_with_defaults() {
        // JSON with only required field
        let json = r#"{"name":"test-skill"}"#;
        let skill: SkillDescriptor = serde_json::from_str(json).unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, None);
        assert!(skill.triggers.is_empty());
    }

    #[test]
    fn discovery_result_serde_roundtrip() {
        let result = DiscoveryResult {
            plugins: vec![PluginDescriptor {
                name: "test-plugin".to_string(),
                path: Some("plugins/test".to_string()),
                description: Some("A test plugin".to_string()),
                skills: vec![SkillDescriptor {
                    name: "skill-1".to_string(),
                    description: None,
                    triggers: vec![],
                }],
                commands: vec![],
                agents: vec![],
                hooks: None,
                mcp_servers: HashMap::new(),
            }],
            all_skills: vec![SkillDescriptor {
                name: "skill-1".to_string(),
                description: None,
                triggers: vec![],
            }],
            all_commands: vec![],
            all_agents: vec![],
            all_mcp_servers: HashMap::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: DiscoveryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, result);
    }

    #[test]
    fn discovery_result_from_plugins_flattens_components() {
        let plugins = vec![
            PluginDescriptor {
                name: "plugin-a".to_string(),
                path: Some("plugins/a".to_string()),
                description: None,
                skills: vec![SkillDescriptor {
                    name: "skill-1".to_string(),
                    description: None,
                    triggers: vec![],
                }],
                commands: vec![],
                agents: vec![],
                hooks: None,
                mcp_servers: HashMap::new(),
            },
            PluginDescriptor {
                name: "plugin-b".to_string(),
                path: Some("plugins/b".to_string()),
                description: None,
                skills: vec![SkillDescriptor {
                    name: "skill-2".to_string(),
                    description: None,
                    triggers: vec![],
                }],
                commands: vec![],
                agents: vec![],
                hooks: None,
                mcp_servers: HashMap::new(),
            },
        ];

        let result = DiscoveryResult::from_plugins(plugins);
        assert_eq!(result.plugins.len(), 2);
        assert_eq!(result.all_skills.len(), 2);
        assert_eq!(result.all_skills[0].name, "skill-1");
        assert_eq!(result.all_skills[1].name, "skill-2");
    }

    #[test]
    fn discovery_result_empty_serde_roundtrip() {
        let result = DiscoveryResult {
            plugins: vec![],
            all_skills: vec![],
            all_commands: vec![],
            all_agents: vec![],
            all_mcp_servers: HashMap::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert_eq!(json, "{}");
        let parsed: DiscoveryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, result);
    }
}
