//! CLI install command implementation.

use std::io::IsTerminal;

use color_eyre::eyre::{Result, eyre};
use dialoguer_multiselect::GroupMultiSelect;
use dialoguer_multiselect::theme::ColorfulTheme;

use crate::config::{BridleConfig, ProfileManager};
use crate::harness::HarnessConfig;
use crate::install::discovery::{DiscoveryError, discover_skills};
use crate::install::installer::{install_agent, install_command, install_skills};
use crate::install::{
    AgentInfo, CommandInfo, DiscoveryResult, InstallOptions, InstallTarget, McpInfo, SkillInfo,
};

/// Selected components from the discovery result
struct SelectedComponents {
    skills: Vec<SkillInfo>,
    mcp_servers: Vec<McpInfo>,
    agents: Vec<AgentInfo>,
    commands: Vec<CommandInfo>,
}

impl SelectedComponents {
    fn is_empty(&self) -> bool {
        self.skills.is_empty()
            && self.mcp_servers.is_empty()
            && self.agents.is_empty()
            && self.commands.is_empty()
    }
}

pub fn run(source: &str, force: bool) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        return Err(eyre!(
            "Interactive mode requires a terminal. Use --help for non-interactive options."
        ));
    }

    let url = normalize_source(source);

    eprintln!("Discovering components from {}...", url);

    let discovery = discover_skills(&url).map_err(|e| match e {
        DiscoveryError::InvalidUrl(msg) => eyre!("Invalid URL: {}", msg),
        DiscoveryError::FetchError(e) => eyre!("Failed to fetch repository: {}", e),
        DiscoveryError::NoSkillsFound => eyre!("No installable components found in repository"),
    })?;

    // Build summary of what was found
    let mut found_parts = Vec::new();
    if !discovery.skills.is_empty() {
        found_parts.push(format!("{} skill(s)", discovery.skills.len()));
    }
    if !discovery.mcp_servers.is_empty() {
        found_parts.push(format!("{} MCP server(s)", discovery.mcp_servers.len()));
    }
    if !discovery.agents.is_empty() {
        found_parts.push(format!("{} agent(s)", discovery.agents.len()));
    }
    if !discovery.commands.is_empty() {
        found_parts.push(format!("{} command(s)", discovery.commands.len()));
    }

    if found_parts.is_empty() {
        eprintln!("No installable components found in {}", url);
        return Ok(());
    }

    eprintln!(
        "Found {} from {}/{}",
        found_parts.join(", "),
        discovery.source.owner,
        discovery.source.repo
    );

    let selected = select_components(&discovery)?;

    if selected.is_empty() {
        eprintln!("No components selected");
        return Ok(());
    }

    let targets = select_targets()?;

    if targets.is_empty() {
        eprintln!("No targets selected");
        return Ok(());
    }

    let options = InstallOptions { force };

    for target in &targets {
        eprintln!("\nInstalling to {}/{}...", target.harness, target.profile);

        // Install skills
        if !selected.skills.is_empty() {
            let report = install_skills(&selected.skills, target, &options);

            for success in &report.installed {
                eprintln!("  + Installed skill: {}", success.skill);
            }
            for skip in &report.skipped {
                eprintln!("  = Skipped skill: {} (already exists)", skip.skill);
            }
            for error in &report.errors {
                eprintln!(
                    "  ! Error installing skill {}: {}",
                    error.skill, error.error
                );
            }
        }

        // Install agents
        for agent in &selected.agents {
            match install_agent(agent, target, &options) {
                Ok(crate::install::installer::InstallOutcome::Installed(success)) => {
                    eprintln!("  + Installed agent: {}", success.skill);
                }
                Ok(crate::install::installer::InstallOutcome::Skipped(skip)) => {
                    eprintln!("  = Skipped agent: {} (already exists)", skip.skill);
                }
                Err(e) => {
                    eprintln!("  ! Error installing agent {}: {}", agent.name, e);
                }
            }
        }

        // Install commands
        for cmd in &selected.commands {
            match install_command(cmd, target, &options) {
                Ok(crate::install::installer::InstallOutcome::Installed(success)) => {
                    eprintln!("  + Installed command: {}", success.skill);
                }
                Ok(crate::install::installer::InstallOutcome::Skipped(skip)) => {
                    eprintln!("  = Skipped command: {} (already exists)", skip.skill);
                }
                Err(e) => {
                    eprintln!("  ! Error installing command {}: {}", cmd.name, e);
                }
            }
        }

        // TODO: Install MCP servers when installer is implemented
        for mcp in &selected.mcp_servers {
            eprintln!(
                "  ~ MCP server: {} (installer not yet implemented)",
                mcp.name
            );
        }
    }

    eprintln!("\nDone!");
    Ok(())
}

