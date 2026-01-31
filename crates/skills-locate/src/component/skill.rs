use serde::de::Error as _;

use crate::{Error, Result, SkillDescriptor};

pub fn parse_skill_descriptor(content: &str) -> Result<SkillDescriptor> {
    let content = content.replace("\r\n", "\n");

    if !content.starts_with("---\n") {
        return Err(Error::YamlParse(serde_yaml::Error::custom(
            "missing frontmatter",
        )));
    }

    let after_opener = &content[4..];

    let yaml_end = if after_opener.starts_with("---\n") {
        0
    } else if let Some(pos) = after_opener.find("\n---\n") {
        pos
    } else if after_opener.ends_with("\n---") {
        after_opener.len() - 4
    } else if after_opener == "---" {
        0
    } else {
        return Err(Error::YamlParse(serde_yaml::Error::custom(
            "unclosed frontmatter",
        )));
    };

    let yaml_content = &after_opener[..yaml_end];

    let descriptor: SkillDescriptor = serde_yaml::from_str(yaml_content)?;

    if descriptor.name.is_empty() {
        return Err(Error::YamlParse(serde_yaml::Error::custom(
            "missing required field: name",
        )));
    }

    Ok(descriptor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_skill() {
        let content =
            "---\nname: test-skill\ndescription: A test\ntriggers:\n  - /test\n---\n# Body";
        let desc = parse_skill_descriptor(content).unwrap();
        assert_eq!(desc.name, "test-skill");
        assert_eq!(desc.description, Some("A test".to_string()));
        assert_eq!(desc.triggers, vec!["/test"]);
    }

    #[test]
    fn parse_minimal_skill() {
        let content = "---\nname: minimal\n---\nBody";
        let desc = parse_skill_descriptor(content).unwrap();
        assert_eq!(desc.name, "minimal");
        assert_eq!(desc.description, None);
        assert!(desc.triggers.is_empty());
    }

    #[test]
    fn parse_crlf_line_endings() {
        let content = "---\r\nname: crlf-test\r\n---\r\nBody";
        let desc = parse_skill_descriptor(content).unwrap();
        assert_eq!(desc.name, "crlf-test");
    }

    #[test]
    fn error_missing_frontmatter() {
        let content = "# No frontmatter";
        assert!(parse_skill_descriptor(content).is_err());
    }

    #[test]
    fn error_missing_name() {
        let content = "---\ndescription: no name\n---\nBody";
        assert!(parse_skill_descriptor(content).is_err());
    }

    #[test]
    fn error_empty_name() {
        let content = "---\nname: \"\"\n---\nBody";
        assert!(parse_skill_descriptor(content).is_err());
    }
}
