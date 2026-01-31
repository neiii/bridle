use serde::de::Error as _;

use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CommandDescriptor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CommandFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    allowed_tools: Vec<String>,
}

pub fn parse_command_descriptor(content: &str, filename: &str) -> Result<CommandDescriptor> {
    let content = content.replace("\r\n", "\n");

    if !content.starts_with("---\n") {
        return Err(Error::YamlParse(serde_yaml::Error::custom(
            "missing frontmatter",
        )));
    }

    let after_opener = &content[4..];
    let yaml_end = after_opener
        .find("\n---")
        .ok_or_else(|| Error::YamlParse(serde_yaml::Error::custom("unclosed frontmatter")))?;

    let yaml = &after_opener[..yaml_end];
    let frontmatter: CommandFrontmatter = serde_yaml::from_str(yaml)?;

    let name = frontmatter
        .name
        .unwrap_or_else(|| derive_name_from_filename(filename));

    if name.is_empty() {
        return Err(Error::YamlParse(serde_yaml::Error::custom(
            "command name cannot be empty",
        )));
    }

    Ok(CommandDescriptor {
        name,
        description: frontmatter.description,
        allowed_tools: frontmatter.allowed_tools,
    })
}

fn derive_name_from_filename(filename: &str) -> String {
    filename
        .trim_end_matches(".md")
        .trim_end_matches(".MD")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_with_explicit_name() {
        let content = r#"---
name: my-command
description: Does something
allowed_tools:
  - Read
  - Edit
---
# Command body
"#;
        let cmd = parse_command_descriptor(content, "other.md").unwrap();
        assert_eq!(cmd.name, "my-command");
        assert_eq!(cmd.description, Some("Does something".into()));
        assert_eq!(cmd.allowed_tools, vec!["Read", "Edit"]);
    }

    #[test]
    fn derives_name_from_filename_when_not_in_frontmatter() {
        let content = r#"---
description: A command
---
body
"#;
        let cmd = parse_command_descriptor(content, "do-stuff.md").unwrap();
        assert_eq!(cmd.name, "do-stuff");
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let content = "# No frontmatter";
        assert!(parse_command_descriptor(content, "test.md").is_err());
    }

    #[test]
    fn rejects_empty_name() {
        let content = "---\nname: \"\"\n---\nbody\n";
        assert!(parse_command_descriptor(content, ".md").is_err());
    }
}
