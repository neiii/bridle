//! Skill discovery from GitHub repositories.
//!
//! Wraps the `skills-locate` crate to discover installable skills.

use std::collections::HashMap;

use harness_locate::McpServer;
use skills_locate::parse_mcp_json;
use skills_locate::{GitHubRef, extract_file, fetch_bytes, list_files, parse_skill_descriptor};
use thiserror::Error;

use super::types::{AgentInfo, CommandInfo, DiscoveryResult, SkillInfo, SourceInfo};

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Invalid GitHub URL: {0}")]
    InvalidUrl(String),

    #[error("Failed to fetch repository: {0}")]
    FetchError(#[source] skills_locate::Error),

    #[error("No skills found in repository")]
    NoSkillsFound,
}

pub fn discover_skills(url: &str) -> Result<DiscoveryResult, DiscoveryError> {
    let github_ref =
        GitHubRef::parse(url).map_err(|e| DiscoveryError::InvalidUrl(e.to_string()))?;

    let source = SourceInfo {
        owner: github_ref.owner.clone(),
        repo: github_ref.repo.clone(),
        git_ref: Some(github_ref.git_ref.clone()),
    };

    let archive_url = github_ref.archive_url();
    let zip_bytes = fetch_bytes(&archive_url).map_err(DiscoveryError::FetchError)?;

    let skill_paths = list_files(&zip_bytes, "SKILL.md").map_err(DiscoveryError::FetchError)?;

    let mut skills = Vec::new();
    for path in skill_paths {
        let content = match extract_file(&zip_bytes, &path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let descriptor = match parse_skill_descriptor(&content) {
            Ok(d) => d,
            Err(_) => continue,
        };

        skills.push(SkillInfo {
            name: descriptor.name,
            description: descriptor.description,
            path: normalize_archive_path(&path, &github_ref),
            content,
        });
    }

    let mcp_paths = list_files(&zip_bytes, ".mcp.json").map_err(DiscoveryError::FetchError)?;

    let mut mcp_servers: HashMap<String, McpServer> = HashMap::new();
    for path in mcp_paths {
        let content = match extract_file(&zip_bytes, &path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Ok(servers) = parse_mcp_json(&content) {
            mcp_servers.extend(servers);
        }
    }

    // Discover agents from AGENT.md files (legacy format)
    let agent_paths = list_files(&zip_bytes, "AGENT.md").map_err(DiscoveryError::FetchError)?;

    let mut agents = Vec::new();
    for path in agent_paths {
        let content = match extract_file(&zip_bytes, &path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(agent) = parse_agent_frontmatter(&content, &path) {
            agents.push(AgentInfo {
                name: agent.0,
                description: agent.1,
                path: normalize_archive_path(&path, &github_ref),
                content,
            });
        }
    }

    // Discover agents from */agents/*.md directories (claude-code format)
    let all_md_paths = list_files(&zip_bytes, ".md").map_err(DiscoveryError::FetchError)?;
    for path in &all_md_paths {
        if !is_in_agents_dir(path) {
            continue;
        }
        let content = match extract_file(&zip_bytes, path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(agent) = parse_agent_frontmatter(&content, path) {
            agents.push(AgentInfo {
                name: agent.0,
                description: agent.1,
                path: normalize_archive_path(path, &github_ref),
                content,
            });
        }
    }

    // Discover commands from COMMAND.md files (legacy format)
    let command_paths = list_files(&zip_bytes, "COMMAND.md").map_err(DiscoveryError::FetchError)?;

    let mut commands = Vec::new();
    for path in command_paths {
        let content = match extract_file(&zip_bytes, &path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(cmd) = parse_command_frontmatter(&content, &path) {
            commands.push(CommandInfo {
                name: cmd.0,
                description: cmd.1,
                path: normalize_archive_path(&path, &github_ref),
                content,
            });
        }
    }

    // Discover commands from */commands/*.md directories (claude-code format)
    for path in &all_md_paths {
        if !is_in_commands_dir(path) {
            continue;
        }
        let content = match extract_file(&zip_bytes, path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(cmd) = parse_command_frontmatter(&content, path) {
            commands.push(CommandInfo {
                name: cmd.0,
                description: cmd.1,
                path: normalize_archive_path(path, &github_ref),
                content,
            });
        }
    }

    if skills.is_empty() && mcp_servers.is_empty() && agents.is_empty() && commands.is_empty() {
        return Err(DiscoveryError::NoSkillsFound);
    }

    Ok(DiscoveryResult {
        skills,
        mcp_servers,
        agents,
        commands,
        source,
    })
}

fn parse_agent_frontmatter(content: &str, path: &str) -> Option<(String, Option<String>)> {
    parse_yaml_frontmatter(content, filename_stem(path))
}

fn parse_command_frontmatter(content: &str, path: &str) -> Option<(String, Option<String>)> {
    parse_yaml_frontmatter(content, filename_stem(path))
}

fn parse_yaml_frontmatter(
    content: &str,
    fallback_name: Option<&str>,
) -> Option<(String, Option<String>)> {
    let content = content.trim();
    if !content.starts_with("---") {
        return fallback_name.map(|n| (n.to_string(), None));
    }

    let end = content[3..].find("---")?;
    let yaml_content = &content[3..3 + end];

    #[derive(serde::Deserialize)]
    struct Frontmatter {
        name: Option<String>,
        description: Option<String>,
    }

    let fm: Frontmatter = serde_yaml::from_str(yaml_content).ok()?;
    let name = fm.name.or_else(|| fallback_name.map(String::from))?;
    Some((name, fm.description))
}

fn filename_stem(path: &str) -> Option<&str> {
    path.rsplit('/').next()?.strip_suffix(".md")
}

fn normalize_archive_path(archive_path: &str, github_ref: &GitHubRef) -> String {
    let prefix = format!("{}-{}/", github_ref.repo, github_ref.git_ref);
    archive_path
        .strip_prefix(&prefix)
        .unwrap_or(archive_path)
        .to_string()
}

fn is_in_agents_dir(path: &str) -> bool {
    path.contains("/agents/") && path.ends_with(".md") && !path.ends_with("AGENT.md")
}

fn is_in_commands_dir(path: &str) -> bool {
    path.contains("/commands/") && path.ends_with(".md") && !path.ends_with("COMMAND.md")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_skills_invalid_url() {
        let result = discover_skills("https://gitlab.com/owner/repo");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DiscoveryError::InvalidUrl(_)));
    }

    #[test]
    fn discover_skills_missing_owner() {
        let result = discover_skills("https://github.com/");
        assert!(result.is_err());
    }

    #[test]
    fn normalize_path_strips_prefix() {
        let github_ref = GitHubRef::parse("https://github.com/owner/my-repo").unwrap();
        let path = "my-repo-main/skills/test/SKILL.md";
        assert_eq!(
            normalize_archive_path(path, &github_ref),
            "skills/test/SKILL.md"
        );
    }

    #[test]
    fn normalize_path_handles_no_prefix() {
        let github_ref = GitHubRef::parse("https://github.com/owner/repo").unwrap();
        let path = "other/skills/SKILL.md";
        assert_eq!(
            normalize_archive_path(path, &github_ref),
            "other/skills/SKILL.md"
        );
    }

    #[test]
    fn parse_mcp_wrapper_format() {
        let content = r#"{
            "mcpServers": {
                "filesystem": {"command": "npx", "args": ["-y", "@anthropic/mcp-filesystem"]},
                "web": {"type": "sse", "url": "https://example.com/mcp"}
            }
        }"#;
        let servers = skills_locate::parse_mcp_json(content).unwrap();
        assert_eq!(servers.len(), 2);
        assert!(servers.contains_key("filesystem"));
        assert!(servers.contains_key("web"));
    }

    #[test]
    fn parse_mcp_malformed_returns_error() {
        let content = "not valid json";
        let result = skills_locate::parse_mcp_json(content);
        assert!(result.is_err());
    }

    #[test]
    #[ignore = "requires network access"]
    fn discover_skills_real_repo() {
        let result = discover_skills("https://github.com/anthropics/claude-code");
        match result {
            Ok(discovery) => {
                assert_eq!(discovery.source.owner, "anthropics");
                assert_eq!(discovery.source.repo, "claude-code");
                assert!(!discovery.skills.is_empty());
                let first = &discovery.skills[0];
                assert!(!first.name.is_empty());
                assert!(!first.path.is_empty());
                assert!(!first.content.is_empty());
            }
            Err(DiscoveryError::NoSkillsFound) => {
                // Acceptable - repo may not have skills
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }
    }
}
