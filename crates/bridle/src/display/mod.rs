//! Shared display formatting for CLI and TUI.
//!
//! This module provides a semantic intermediate representation (IR) for profile display.
//! Both CLI and TUI consume the same `ProfileNode` tree structure, then render it
//! according to their output format (flat text vs styled lines with tree branches).

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::config::{McpServerInfo, ProfileInfo, ResourceSummary};

/// Semantic section types for profile display.
///
/// Each variant carries semantic meaning that renderers can use for styling decisions
/// (e.g., TUI can color enabled servers green, disabled red).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    /// Profile header (name, harness, status, path).
    Header,
    /// Simple field with optional value.
    Field,
    /// Container for MCP servers.
    McpGroup,
    /// Individual MCP server entry.
    McpServer { enabled: bool },
    /// Container for resources (skills, commands, plugins, agents).
    ResourceGroup { exists: bool },
    /// Individual resource item.
    ResourceItem,
    /// Rules file reference.
    RulesFile { exists: bool },
    /// Error or warning message.
    Error,
}

/// A node in the profile display tree.
///
/// Represents a single displayable element with optional children for nested content.
#[derive(Debug, Clone)]
pub struct ProfileNode {
    /// Semantic type of this node.
    pub kind: SectionKind,
    /// Display label (e.g., "Theme", "MCP Servers").
    pub label: &'static str,
    /// Optional text content.
    pub text: Option<String>,
    /// Child nodes for nested content.
    pub children: Vec<ProfileNode>,
}

impl ProfileNode {
    /// Create a new node with the given kind and label.
    pub fn new(kind: SectionKind, label: &'static str) -> Self {
        Self {
            kind,
            label,
            text: None,
            children: vec![],
        }
    }

    /// Set the text content.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Set child nodes.
    pub fn with_children(mut self, children: Vec<ProfileNode>) -> Self {
        self.children = children;
        self
    }
}

/// Format MCP server detail string.
///
/// Produces a string like `(stdio): npx server-name args` from server info.
pub fn format_mcp_detail(server: &McpServerInfo) -> String {
    let args_str = server
        .args
        .as_ref()
        .map(|a| a.join(" "))
        .unwrap_or_default();

    match (&server.server_type, &server.command, &server.url) {
        (Some(t), Some(cmd), _) if args_str.is_empty() => format!("({t}): {cmd}"),
        (Some(t), Some(cmd), _) => format!("({t}): {cmd} {args_str}"),
        (Some(t), None, Some(url)) => format!("({t}): {url}"),
        (Some(t), None, None) => format!("({t})"),
        _ => String::new(),
    }
}

/// Build semantic display tree from ProfileInfo.
///
/// The returned nodes can be rendered by CLI (`nodes_to_text`) or TUI (`nodes_to_lines`).
pub fn profile_to_nodes(info: &ProfileInfo) -> Vec<ProfileNode> {
    let mut nodes = Vec::new();

    nodes.push(
        ProfileNode::new(SectionKind::Header, "Profile")
            .with_text(&info.name)
            .with_children(vec![
                ProfileNode::new(SectionKind::Field, "Harness").with_text(&info.harness_id),
                ProfileNode::new(SectionKind::Field, "Status").with_text(if info.is_active {
                    "Active"
                } else {
                    "Inactive"
                }),
                ProfileNode::new(SectionKind::Field, "Path")
                    .with_text(info.path.display().to_string()),
            ]),
    );

    let theme_text = match &info.theme {
        Some(theme) => theme.clone(),
        None if info.harness_id == "opencode" => "(not set)".to_string(),
        None => "(not supported)".to_string(),
    };
    nodes.push(ProfileNode::new(SectionKind::Field, "Theme").with_text(theme_text));

    let model_text = match &info.model {
        Some(model) => model.clone(),
        None => "(not set)".to_string(),
    };
    nodes.push(ProfileNode::new(SectionKind::Field, "Model").with_text(model_text));

    nodes.push(build_mcp_node(info));

    nodes.push(build_resource_node("Skills", &info.skills, true));
    nodes.push(build_resource_node("Commands", &info.commands, true));

    match &info.plugins {
        Some(plugins) => nodes.push(build_resource_node("Plugins", plugins, true)),
        None => nodes.push(
            ProfileNode::new(SectionKind::ResourceGroup { exists: false }, "Plugins")
                .with_text("(not supported)"),
        ),
    }

    match &info.agents {
        Some(agents) => nodes.push(build_resource_node("Agents", agents, true)),
        None => nodes.push(
            ProfileNode::new(SectionKind::ResourceGroup { exists: false }, "Agents")
                .with_text("(not supported)"),
        ),
    }

    // Rules file
    let (rules_exists, rules_text) = match &info.rules_file {
        Some(path) => {
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("(unknown)");
            (true, filename.to_string())
        }
        None => (false, "(none)".to_string()),
    };
    nodes.push(
        ProfileNode::new(
            SectionKind::RulesFile {
                exists: rules_exists,
            },
            "Rules",
        )
        .with_text(rules_text),
    );

    if !info.extraction_errors.is_empty() {
        let error_children: Vec<ProfileNode> = info
            .extraction_errors
            .iter()
            .map(|err| ProfileNode::new(SectionKind::Error, "").with_text(err.clone()))
            .collect();
        nodes.push(ProfileNode::new(SectionKind::Error, "Errors").with_children(error_children));
    }

    nodes
}

