//! MCP server installation executor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use harness_locate::{Harness, HarnessKind, McpServer, StdioMcpServer};

use crate::harness::HarnessConfig;
use serde_json::Value;

use super::installer::InstallError;
use super::mcp_config::{mcp_exists, write_mcp_config};
use super::types::{InstallOptions, InstallTarget, SkipReason};
use crate::config::BridleConfig;

#[derive(Debug, Clone)]
pub struct McpInstallSuccess {
    pub name: String,
    pub target: InstallTarget,
    pub profile_path: PathBuf,
    pub harness_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct McpInstallSkip {
    pub name: String,
    pub target: InstallTarget,
    pub reason: SkipReason,
}

pub enum McpInstallOutcome {
    Installed(McpInstallSuccess),
    Skipped(McpInstallSkip),
}

pub type McpInstallResult = Result<McpInstallOutcome, InstallError>;

fn parse_harness_kind(id: &str) -> Option<HarnessKind> {
    match id {
        "claude-code" | "claude" | "cc" => Some(HarnessKind::ClaudeCode),
        "opencode" | "oc" => Some(HarnessKind::OpenCode),
        "goose" => Some(HarnessKind::Goose),
        "amp-code" | "amp" | "ampcode" => Some(HarnessKind::AmpCode),
        _ => None,
    }
}

fn get_profile_config_path(profile_dir: &Path, harness_kind: HarnessKind) -> PathBuf {
    match harness_kind {
        HarnessKind::ClaudeCode => profile_dir.join(".mcp.json"),
        HarnessKind::OpenCode => profile_dir.join("opencode.jsonc"),
        HarnessKind::Goose => profile_dir.join("config.yaml"),
        HarnessKind::AmpCode => profile_dir.join("settings.json"),
        _ => profile_dir.join("config.json"),
    }
}

fn get_harness_config_path(harness: &Harness) -> Option<PathBuf> {
    harness.mcp_config_path()
}

fn has_env_vars(server: &McpServer) -> bool {
    match server {
        McpServer::Stdio(s) => !s.env.is_empty(),
        McpServer::Sse(s) => !s.headers.is_empty(),
        McpServer::Http(h) => !h.headers.is_empty() || h.oauth.is_some(),
    }
}

pub fn install_mcp(
    name: &str,
    server: &McpServer,
    target: &InstallTarget,
    options: &InstallOptions,
) -> McpInstallResult {
    let profiles_dir = BridleConfig::profiles_dir().map_err(|_| InstallError::ProfileNotFound {
        harness: target.harness.clone(),
        profile: target.profile.as_str().to_string(),
    })?;

    install_mcp_to_dir(name, server, target, options, &profiles_dir)
}

pub fn install_mcp_to_dir(
    name: &str,
    server: &McpServer,
    target: &InstallTarget,
    options: &InstallOptions,
    profiles_dir: &Path,
) -> McpInstallResult {
    let kind = parse_harness_kind(&target.harness)
        .ok_or_else(|| InstallError::HarnessNotFound(target.harness.clone()))?;

    let profile_dir = profiles_dir
        .join(&target.harness)
        .join(target.profile.as_str());

    if !profile_dir.exists() {
        return Err(InstallError::ProfileNotFound {
            harness: target.harness.clone(),
            profile: target.profile.as_str().to_string(),
        });
    }

    let profile_config_path = get_profile_config_path(&profile_dir, kind);

    let config = BridleConfig::load().ok();
    let is_active = config
        .as_ref()
        .and_then(|c| c.active_profile_for(&target.harness))
        .map(|active| active == target.profile.as_str())
        .unwrap_or(false);

    let check_path = if is_active {
        Harness::locate(kind)
            .ok()
            .and_then(|h| h.mcp_config_path())
            .unwrap_or_else(|| profile_config_path.clone())
    } else {
        profile_config_path.clone()
    };

    if !options.force && mcp_exists(kind, &check_path, name).unwrap_or(false) {
        return Ok(McpInstallOutcome::Skipped(McpInstallSkip {
            name: name.to_string(),
            target: target.clone(),
            reason: SkipReason::AlreadyExists,
        }));
    }

    let native_value = server
        .to_native_value(kind, name)
        .map_err(|e| InstallError::WriteFile(std::io::Error::other(e)))?;

    let mut servers_to_write: HashMap<String, Value> = HashMap::new();
    servers_to_write.insert(name.to_string(), native_value);

    write_mcp_config(kind, &profile_config_path, &servers_to_write)
        .map_err(|e| InstallError::WriteFile(std::io::Error::other(e)))?;

    let harness_path = write_mcp_to_harness_if_active(name, server, target, kind)?;

    Ok(McpInstallOutcome::Installed(McpInstallSuccess {
        name: name.to_string(),
        target: target.clone(),
        profile_path: profile_config_path,
        harness_path,
    }))
}

fn write_mcp_to_harness_if_active(
    name: &str,
    server: &McpServer,
    target: &InstallTarget,
    kind: HarnessKind,
) -> Result<Option<PathBuf>, InstallError> {
    let config = BridleConfig::load().ok();
    let is_active = config
        .as_ref()
        .and_then(|c| c.active_profile_for(&target.harness))
        .map(|active| active == target.profile.as_str())
        .unwrap_or(false);

    if !is_active {
        return Ok(None);
    }

    let harness =
        Harness::locate(kind).map_err(|_| InstallError::HarnessNotFound(target.harness.clone()))?;

    let Some(config_path) = get_harness_config_path(&harness) else {
        return Ok(None);
    };

    let native_value = server
        .to_native_value(kind, name)
        .map_err(|e| InstallError::WriteFile(std::io::Error::other(e)))?;

    let mut servers_to_write: HashMap<String, Value> = HashMap::new();
    servers_to_write.insert(name.to_string(), native_value);

    write_mcp_config(kind, &config_path, &servers_to_write)
        .map_err(|e| InstallError::WriteFile(std::io::Error::other(e)))?;

    Ok(Some(config_path))
}

pub fn check_env_var_warnings(servers: &HashMap<String, McpServer>) -> Vec<String> {
    servers
        .iter()
        .filter(|(_, server)| has_env_vars(server))
        .map(|(name, _)| {
            format!(
                "MCP server '{}' has environment variables that may need manual configuration",
                name
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProfileName;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_env(harness: &str) -> (TempDir, InstallTarget, PathBuf) {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        let profile_dir = profiles_dir.join(harness).join("test");
        fs::create_dir_all(&profile_dir).unwrap();

        let target = InstallTarget {
            harness: harness.to_string(),
            profile: ProfileName::new("test").unwrap(),
        };

        (temp, target, profiles_dir)
    }

    fn create_stdio_server() -> McpServer {
        McpServer::Stdio(StdioMcpServer {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
            env: HashMap::new(),
            cwd: None,
            enabled: true,
            timeout_ms: None,
        })
    }

    #[test]
    fn install_mcp_to_claude_profile() {
        let (_temp, target, profiles_dir) = setup_test_env("claude-code");
        let server = create_stdio_server();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions::default(),
            &profiles_dir,
        );
        assert!(result.is_ok());

        if let Ok(McpInstallOutcome::Installed(success)) = result {
            assert_eq!(success.name, "filesystem");
            assert!(success.profile_path.exists());

            let content = fs::read_to_string(&success.profile_path).unwrap();
            assert!(content.contains("filesystem"));
            assert!(content.contains("mcpServers"));
        }
    }

    #[test]
    fn install_mcp_to_opencode_profile() {
        let (_temp, target, profiles_dir) = setup_test_env("opencode");
        let server = create_stdio_server();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions::default(),
            &profiles_dir,
        );
        assert!(result.is_ok());

        if let Ok(McpInstallOutcome::Installed(success)) = result {
            assert!(
                success
                    .profile_path
                    .to_string_lossy()
                    .contains("opencode.jsonc")
            );
        }
    }

    #[test]
    fn install_mcp_to_goose_profile() {
        let (_temp, target, profiles_dir) = setup_test_env("goose");
        let server = create_stdio_server();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions::default(),
            &profiles_dir,
        );
        assert!(result.is_ok());

        if let Ok(McpInstallOutcome::Installed(success)) = result {
            assert!(
                success
                    .profile_path
                    .to_string_lossy()
                    .contains("config.yaml")
            );

            let content = fs::read_to_string(&success.profile_path).unwrap();
            assert!(content.contains("extensions:"));
            assert!(content.contains("filesystem:"));
        }
    }

    #[test]
    fn install_mcp_to_amp_profile() {
        let (_temp, target, profiles_dir) = setup_test_env("amp-code");
        let server = create_stdio_server();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions::default(),
            &profiles_dir,
        );
        assert!(result.is_ok());

        if let Ok(McpInstallOutcome::Installed(success)) = result {
            assert!(
                success
                    .profile_path
                    .to_string_lossy()
                    .contains("settings.json")
            );
        }
    }

