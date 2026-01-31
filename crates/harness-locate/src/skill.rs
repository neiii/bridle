//! Skill file parsing utilities.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Parsed frontmatter result.
#[derive(Debug, Clone, PartialEq)]
pub struct Frontmatter<'a> {
    /// Parsed YAML frontmatter, if present.
    pub yaml: Option<serde_yaml::Value>,
    /// The markdown body after the frontmatter.
    pub body: &'a str,
}

/// A parsed skill file with typed frontmatter fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    /// The skill name (required).
    pub name: String,
    /// Optional description of the skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Trigger phrases that activate this skill.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<String>,
    /// The markdown body content.
    #[serde(skip)]
    pub body: String,
    /// Additional frontmatter fields not captured above.
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_yaml::Value>,
}

/// Parse YAML frontmatter from markdown content.
///
/// # Errors
///
/// Returns `Error::YamlParse` if frontmatter exists but contains invalid YAML.
pub fn parse_frontmatter(content: &str) -> Result<Frontmatter<'_>> {
    let (opener, line_ending) = if content.starts_with("---\r\n") {
        ("---\r\n", "\r\n")
    } else if content.starts_with("---\n") {
        ("---\n", "\n")
    } else {
        return Ok(Frontmatter {
            yaml: None,
            body: content,
        });
    };

    let after_opener = &content[opener.len()..];
    let empty_closer = format!("---{line_ending}");
    let closer = format!("{line_ending}---{line_ending}");
    let closer_eof = format!("{line_ending}---");

    let (yaml_content, body) = if after_opener.starts_with(&empty_closer) {
        ("", &after_opener[empty_closer.len()..])
    } else if let Some(pos) = after_opener.find(&closer) {
        (&after_opener[..pos], &after_opener[pos + closer.len()..])
    } else if after_opener.ends_with(&closer_eof) {
        (&after_opener[..after_opener.len() - closer_eof.len()], "")
    } else if after_opener == "---" {
        ("", "")
    } else {
        return Ok(Frontmatter {
            yaml: None,
            body: content,
        });
    };

    let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
    Ok(Frontmatter {
        yaml: Some(yaml_value),
        body,
    })
}

/// Parse a skill file from markdown content with YAML frontmatter.
///
/// # Errors
///
/// Returns `Error::MissingField` if the required `name` field is missing.
/// Returns `Error::YamlParse` if frontmatter contains invalid YAML.
pub fn parse_skill(content: &str) -> Result<Skill> {
    let frontmatter = parse_frontmatter(content)?;

    let yaml = frontmatter
        .yaml
        .ok_or_else(|| Error::MissingField("name".to_string()))?;

    let mut skill: Skill = serde_yaml::from_value(yaml)?;
    skill.body = frontmatter.body.to_string();
    Ok(skill)
}

impl Skill {
    /// Convert the skill back to markdown format with YAML frontmatter.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let yaml = serde_yaml::to_string(self).unwrap_or_default();
        let yaml_trimmed = yaml.trim_end();
        format!("---\n{yaml_trimmed}\n---\n{}", self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_frontmatter() {
        let content = "---\nname: test\nversion: 1\n---\n# Body\n";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.yaml.is_some());
        let yaml = result.yaml.unwrap();
        assert_eq!(yaml["name"], "test");
        assert_eq!(yaml["version"], 1);
        assert_eq!(result.body, "# Body\n");
    }

    #[test]
    fn returns_none_without_frontmatter() {
        let content = "# Just Markdown\nNo frontmatter here.";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.yaml.is_none());
        assert_eq!(result.body, content);
    }

    #[test]
    fn handles_empty_frontmatter() {
        let content = "---\n---\nBody content";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.yaml.is_some());
        assert_eq!(result.yaml.unwrap(), serde_yaml::Value::Null);
        assert_eq!(result.body, "Body content");
    }

    #[test]
    fn returns_error_for_malformed_yaml() {
        let content = "---\ninvalid: yaml: content:\n---\nBody";
        let result = parse_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn handles_crlf_line_endings() {
        let content = "---\r\nname: test\r\n---\r\nBody";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.yaml.is_some());
        assert_eq!(result.yaml.unwrap()["name"], "test");
        assert_eq!(result.body, "Body");
    }

    #[test]
    fn preserves_horizontal_rules_in_body() {
        let content = "---\nname: test\n---\n# Title\n\n---\n\nMore content";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.yaml.is_some());
        assert!(result.body.contains("---"));
        assert!(result.body.contains("More content"));
    }

    #[test]
    fn handles_empty_body() {
        let content = "---\nname: test\n---\n";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.yaml.is_some());
        assert_eq!(result.body, "");
    }

    #[test]
    fn handles_frontmatter_at_eof() {
        let content = "---\nname: test\n---";
        let result = parse_frontmatter(content).unwrap();
        assert!(result.yaml.is_some());
        assert_eq!(result.body, "");
    }

    #[test]
    fn parse_skill_with_all_fields() {
        let content = "---\nname: my-skill\ndescription: A test skill\ntriggers:\n  - hello\n  - hi\ncustom_key: custom_value\n---\n# Body content\n";
        let skill = parse_skill(content).unwrap();

        assert_eq!(skill.name, "my-skill");
        assert_eq!(skill.description, Some("A test skill".to_string()));
        assert_eq!(skill.triggers, vec!["hello", "hi"]);
        assert_eq!(skill.body, "# Body content\n");
        assert!(skill.metadata.contains_key("custom_key"));
    }

    #[test]
    fn parse_skill_with_minimal_fields() {
        let content = "---\nname: minimal\n---\nBody";
        let skill = parse_skill(content).unwrap();

        assert_eq!(skill.name, "minimal");
        assert_eq!(skill.description, None);
        assert!(skill.triggers.is_empty());
        assert_eq!(skill.body, "Body");
        assert!(skill.metadata.is_empty());
    }

    #[test]
    fn parse_skill_captures_unknown_keys() {
        let content = "---\nname: test\nfoo: bar\nnested:\n  a: 1\n  b: 2\n---\n";
        let skill = parse_skill(content).unwrap();

        assert_eq!(skill.metadata.get("foo").unwrap(), "bar");
        assert!(skill.metadata.contains_key("nested"));
    }

    #[test]
    fn parse_skill_fails_without_name() {
        let content = "---\ndescription: no name here\n---\nBody";
        let result = parse_skill(content);
        assert!(result.is_err());
    }

    #[test]
    fn parse_skill_fails_without_frontmatter() {
        let content = "# Just markdown\nNo frontmatter";
        let result = parse_skill(content);
        assert!(result.is_err());
    }

    #[test]
    fn parse_skill_handles_empty_body() {
        let content = "---\nname: empty-body\n---\n";
        let skill = parse_skill(content).unwrap();

        assert_eq!(skill.name, "empty-body");
        assert_eq!(skill.body, "");
    }

    #[test]
    fn skill_round_trip_preserves_content() {
        let content = "---\nname: roundtrip\ndescription: Test round trip\ntriggers:\n  - test\n---\n# Hello\n\nWorld\n";
        let skill = parse_skill(content).unwrap();
        let markdown = skill.to_markdown();
        let reparsed = parse_skill(&markdown).unwrap();

        assert_eq!(skill.name, reparsed.name);
        assert_eq!(skill.description, reparsed.description);
        assert_eq!(skill.triggers, reparsed.triggers);
        assert_eq!(skill.body, reparsed.body);
    }
}