fn build_mcp_node(info: &ProfileInfo) -> ProfileNode {
    if info.mcp_servers.is_empty() {
        return ProfileNode::new(SectionKind::McpGroup, "MCP Servers").with_text("(none)");
    }

    let children: Vec<ProfileNode> = info
        .mcp_servers
        .iter()
        .map(|server| {
            let detail = format_mcp_detail(server);
            let disabled_suffix = if server.enabled { "" } else { " (disabled)" };
            let text = if detail.is_empty() {
                format!("{}{}", server.name, disabled_suffix)
            } else {
                format!("{} {}{}", server.name, detail, disabled_suffix)
            };
            ProfileNode::new(
                SectionKind::McpServer {
                    enabled: server.enabled,
                },
                "",
            )
            .with_text(text)
        })
        .collect();

    ProfileNode::new(SectionKind::McpGroup, "MCP Servers")
        .with_text(format!("({})", info.mcp_servers.len()))
        .with_children(children)
}

fn build_resource_node(
    label: &'static str,
    summary: &ResourceSummary,
    _supported: bool,
) -> ProfileNode {
    if !summary.directory_exists {
        return ProfileNode::new(SectionKind::ResourceGroup { exists: false }, label)
            .with_text("(directory not found)");
    }

    if summary.items.is_empty() {
        return ProfileNode::new(SectionKind::ResourceGroup { exists: true }, label)
            .with_text("(none)");
    }

    let items_text = summary.items.join(", ");
    let children: Vec<ProfileNode> = summary
        .items
        .iter()
        .map(|item| ProfileNode::new(SectionKind::ResourceItem, "").with_text(item.clone()))
        .collect();

    ProfileNode::new(SectionKind::ResourceGroup { exists: true }, label)
        .with_text(format!("({}) {}", summary.items.len(), items_text))
        .with_children(children)
}

/// Render profile nodes to flat CLI text output.
pub fn nodes_to_text(nodes: &[ProfileNode]) -> String {
    let mut output = String::new();
    for node in nodes {
        render_node_text(&mut output, node);
    }
    output
}