/// Select components to install using grouped multi-select UI
fn select_components(discovery: &DiscoveryResult) -> Result<SelectedComponents> {
    // Build groups for each non-empty category
    let mut groups: Vec<(&str, Vec<String>, Vec<usize>)> = Vec::new();

    if !discovery.skills.is_empty() {
        let names: Vec<String> = discovery.skills.iter().map(|s| s.name.clone()).collect();
        let indices: Vec<usize> = (0..discovery.skills.len()).collect();
        groups.push(("Skills", names, indices));
    }

    if !discovery.mcp_servers.is_empty() {
        let names: Vec<String> = discovery
            .mcp_servers
            .iter()
            .map(|m| m.name.clone())
            .collect();
        let indices: Vec<usize> = (0..discovery.mcp_servers.len()).collect();
        groups.push(("MCP Servers", names, indices));
    }

    if !discovery.agents.is_empty() {
        let names: Vec<String> = discovery.agents.iter().map(|a| a.name.clone()).collect();
        let indices: Vec<usize> = (0..discovery.agents.len()).collect();
        groups.push(("Agents", names, indices));
    }

    if !discovery.commands.is_empty() {
        let names: Vec<String> = discovery.commands.iter().map(|c| c.name.clone()).collect();
        let indices: Vec<usize> = (0..discovery.commands.len()).collect();
        groups.push(("Commands", names, indices));
    }

    if groups.is_empty() {
        return Ok(SelectedComponents {
            skills: Vec::new(),
            mcp_servers: Vec::new(),
            agents: Vec::new(),
            commands: Vec::new(),
        });
    }

    // All items selected by default
    let defaults: Vec<Vec<bool>> = groups
        .iter()
        .map(|(_, names, _)| vec![true; names.len()])
        .collect();

    let theme = ColorfulTheme::default();
    let mut group_select = GroupMultiSelect::new()
        .with_theme(&theme)
        .with_prompt("Select components to install (Esc to cancel)")
        .defaults(defaults);

    for (category, names, _) in &groups {
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        group_select = group_select.group(*category, name_refs);
    }

    let Some(selections) = group_select.interact_opt()? else {
        return Ok(SelectedComponents {
            skills: Vec::new(),
            mcp_servers: Vec::new(),
            agents: Vec::new(),
            commands: Vec::new(),
        });
    };

    // Map selections back to discovery items
    let mut selected = SelectedComponents {
        skills: Vec::new(),
        mcp_servers: Vec::new(),
        agents: Vec::new(),
        commands: Vec::new(),
    };

    for (group_idx, selected_indices) in selections.iter().enumerate() {
        let (category, _, _) = &groups[group_idx];
        match *category {
            "Skills" => {
                for &idx in selected_indices {
                    selected.skills.push(discovery.skills[idx].clone());
                }
            }
            "MCP Servers" => {
                for &idx in selected_indices {
                    selected
                        .mcp_servers
                        .push(discovery.mcp_servers[idx].clone());
                }
            }
            "Agents" => {
                for &idx in selected_indices {
                    selected.agents.push(discovery.agents[idx].clone());
                }
            }
            "Commands" => {
                for &idx in selected_indices {
                    selected.commands.push(discovery.commands[idx].clone());
                }
            }
            _ => {}
        }
    }

    Ok(selected)
}

fn normalize_source(source: &str) -> String {
    if source.starts_with("http://") || source.starts_with("https://") {
        source.to_string()
    } else if source.contains('/') && !source.contains(':') {
        format!("https://github.com/{}", source)
    } else {
        source.to_string()
    }
}

fn select_targets() -> Result<Vec<InstallTarget>> {
    use harness_locate::{Harness, HarnessKind};

    let config = BridleConfig::load()?;
    let profiles_dir = BridleConfig::profiles_dir()?;
    let manager = ProfileManager::new(profiles_dir);

    let harness_kinds = [
        HarnessKind::OpenCode,
        HarnessKind::ClaudeCode,
        HarnessKind::Goose,
        HarnessKind::AmpCode,
    ];

    let mut groups: Vec<(String, Vec<String>, Vec<InstallTarget>)> = Vec::new();

    for kind in &harness_kinds {
        let Ok(harness) = Harness::locate(*kind) else {
            continue;
        };
        let harness_id = harness.id();
        let Ok(profiles) = manager.list_profiles(&harness) else {
            continue;
        };

        if profiles.is_empty() {
            continue;
        }

        let active_profile = config.active_profile_for(harness_id);
        let mut labels = Vec::new();
        let mut targets = Vec::new();

        for profile in profiles {
            let is_active = active_profile == Some(profile.as_str());
            let label = if is_active {
                format!("{} (active)", profile)
            } else {
                profile.to_string()
            };
            labels.push(label);
            targets.push(InstallTarget {
                harness: harness_id.to_string(),
                profile,
            });
        }

        groups.push((harness_id.to_string(), labels, targets));
    }

    if groups.is_empty() {
        return Err(eyre!(
            "No profiles found. Create a profile first with: bridle profile create <harness> <name>"
        ));
    }

    let defaults: Vec<Vec<bool>> = groups
        .iter()
        .map(|(_, labels, _)| {
            labels
                .iter()
                .map(|label| label.contains("(active)"))
                .collect()
        })
        .collect();

    let theme = ColorfulTheme::default();
    let mut group_select = GroupMultiSelect::new()
        .with_theme(&theme)
        .with_prompt("Select target profiles (Esc to cancel)")
        .defaults(defaults);

    for (harness_id, labels, _) in &groups {
        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        group_select = group_select.group(harness_id, label_refs);
    }

    let Some(selections) = group_select.interact_opt()? else {
        return Ok(Vec::new());
    };

    let mut selected_targets = Vec::new();
    for (group_idx, indices) in selections.iter().enumerate() {
        let (_, _, targets) = &groups[group_idx];
        for &item_idx in indices {
            selected_targets.push(targets[item_idx].clone());
        }
    }

    Ok(selected_targets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_source_handles_shorthand() {
        assert_eq!(
            normalize_source("owner/repo"),
            "https://github.com/owner/repo"
        );
    }

    #[test]
    fn normalize_source_preserves_full_url() {
        let url = "https://github.com/owner/repo";
        assert_eq!(normalize_source(url), url);
    }

    #[test]
    fn normalize_source_preserves_http() {
        let url = "http://example.com/repo";
        assert_eq!(normalize_source(url), url);
    }
}
