//! Plugin discovery from GitHub repositories.

use crate::component::{
    parse_agent_descriptor, parse_command_descriptor, parse_hooks_json, parse_mcp_json,
    parse_skill_descriptor,
};
use crate::error::{Error, Result};
use crate::fetch::{extract_file, fetch_bytes, list_files};
use crate::github::GitHubRef;
use crate::marketplace::Marketplace;
use crate::types::{DiscoveryResult, PluginDescriptor, PluginSource};

#[derive(Debug, Clone, serde::Deserialize)]
struct PluginJson {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

pub fn discover_plugins(repo_url: &str) -> Result<Vec<PluginDescriptor>> {
    let github_ref = GitHubRef::parse(repo_url)?;
    let archive_url = github_ref.archive_url();
    let archive_bytes = fetch_bytes(&archive_url)?;

    let marketplace_path = find_marketplace_json(&archive_bytes)?;
    let marketplace_content = extract_file(&archive_bytes, &marketplace_path)?;
    let marketplace: Marketplace = serde_json::from_str(&marketplace_content)?;

    let mut plugins = Vec::new();
    let prefix = extract_archive_prefix(&archive_bytes)?;

    for entry in marketplace.plugins {
        let source_str = extract_source_path(&entry.source);
        let plugin_path = resolve_plugin_path(&source_str);

        if let Ok(plugin) = discover_single_plugin(&archive_bytes, &prefix, &plugin_path) {
            plugins.push(plugin);
        }
    }

    Ok(plugins)
}

fn find_marketplace_json(archive: &[u8]) -> Result<String> {
    let candidates = list_files(archive, "marketplace.json")?;

    for path in candidates {
        if path.contains(".claude-plugin/marketplace.json") {
            return Ok(path);
        }
    }

    Err(Error::NotFound(
        ".claude-plugin/marketplace.json".to_string(),
    ))
}

fn extract_archive_prefix(archive: &[u8]) -> Result<String> {
    let files = list_files(archive, "")?;
    if let Some(first) = files.first()
        && let Some(slash_pos) = first.find('/')
    {
        return Ok(first[..=slash_pos].to_string());
    }
    Ok(String::new())
}

fn extract_source_path(source: &PluginSource) -> String {
    match source {
        PluginSource::Relative(path) => path.clone(),
        PluginSource::GitHub { github } => github.clone(),
        PluginSource::Url { url } => url.clone(),
    }
}

fn resolve_plugin_path(source: &str) -> String {
    source.strip_prefix("./").unwrap_or(source).to_string()
}

fn scan_components<T, F>(
    archive: &[u8],
    plugin_prefix: &str,
    subdir: &str,
    suffix: &str,
    parser: F,
) -> Vec<T>
where
    F: Fn(&str) -> Option<T>,
{
    let dir_prefix = format!("{plugin_prefix}{subdir}");
    let Ok(files) = list_files(archive, suffix) else {
        return Vec::new();
    };

    files
        .into_iter()
        .filter(|path| path.starts_with(&dir_prefix))
        .filter_map(|path| {
            extract_file(archive, &path)
                .ok()
                .and_then(|content| parser(&content))
        })
        .collect()
}

fn discover_single_plugin(
    archive: &[u8],
    prefix: &str,
    plugin_path: &str,
) -> Result<PluginDescriptor> {
    // Build base path, avoiding double slashes when plugin_path is empty
    let base = if plugin_path.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}{plugin_path}/")
    };

    let plugin_json_path = format!("{base}.claude-plugin/plugin.json");
    let alt_plugin_json_path = format!("{base}plugin.json");

    let plugin_content = extract_file(archive, &plugin_json_path)
        .or_else(|_| extract_file(archive, &alt_plugin_json_path))?;

    let plugin_json: PluginJson = serde_json::from_str(&plugin_content)?;

    let plugin_prefix = base;

    let skills = scan_components(archive, &plugin_prefix, "skills/", "SKILL.md", |content| {
        parse_skill_descriptor(content).ok()
    });

    let commands = scan_components(archive, &plugin_prefix, "commands/", ".md", |content| {
        parse_command_descriptor(content, "command").ok()
    });

    let agents = scan_components(archive, &plugin_prefix, "agents/", ".md", |content| {
        parse_agent_descriptor(content).ok()
    });

    let hooks_path = format!("{plugin_prefix}.claude-plugin/hooks.json");
    let hooks = extract_file(archive, &hooks_path)
        .ok()
        .and_then(|content| parse_hooks_json(&content).ok());

    let mcp_path = format!("{plugin_prefix}.claude-plugin/.mcp.json");
    let mcp_servers = extract_file(archive, &mcp_path)
        .ok()
        .and_then(|content| parse_mcp_json(&content).ok())
        .unwrap_or_default();

    Ok(PluginDescriptor {
        name: plugin_json.name,
        path: if plugin_path.is_empty() {
            None
        } else {
            Some(plugin_path.to_string())
        },
        description: plugin_json.description,
        skills,
        commands,
        agents,
        hooks,
        mcp_servers,
    })
}