fn render_node_text(out: &mut String, node: &ProfileNode) {
    use std::fmt::Write;

    match &node.kind {
        SectionKind::Header => {
            let _ = writeln!(
                out,
                "{}: {}",
                node.label,
                node.text.as_deref().unwrap_or("")
            );
            for child in &node.children {
                let _ = writeln!(
                    out,
                    "{}: {}",
                    child.label,
                    child.text.as_deref().unwrap_or("")
                );
            }
            let _ = writeln!(out);
        }
        SectionKind::Field => {
            let _ = writeln!(
                out,
                "{}: {}",
                node.label,
                node.text.as_deref().unwrap_or("")
            );
            if node.label == "Model" {
                let _ = writeln!(out);
            }
        }
        SectionKind::McpGroup => {
            if node.children.is_empty() {
                let _ = writeln!(
                    out,
                    "{}: {}",
                    node.label,
                    node.text.as_deref().unwrap_or("(none)")
                );
            } else {
                let _ = writeln!(
                    out,
                    "{} {}:",
                    node.label,
                    node.text.as_deref().unwrap_or("")
                );
                for child in &node.children {
                    render_node_text(out, child);
                }
            }
            let _ = writeln!(out);
        }
        SectionKind::McpServer { enabled } => {
            let indicator = if *enabled { "\u{2713}" } else { "\u{2717}" };
            let _ = writeln!(
                out,
                "  {} {}",
                indicator,
                node.text.as_deref().unwrap_or("")
            );
        }
        SectionKind::ResourceGroup { exists: _ } => {
            let text = node.text.as_deref().unwrap_or("");
            if node.children.is_empty() || text.starts_with("(not ") || text == "(none)" {
                let _ = writeln!(out, "{}: {}", node.label, text);
            } else {
                let count_part = text.split(')').next().unwrap_or("");
                let _ = writeln!(out, "{} {}):", node.label, count_part);
                let items: Vec<&str> = node
                    .children
                    .iter()
                    .filter_map(|c| c.text.as_deref())
                    .collect();
                let _ = writeln!(out, "  {}", items.join(", "));
            }
        }
        SectionKind::ResourceItem => {}
        SectionKind::RulesFile { exists: _ } => {
            let _ = writeln!(
                out,
                "{}: {}",
                node.label,
                node.text.as_deref().unwrap_or("")
            );
        }
        SectionKind::Error => {
            if node.label == "Errors" {
                let _ = writeln!(out);
                let _ = writeln!(out, "{}:", node.label);
                for child in &node.children {
                    let _ = writeln!(out, "  \u{26a0} {}", child.text.as_deref().unwrap_or(""));
                }
            }
        }
    }
}

fn extract_header_info(nodes: &[ProfileNode]) -> (String, bool) {
    for node in nodes {
        if matches!(node.kind, SectionKind::Header) {
            let name = node.text.clone().unwrap_or_default();
            let is_active = node
                .children
                .iter()
                .find(|c| c.label == "Status")
                .and_then(|c| c.text.as_ref())
                .is_some_and(|s| s == "Active");
            return (name, is_active);
        }
    }
    (String::new(), false)
}

/// Tree branch characters for hierarchical display.
pub struct TreeBranch {
    pub branch: &'static str,
    pub continuation: &'static str,
}

impl TreeBranch {
    pub fn for_index(index: usize, total: usize) -> Self {
        let is_last = index == total - 1;
        Self {
            branch: if is_last { "└─" } else { "├─" },
            continuation: if is_last { "   " } else { "│  " },
        }
    }
}

/// Render profile nodes to TUI lines with styling.
pub fn nodes_to_lines(nodes: &[ProfileNode]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let (name, is_active) = extract_header_info(nodes);
    let active_marker = if is_active { "● " } else { "  " };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{}{}", active_marker, name),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " ─────────────────────────",
            Style::default().fg(Color::Gray),
        ),
    ]));

    let display_nodes: Vec<&ProfileNode> = nodes
        .iter()
        .filter(|n| !matches!(n.kind, SectionKind::Header))
        .filter(|n| {
            !matches!(n.kind, SectionKind::Field) || (n.label == "Theme" || n.label == "Model")
        })
        .filter(|n| {
            if matches!(n.kind, SectionKind::ResourceGroup { .. }) {
                let text = n.text.as_deref().unwrap_or("");
                !text.starts_with("(not ") && text != "(none)" && !n.children.is_empty()
            } else {
                true
            }
        })
        .filter(|n| !matches!(n.kind, SectionKind::RulesFile { exists: false }))
        .collect();

    let total = display_nodes.len();
    for (idx, node) in display_nodes.iter().enumerate() {
        let tree = TreeBranch::for_index(idx, total);
        render_node_lines(&mut lines, node, &tree);
    }

    lines
}