    #[test]
    fn install_mcp_skips_existing_without_force() {
        let (temp, target, profiles_dir) = setup_test_env("claude-code");
        let server = create_stdio_server();

        let config_path = temp.path().join("profiles/claude-code/test/.mcp.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"filesystem":{"command":"old"}}}"#,
        )
        .unwrap();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions::default(),
            &profiles_dir,
        );
        assert!(matches!(result, Ok(McpInstallOutcome::Skipped(_))));

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(
            content.contains("old"),
            "Should not overwrite without force"
        );
    }

    #[test]
    fn install_mcp_overwrites_with_force() {
        let (temp, target, profiles_dir) = setup_test_env("claude-code");
        let server = create_stdio_server();

        let config_path = temp.path().join("profiles/claude-code/test/.mcp.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"filesystem":{"command":"old"}}}"#,
        )
        .unwrap();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions { force: true },
            &profiles_dir,
        );
        assert!(matches!(result, Ok(McpInstallOutcome::Installed(_))));

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("npx"), "Should overwrite with force");
    }

    #[test]
    fn install_mcp_preserves_existing_servers() {
        let (temp, target, profiles_dir) = setup_test_env("claude-code");
        let server = create_stdio_server();

        let config_path = temp.path().join("profiles/claude-code/test/.mcp.json");
        fs::write(
            &config_path,
            r#"{"mcpServers":{"other-server":{"command":"other"}}}"#,
        )
        .unwrap();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions::default(),
            &profiles_dir,
        );
        assert!(result.is_ok());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(
            content.contains("other-server"),
            "Should preserve existing servers"
        );
        assert!(content.contains("filesystem"), "Should add new server");
    }

    #[test]
    fn install_mcp_returns_error_for_missing_profile() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();

        let target = InstallTarget {
            harness: "claude-code".to_string(),
            profile: ProfileName::new("nonexistent").unwrap(),
        };
        let server = create_stdio_server();

        let result = install_mcp_to_dir(
            "filesystem",
            &server,
            &target,
            &InstallOptions::default(),
            &profiles_dir,
        );
        assert!(matches!(result, Err(InstallError::ProfileNotFound { .. })));
    }

    #[test]
    fn check_env_var_warnings_detects_env_vars() {
        let mut servers = HashMap::new();

        servers.insert(
            "no-env".to_string(),
            McpServer::Stdio(StdioMcpServer {
                command: "cmd".to_string(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                enabled: true,
                timeout_ms: None,
            }),
        );

        let mut env_map = HashMap::new();
        env_map.insert(
            "API_KEY".to_string(),
            harness_locate::EnvValue::plain("secret"),
        );
        servers.insert(
            "with-env".to_string(),
            McpServer::Stdio(StdioMcpServer {
                command: "cmd".to_string(),
                args: vec![],
                env: env_map,
                cwd: None,
                enabled: true,
                timeout_ms: None,
            }),
        );

        let warnings = check_env_var_warnings(&servers);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("with-env"));
    }

    #[test]
    fn check_env_var_warnings_empty_for_no_env() {
        let mut servers = HashMap::new();
        servers.insert(
            "server".to_string(),
            McpServer::Stdio(StdioMcpServer {
                command: "cmd".to_string(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                enabled: true,
                timeout_ms: None,
            }),
        );

        let warnings = check_env_var_warnings(&servers);
        assert!(warnings.is_empty());
    }
}