pub fn discover_from_source(source: &PluginSource) -> Result<Vec<PluginDescriptor>> {
    match source {
        PluginSource::GitHub { github } => discover_plugins(github),
        PluginSource::Url { url } => {
            // Check if this is actually a GitHub URL
            if url.contains("github.com/") {
                discover_plugins(url)
            } else {
                Err(Error::NotFound(format!(
                    "URL-based plugin discovery is only supported for GitHub URLs: {url}"
                )))
            }
        }
        PluginSource::Relative(_) => Err(Error::NotFound(
            "Cannot discover from relative path without base URL".to_string(),
        )),
    }
}

#[derive(Debug)]
struct DetectedPlugin {
    path: String,
    method: DetectionMethod,
}

#[derive(Debug)]
enum DetectionMethod {
    Marketplace,
    PluginJson,
    PluginsDir,
    ComponentHeuristic,
}

fn detect_plugins(archive: &[u8], prefix: &str) -> Vec<DetectedPlugin> {
    let mut detected = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // Priority 1: marketplace.json
    if let Ok(marketplace_path) = find_marketplace_json(archive)
        && let Ok(content) = extract_file(archive, &marketplace_path)
        && let Ok(marketplace) = serde_json::from_str::<Marketplace>(&content)
    {
        for entry in marketplace.plugins {
            let source = extract_source_path(&entry.source);
            let path = resolve_plugin_path(&source);
            if seen_paths.insert(path.clone()) {
                detected.push(DetectedPlugin {
                    path,
                    method: DetectionMethod::Marketplace,
                });
            }
        }
    }

    // Priority 2: Root .claude-plugin/plugin.json
    let root_plugin_json = format!("{prefix}.claude-plugin/plugin.json");
    if file_exists(archive, &root_plugin_json) && seen_paths.insert(String::new()) {
        detected.push(DetectedPlugin {
            path: String::new(),
            method: DetectionMethod::PluginJson,
        });
    }

    // Priority 3: plugins/*/.claude-plugin/plugin.json
    if let Ok(files) = list_files(archive, "plugin.json") {
        for file in files {
            if let Some(plugin_path) = extract_plugins_dir_path(&file, prefix)
                && seen_paths.insert(plugin_path.clone())
            {
                detected.push(DetectedPlugin {
                    path: plugin_path,
                    method: DetectionMethod::PluginsDir,
                });
            }
        }
    }

    // Priority 4: Component heuristic (2+ of skills/, commands/, agents/)
    if detected.is_empty() && has_component_dirs(archive, prefix) {
        detected.push(DetectedPlugin {
            path: String::new(),
            method: DetectionMethod::ComponentHeuristic,
        });
    }

    detected
}

fn extract_plugins_dir_path(file_path: &str, prefix: &str) -> Option<String> {
    let relative = file_path.strip_prefix(prefix)?;
    if relative.starts_with("plugins/") {
        let after_plugins = relative.strip_prefix("plugins/")?;
        let plugin_name = after_plugins.split('/').next()?;
        Some(format!("plugins/{plugin_name}"))
    } else {
        None
    }
}

fn has_component_dirs(archive: &[u8], prefix: &str) -> bool {
    let dirs = ["skills/", "commands/", "agents/"];
    let count = dirs
        .iter()
        .filter(|dir| {
            let path = format!("{prefix}{dir}");
            list_files(archive, "")
                .map(|f| f.iter().any(|p| p.starts_with(&path)))
                .unwrap_or(false)
        })
        .count();
    count >= 2
}

fn file_exists(archive: &[u8], path: &str) -> bool {
    extract_file(archive, path).is_ok()
}

