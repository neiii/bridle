use serde::de::Error as _;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AgentDescriptor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentFrontmatter {
    name: String,
    description: Option<String>,
    #[serde(default)]
    tools: ToolsField,
    model: Option<String>,
    color: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum ToolsField {
    #[default]
    None,
    List(Vec<String>),
    CommaSeparated(String),
}

impl ToolsField {
    fn into_vec(self) -> Vec<String> {
        match self {
            ToolsField::None => Vec::new(),
            ToolsField::List(v) => v,
            ToolsField::CommaSeparated(s) => s.split(',').map(|t| t.trim().to_string()).collect(),
        }
    }
}

pub fn parse_agent_descriptor(content: &str) -> Result<AgentDescriptor> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Err(Error::YamlParse(serde_yaml::Error::custom(
            "agent file must start with YAML frontmatter",
        )));
    }

    let rest = &content[3..];
    let end = rest.find("---").ok_or_else(|| {
        Error::YamlParse(serde_yaml::Error::custom("unterminated YAML frontmatter"))
    })?;

    let yaml = &rest[..end];
    let frontmatter: AgentFrontmatter = serde_yaml::from_str(yaml)?;

    if frontmatter.name.is_empty() {
        return Err(Error::YamlParse(serde_yaml::Error::custom(
            "agent name cannot be empty",
        )));
    }

    Ok(AgentDescriptor {
        name: frontmatter.name,
        description: frontmatter.description,
        tools: frontmatter.tools.into_vec(),
        model: frontmatter.model,
        color: frontmatter.color,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_agent_frontmatter() {
        let content = r#"---
name: code-explorer
description: Explores codebases
tools: read, grep, glob
model: sonnet
color: yellow
---
Instructions here
"#;
        let agent = parse_agent_descriptor(content).unwrap();
        assert_eq!(agent.name, "code-explorer");
        assert_eq!(agent.description.as_deref(), Some("Explores codebases"));
        assert_eq!(agent.tools, vec!["read", "grep", "glob"]);
        assert_eq!(agent.model.as_deref(), Some("sonnet"));
        assert_eq!(agent.color.as_deref(), Some("yellow"));
    }

    #[test]
    fn parses_tools_as_list() {
        let content = r#"---
name: test-agent
tools:
  - read
  - write
---
"#;
        let agent = parse_agent_descriptor(content).unwrap();
        assert_eq!(agent.tools, vec!["read", "write"]);
    }

    #[test]
    fn handles_empty_tools() {
        let content = r#"---
name: minimal-agent
---
"#;
        let agent = parse_agent_descriptor(content).unwrap();
        assert!(agent.tools.is_empty());
    }

    #[test]
    fn rejects_missing_name() {
        let content = r#"---
description: No name provided
---
"#;
        assert!(parse_agent_descriptor(content).is_err());
    }

    #[test]
    fn rejects_empty_name() {
        let content = r#"---
name: ""
---
"#;
        assert!(parse_agent_descriptor(content).is_err());
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let content = "Just plain markdown";
        assert!(parse_agent_descriptor(content).is_err());
    }
}