fn render_node_lines(lines: &mut Vec<Line<'static>>, node: &ProfileNode, tree: &TreeBranch) {
    match &node.kind {
        SectionKind::Field => {
            lines.push(Line::styled(
                format!(
                    "  {} {}: {}",
                    tree.branch,
                    node.label,
                    node.text.as_deref().unwrap_or("")
                ),
                Style::default().fg(Color::Gray),
            ));
        }
        SectionKind::McpGroup => {
            if node.children.is_empty() {
                return;
            }
            lines.push(Line::styled(
                format!(
                    "  {} MCP {}",
                    tree.branch,
                    node.text.as_deref().unwrap_or("")
                ),
                Style::default().fg(Color::Gray),
            ));
            let server_count = node.children.len();
            for (i, child) in node.children.iter().enumerate() {
                let sub_tree = TreeBranch::for_index(i, server_count);
                render_mcp_server_line(lines, child, tree.continuation, &sub_tree);
            }
        }
        SectionKind::ResourceGroup { exists: _ } => {
            let text = node.text.as_deref().unwrap_or("");
            if node.children.is_empty() {
                return;
            }
            let count_part = text.split(')').next().unwrap_or("");
            lines.push(Line::styled(
                format!("  {} {} {})", tree.branch, node.label, count_part),
                Style::default().fg(Color::Gray),
            ));
            let item_count = node.children.len();
            for (i, child) in node.children.iter().enumerate() {
                let sub_tree = TreeBranch::for_index(i, item_count);
                lines.push(Line::styled(
                    format!(
                        "  {} {} {}",
                        tree.continuation,
                        sub_tree.branch,
                        child.text.as_deref().unwrap_or("")
                    ),
                    Style::default().fg(Color::Gray),
                ));
            }
        }
        SectionKind::RulesFile { exists } => {
            if *exists {
                lines.push(Line::styled(
                    format!(
                        "  {} Rules: {}",
                        tree.branch,
                        node.text.as_deref().unwrap_or("")
                    ),
                    Style::default().fg(Color::Gray),
                ));
            }
        }
        SectionKind::Error => {
            if node.label == "Errors" {
                for child in &node.children {
                    lines.push(Line::styled(
                        format!(
                            "  {} ⚠ {}",
                            tree.branch,
                            child.text.as_deref().unwrap_or("")
                        ),
                        Style::default().fg(Color::Yellow),
                    ));
                }
            } else {
                lines.push(Line::styled(
                    format!("  {} ⚠ {}", tree.branch, node.text.as_deref().unwrap_or("")),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }
        _ => {}
    }
}

fn render_mcp_server_line(
    lines: &mut Vec<Line<'static>>,
    node: &ProfileNode,
    cont: &'static str,
    sub_tree: &TreeBranch,
) {
    if let SectionKind::McpServer { enabled } = &node.kind {
        let (marker, color) = if *enabled {
            ("✓", Color::Green)
        } else {
            ("✗", Color::Gray)
        };

        let full_text = node.text.as_deref().unwrap_or("");
        let (name, detail) = full_text.split_once(' ').unwrap_or((full_text, ""));

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} {} ", cont, sub_tree.branch),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(format!("{} {}", marker, name), Style::default().fg(color)),
            Span::styled(format!(" {}", detail), Style::default().fg(Color::DarkGray)),
        ]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_mcp_detail_stdio_with_args() {
        let server = McpServerInfo {
            name: "test".to_string(),
            enabled: true,
            server_type: Some("stdio".to_string()),
            command: Some("npx".to_string()),
            args: Some(vec!["@server/mcp".to_string(), "--flag".to_string()]),
            url: None,
        };
        assert_eq!(
            format_mcp_detail(&server),
            "(stdio): npx @server/mcp --flag"
        );
    }

    #[test]
    fn test_format_mcp_detail_stdio_no_args() {
        let server = McpServerInfo {
            name: "test".to_string(),
            enabled: true,
            server_type: Some("stdio".to_string()),
            command: Some("server-bin".to_string()),
            args: None,
            url: None,
        };
        assert_eq!(format_mcp_detail(&server), "(stdio): server-bin");
    }

    #[test]
    fn test_format_mcp_detail_url() {
        let server = McpServerInfo {
            name: "test".to_string(),
            enabled: true,
            server_type: Some("sse".to_string()),
            command: None,
            args: None,
            url: Some("http://localhost:3000".to_string()),
        };
        assert_eq!(format_mcp_detail(&server), "(sse): http://localhost:3000");
    }

    #[test]
    fn test_profile_to_nodes_basic() {
        let info = ProfileInfo {
            name: "test-profile".to_string(),
            harness_id: "opencode".to_string(),
            is_active: true,
            path: PathBuf::from("/path/to/profile"),
            mcp_servers: vec![],
            skills: ResourceSummary::default(),
            commands: ResourceSummary::default(),
            plugins: None,
            agents: None,
            rules_file: None,
            theme: Some("dark".to_string()),
            model: Some("gpt-4".to_string()),
            extraction_errors: vec![],
        };

        let nodes = profile_to_nodes(&info);

        assert!(nodes.len() >= 9);
        assert_eq!(nodes[0].kind, SectionKind::Header);
        assert_eq!(nodes[0].text.as_deref(), Some("test-profile"));
    }

    #[test]
    fn test_profile_to_nodes_with_errors() {
        let info = ProfileInfo {
            name: "test".to_string(),
            harness_id: "test".to_string(),
            is_active: false,
            path: PathBuf::from("/tmp"),
            mcp_servers: vec![],
            skills: ResourceSummary::default(),
            commands: ResourceSummary::default(),
            plugins: None,
            agents: None,
            rules_file: None,
            theme: None,
            model: None,
            extraction_errors: vec!["Error 1".to_string(), "Error 2".to_string()],
        };

        let nodes = profile_to_nodes(&info);
        let errors_node = nodes.iter().find(|n| n.label == "Errors");

        assert!(errors_node.is_some());
        assert_eq!(errors_node.unwrap().children.len(), 2);
    }

    #[test]
    fn test_nodes_to_text_renders_header_and_fields() {
        let nodes = vec![
            ProfileNode::new(SectionKind::Header, "Profile")
                .with_text("test".to_string())
                .with_children(vec![
                    ProfileNode::new(SectionKind::Field, "Harness")
                        .with_text("opencode".to_string()),
                    ProfileNode::new(SectionKind::Field, "Status").with_text("Active".to_string()),
                ]),
            ProfileNode::new(SectionKind::Field, "Theme").with_text("dark".to_string()),
        ];

        let output = nodes_to_text(&nodes);

        assert!(output.contains("Profile: test"));
        assert!(output.contains("Harness: opencode"));
        assert!(output.contains("Status: Active"));
        assert!(output.contains("Theme: dark"));
    }

    #[test]
    fn test_nodes_to_lines_renders_tree_structure() {
        let nodes = vec![
            ProfileNode::new(SectionKind::Header, "Profile")
                .with_text("test-profile".to_string())
                .with_children(vec![
                    ProfileNode::new(SectionKind::Field, "Harness")
                        .with_text("opencode".to_string()),
                    ProfileNode::new(SectionKind::Field, "Status").with_text("Active".to_string()),
                ]),
            ProfileNode::new(SectionKind::McpGroup, "MCP Servers").with_children(vec![
                ProfileNode {
                    kind: SectionKind::McpServer { enabled: true },
                    label: "",
                    text: Some("enabled-server (stdio): cmd".to_string()),
                    children: vec![],
                },
                ProfileNode {
                    kind: SectionKind::McpServer { enabled: false },
                    label: "",
                    text: Some("disabled-server (stdio): cmd2 (disabled)".to_string()),
                    children: vec![],
                },
            ]),
        ];

        let lines = nodes_to_lines(&nodes);

        assert!(!lines.is_empty());

        let line_strings: Vec<String> = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref() as &str)
                    .collect::<String>()
            })
            .collect();

        assert!(line_strings.iter().any(|s| s.contains("test-profile")));
        assert!(line_strings.iter().any(|s| s.contains("enabled-server")));
        assert!(line_strings.iter().any(|s| s.contains("disabled-server")));
    }

    #[test]
    fn test_nodes_to_lines_disabled_mcp_uses_gray() {
        let nodes = vec![
            ProfileNode::new(SectionKind::McpGroup, "MCP Servers").with_children(vec![
                ProfileNode {
                    kind: SectionKind::McpServer { enabled: false },
                    label: "",
                    text: Some("disabled-server (stdio): cmd (disabled)".to_string()),
                    children: vec![],
                },
            ]),
        ];

        let lines = nodes_to_lines(&nodes);

        let disabled_line = lines
            .iter()
            .find(|line| {
                line.spans
                    .iter()
                    .any(|span| span.content.contains("disabled-server"))
            })
            .expect("Should have a line with disabled-server");

        let server_name_span = disabled_line
            .spans
            .iter()
            .find(|span| span.content.contains("disabled-server"))
            .expect("Should have span with server name");

        assert_eq!(
            server_name_span.style.fg,
            Some(Color::Gray),
            "Disabled server name should be gray, got {:?}",
            server_name_span.style.fg
        );
    }
}
