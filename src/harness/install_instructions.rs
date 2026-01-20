use harness_locate::{HarnessKind, InstallationStatus};

pub fn get_install_instructions(kind: HarnessKind) -> Vec<String> {
    match kind {
        HarnessKind::ClaudeCode => claude_code_instructions(),
        HarnessKind::OpenCode => opencode_instructions(),
        HarnessKind::Goose => goose_instructions(),
        HarnessKind::AmpCode => amp_instructions(),
        HarnessKind::CopilotCli => copilot_cli_instructions(),
        HarnessKind::Crush => crush_instructions(),
        _ => vec!["Unknown harness".to_string()],
    }
}

fn copilot_cli_instructions() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec![
            "- npm install -g @github/copilot".to_string(),
            "- brew install copilot-cli".to_string(),
            "- curl -fsSL https://gh.io/copilot-install | bash".to_string(),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "- npm install -g @github/copilot".to_string(),
            "- winget install GitHub.Copilot".to_string(),
        ]
    } else {
        vec![
            "- npm install -g @github/copilot".to_string(),
            "- brew install copilot-cli".to_string(),
            "- curl -fsSL https://gh.io/copilot-install | bash".to_string(),
        ]
    }
}

fn crush_instructions() -> Vec<String> {
    vec![
        "- Visit https://charm.sh/crush for installation instructions".to_string(),
        "- brew install charmbracelet/tap/crush".to_string(),
    ]
}

fn claude_code_instructions() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec![
            "- brew install --cask claude-code".to_string(),
            "- curl -fsSL https://claude.ai/install.sh | bash".to_string(),
            "- npm install -g @anthropic-ai/claude-code".to_string(),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "- irm https://claude.ai/install.ps1 | iex".to_string(),
            "- npm install -g @anthropic-ai/claude-code".to_string(),
        ]
    } else {
        vec![
            "- curl -fsSL https://claude.ai/install.sh | bash".to_string(),
            "- npm install -g @anthropic-ai/claude-code".to_string(),
        ]
    }
}

fn opencode_instructions() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec![
            "- brew install anomalyco/tap/opencode".to_string(),
            "- curl -fsSL https://opencode.ai/install | bash".to_string(),
            "- npm install -g opencode-ai".to_string(),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "- choco install opencode".to_string(),
            "- scoop install extras/opencode".to_string(),
            "- npm install -g opencode-ai".to_string(),
        ]
    } else {
        vec![
            "- curl -fsSL https://opencode.ai/install | bash".to_string(),
            "- npm install -g opencode-ai".to_string(),
            "- brew install anomalyco/tap/opencode".to_string(),
        ]
    }
}

fn goose_instructions() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec![
            "- brew install block-goose-cli".to_string(),
            "- curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh | bash".to_string(),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "PowerShell:".to_string(),
            "- Invoke-WebRequest -Uri \"https://raw.githubusercontent.com/block/goose/main/download_cli.ps1\" -OutFile \"download_cli.ps1\"; .\\download_cli.ps1".to_string(),
            "Git Bash:".to_string(),
            "- curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh | bash".to_string(),
        ]
    } else {
        vec![
            "- curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh | bash".to_string(),
        ]
    }
}

fn amp_instructions() -> Vec<String> {
    if cfg!(target_os = "macos") {
        vec![
            "- curl -fsSL https://ampcode.com/install.sh | bash".to_string(),
            "- npm install -g @sourcegraph/amp@latest".to_string(),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "- npm install -g @sourcegraph/amp@latest".to_string(),
            "WSL:".to_string(),
            "- curl -fsSL https://ampcode.com/install.sh | bash".to_string(),
        ]
    } else {
        vec![
            "- curl -fsSL https://ampcode.com/install.sh | bash".to_string(),
            "- npm install -g @sourcegraph/amp@latest".to_string(),
        ]
    }
}

pub fn get_empty_state_message(
    kind: HarnessKind,
    status: InstallationStatus,
    has_profiles: bool,
) -> Vec<String> {
    let harness_name = match kind {
        HarnessKind::ClaudeCode => "Claude Code",
        HarnessKind::OpenCode => "OpenCode",
        HarnessKind::Goose => "Goose",
        HarnessKind::AmpCode => "AMP Code",
        HarnessKind::CopilotCli => "Copilot CLI",
        HarnessKind::Crush => "Crush",
        _ => "Unknown",
    };

    match status {
        InstallationStatus::FullyInstalled { .. } if !has_profiles => {
            vec![
                "No profiles found".to_string(),
                String::new(),
                "Press 'n' to create a profile".to_string(),
                "or run: bridle profile create".to_string(),
            ]
        }
        InstallationStatus::FullyInstalled { .. } => {
            vec![
                "No profiles found".to_string(),
                String::new(),
                "This shouldn't happen - please refresh".to_string(),
                "Press 'r' to refresh.".to_string(),
            ]
        }
        InstallationStatus::NotInstalled => {
            let mut lines = vec![format!("{} not installed", harness_name), String::new()];
            lines.extend(get_install_instructions(kind));
            lines.push(String::new());
            lines.push("Profiles are disabled until installed.".to_string());
            lines
        }
        InstallationStatus::ConfigOnly { .. } => {
            let mut lines = vec![
                format!("{} binary not found", harness_name),
                String::new(),
                "Configuration exists but binary is missing.".to_string(),
                String::new(),
            ];
            lines.extend(get_install_instructions(kind));
            lines.push(String::new());
            lines.push("Profiles are disabled until installed.".to_string());
            lines.push("Press 'r' to refresh after installation.".to_string());
            lines
        }
        InstallationStatus::BinaryOnly { .. } => {
            let run_command = match kind {
                HarnessKind::ClaudeCode => "claude",
                HarnessKind::OpenCode => "opencode",
                HarnessKind::Goose => "goose",
                HarnessKind::AmpCode => "amp",
                HarnessKind::CopilotCli => "copilot",
                HarnessKind::Crush => "crush",
                _ => "<unknown>",
            };

            let mut lines = vec![
                format!("{} not configured", harness_name),
                String::new(),
                "Binary found but no configuration directory.".to_string(),
                String::new(),
                "Run once to initialize configuration:".to_string(),
                format!("- {}", run_command),
            ];

            lines.push(String::new());
            lines.push("Profiles are disabled until configured.".to_string());
            lines.push("Press 'r' to refresh after first run.".to_string());
            lines
        }
        _ => {
            vec![
                "Unknown installation status".to_string(),
                String::new(),
                "Press 'r' to refresh.".to_string(),
            ]
        }
    }
}