fn discover_synthetic_plugin(
    archive: &[u8],
    prefix: &str,
    plugin_path: &str,
    name: String,
) -> PluginDescriptor {
    let base = if plugin_path.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}{plugin_path}/")
    };

    let skills = scan_components(archive, &base, "skills/", "SKILL.md", |content| {
        parse_skill_descriptor(content).ok()
    });

    let commands = scan_components(archive, &base, "commands/", ".md", |content| {
        parse_command_descriptor(content, "command").ok()
    });

    let agents = scan_components(archive, &base, "agents/", ".md", |content| {
        parse_agent_descriptor(content).ok()
    });

    let mcp_path = format!("{base}.claude-plugin/.mcp.json");
    let mcp_servers = extract_file(archive, &mcp_path)
        .ok()
        .and_then(|content| parse_mcp_json(&content).ok())
        .unwrap_or_default();

    PluginDescriptor {
        name,
        path: if plugin_path.is_empty() {
            None
        } else {
            Some(plugin_path.to_string())
        },
        description: None,
        skills,
        commands,
        agents,
        hooks: None,
        mcp_servers,
    }
}

pub fn discover_all(repo_url: &str) -> Result<DiscoveryResult> {
    let github_ref = GitHubRef::parse(repo_url)?;
    let archive_url = github_ref.archive_url();
    let archive_bytes = fetch_bytes(&archive_url)?;
    let prefix = extract_archive_prefix(&archive_bytes)?;

    let detected = detect_plugins(&archive_bytes, &prefix);

    let mut plugins = Vec::new();
    for det in detected {
        let plugin_path = &det.path;
        let derived_name = derive_plugin_name(plugin_path, &github_ref);

        let plugin = match det.method {
            DetectionMethod::ComponentHeuristic => {
                discover_synthetic_plugin(&archive_bytes, &prefix, plugin_path, derived_name)
            }
            _ => match discover_single_plugin(&archive_bytes, &prefix, plugin_path) {
                Ok(mut p) => {
                    if p.name.is_empty() {
                        p.name = derived_name;
                    }
                    p
                }
                Err(_) => continue,
            },
        };

        plugins.push(plugin);
    }

    Ok(DiscoveryResult::from_plugins(plugins))
}

fn derive_plugin_name(path: &str, github_ref: &GitHubRef) -> String {
    if path.is_empty() {
        github_ref.repo.clone()
    } else {
        path.rsplit('/')
            .next()
            .unwrap_or(&github_ref.repo)
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_plugin_path_strips_prefix() {
        assert_eq!(resolve_plugin_path("./plugins/foo"), "plugins/foo");
        assert_eq!(resolve_plugin_path("plugins/bar"), "plugins/bar");
    }

    #[test]
    #[ignore = "requires network"]
    fn discover_anthropics_claude_code() {
        let plugins = discover_plugins("https://github.com/anthropics/claude-code").unwrap();
        assert!(
            plugins.len() >= 13,
            "Expected at least 13 plugins, got {}",
            plugins.len()
        );

        let names: Vec<_> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.contains(&"Code Review"),
            "Should contain Code Review plugin"
        );
    }

    #[test]
    fn extract_plugins_dir_path_valid() {
        let prefix = "repo-main/";

        let path = "repo-main/plugins/code-review/.claude-plugin/plugin.json";
        assert_eq!(
            extract_plugins_dir_path(path, prefix),
            Some("plugins/code-review".to_string())
        );

        let path = "repo-main/plugins/my-plugin/plugin.json";
        assert_eq!(
            extract_plugins_dir_path(path, prefix),
            Some("plugins/my-plugin".to_string())
        );
    }

    #[test]
    fn extract_plugins_dir_path_invalid() {
        let prefix = "repo-main/";

        let path = "repo-main/.claude-plugin/plugin.json";
        assert_eq!(extract_plugins_dir_path(path, prefix), None);

        let path = "repo-main/src/plugin.json";
        assert_eq!(extract_plugins_dir_path(path, prefix), None);

        let path = "other-repo/plugins/foo/plugin.json";
        assert_eq!(extract_plugins_dir_path(path, prefix), None);
    }

    #[test]
    fn derive_plugin_name_from_path() {
        let github_ref = GitHubRef::parse("https://github.com/owner/my-repo").unwrap();

        assert_eq!(derive_plugin_name("", &github_ref), "my-repo");
        assert_eq!(
            derive_plugin_name("plugins/code-review", &github_ref),
            "code-review"
        );
        assert_eq!(
            derive_plugin_name("plugins/deep/nested", &github_ref),
            "nested"
        );
    }
}
