# Bridle Codebase Improvements

Analysis performed: 2025-12-30

## High Priority

### 1. Extract Shared CLI/TUI Display Logic

**Problem**: Display logic is duplicated across three locations, violating the architecture principle documented in AGENTS.md.

| Component           | Location                                              | Lines |
| ------------------- | ----------------------------------------------------- | ----- |
| CLI profile display | `cli/profile.rs` → `print_profile_text()`             | ~80   |
| TUI profile display | `tui/mod.rs` → `render_profile_expanded()`            | ~100  |
| TUI detail pane     | `tui/widgets/detail_pane.rs` → `render_profile_details()` | ~150  |

**Recommendation**: Create a shared display module:

```rust
// src/display/mod.rs
pub struct ProfileSection {
    pub label: &'static str,
    pub value: Option<String>,
    pub children: Vec<ProfileSection>,
}

pub fn profile_to_sections(info: &ProfileInfo) -> Vec<ProfileSection> {
    // Single source of truth for what gets displayed
}

// CLI consumes sections and prints text
// TUI consumes sections and renders widgets
```

**Benefits**:
- Single source of truth for display data
- Feature parity guaranteed between CLI and TUI
- Easier to test display logic in isolation

**Caveat**: CLI uses flat `println!` output while TUI uses tree-branch formatting (`├─`/`└─`). The shared module needs either:
- A `DisplayFormat` enum parameter, or
- A `Renderer` trait with CLI/TUI implementations

The `ProfileSection` struct alone won't solve the formatting divergence.

---

### 2. Split `config/manager.rs` (1,625 lines)

**Problem**: This file handles too many responsibilities:
- Profile CRUD operations
- JSONC comment stripping
- Config parsing for 4 different harnesses
- MCP server extraction
- Resource extraction (skills, commands, plugins, agents)
- Theme/model extraction

**Recommendation**: Split into focused modules:

```
src/config/
├── mod.rs
├── bridle.rs           # BridleConfig (existing)
├── profile_name.rs     # ProfileName (existing)
├── manager.rs          # ProfileManager CRUD only (~400 lines)
├── parser/
│   ├── mod.rs          # JSONC utilities, strip_comments, strip_trailing_commas
│   ├── opencode.rs     # OpenCode-specific parsing
│   ├── claude.rs       # Claude Code-specific parsing
│   ├── goose.rs        # Goose-specific parsing (YAML, GOOSE_* keys)
│   └── amp.rs          # AMP Code-specific parsing
└── extractors.rs       # MCP, resources, theme, model extraction
```

**Benefits**:
- Easier to navigate and maintain
- Harness-specific logic isolated
- Easier to add new harnesses

---

### 3. Add Module-Level Documentation

**Problem**: No `//!` module docs, minimal function docs, no examples.

**Recommendation**: Add documentation to all public modules and key functions.

**Example for `config/manager.rs`**:
```rust
//! Profile management for AI coding assistant configurations.
//!
//! This module provides [`ProfileManager`] for creating, switching, and
//! deleting configuration profiles stored in `~/.config/bridle/profiles/`.
//!
//! # Supported Harnesses
//!
//! - OpenCode (`opencode.jsonc`)
//! - Claude Code (`settings.json`, `.mcp.json`)
//! - Goose (`config.yaml` with `GOOSE_*` keys)
//! - AMP Code (`settings.json` with `amp.*` keys)
//!
//! # Examples
//!
//! ```no_run
//! use bridle::config::ProfileManager;
//!
//! let manager = ProfileManager::new()?;
//! let profiles = manager.list_profiles(&harness)?;
//! ```
```

**Example for public functions**:
```rust
/// Creates a new profile by copying the harness's current configuration.
///
/// # Arguments
///
/// * `harness` - The AI harness to create a profile for
/// * `name` - Profile name (1-64 chars, lowercase alphanumeric + hyphens)
///
/// # Errors
///
/// Returns [`Error::ProfileExists`] if a profile with this name already exists.
/// Returns [`Error::NoConfigFound`] if the harness has no configuration to copy.
///
/// # Examples
///
/// ```no_run
/// manager.create_profile(&harness, ProfileName::new("work")?)?;
/// ```
pub fn create_profile(&self, harness: &Harness, name: ProfileName) -> Result<()>
```

---

## Medium Priority

### 4. Remove or Complete `HarnessAdapter`

**Problem**: `harness/adapter.rs` (21 lines) wraps `Harness` but adds no meaningful abstraction.

```rust
pub struct HarnessAdapter {
    inner: Harness,
}
```

**Options**:
1. **Remove it** if no additional abstraction is needed
2. **Expand it** to provide value, e.g.:
   - Caching parsed configs
   - Version detection
   - Config validation

---

### 5. Clean Up Dead Code in Theme

**Problem**: Multiple `#[allow(dead_code)]` in `tui/theme.rs` and `tui/views/mod.rs`.

**Recommendation**:
- Remove unused theme methods
- Or implement the features that would use them
- Dead code is technical debt and confuses readers

---

## Low Priority (Polish)

### 6. Standardize Error Reporting

**Problem**: Mixed approaches to error reporting:
- `eprintln!()` in CLI commands
- `color_eyre` chain at top level
- Direct string returns in some places

**Recommendation**: Return `Result<T>` from all functions, let `color_eyre` handle presentation:

```rust
// Instead of:
eprintln!("Error: {}", e);
return;

// Do:
return Err(e.into());
// Let main.rs handle presentation
```

---

### 7. Add Integration Tests for CLI Commands

**Problem**: Unit tests exist for ProfileManager, but no end-to-end CLI tests.

**Recommendation**: Add integration tests in `tests/`:

```rust
// tests/cli_integration.rs
use assert_cmd::Command;

#[test]
fn test_profile_list_empty() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("bridle")
        .env("HOME", temp.path())
        .args(["profile", "list", "opencode"])
        .assert()
        .success()
        .stdout(predicates::str::contains("No profiles"));
}
```

---

## Architecture Diagram (Current)

```
┌─────────────────────────────────────────────────────────────┐
│                         main.rs                             │
│                      (CLI routing)                          │
└─────────────────────────┬───────────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         │                │                │
         ▼                ▼                ▼
┌─────────────┐   ┌─────────────┐   ┌─────────────┐
│   cli/*     │   │   tui/*     │   │  config/*   │
│  Commands   │   │  Terminal   │   │  Manager    │
│  Profile    │   │  Views      │   │  Bridle     │
│  Output     │   │  Widgets    │   │  ProfileName│
└──────┬──────┘   └──────┬──────┘   └──────┬──────┘
       │                 │                 │
       │    ┌────────────┴─────────────────┤
       │    │                              │
       │    │  DUPLICATION                 │
       │    │  (display logic)             │
       │    │                              │
       └────┴──────────────────────────────┘
                          │
                          ▼
              ┌─────────────────────┐
              │     harness/*       │
              │   HarnessConfig     │
              │   (harness-locate)  │
              └─────────────────────┘
```

## Architecture Diagram (Proposed)

```
┌─────────────────────────────────────────────────────────────┐
│                         main.rs                             │
└─────────────────────────┬───────────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         │                │                │
         ▼                ▼                ▼
┌─────────────┐   ┌─────────────┐   ┌─────────────┐
│   cli/*     │   │   tui/*     │   │  config/*   │
└──────┬──────┘   └──────┬──────┘   │  manager    │
       │                 │          │  parsers/*  │
       │                 │          └──────┬──────┘
       │                 │                 │
       └────────┬────────┘                 │
                │                          │
                ▼                          │
       ┌─────────────────┐                 │
       │   display/*     │◄────────────────┘
       │ (shared logic)  │
       └────────┬────────┘
                │
                ▼
       ┌─────────────────┐
       │    harness/*    │
       └─────────────────┘
```

---

## Tracking

Create beads for these improvements:

```bash
bd create --title="Extract shared CLI/TUI display logic" --type=task --priority=1
bd create --title="Split manager.rs into focused modules" --type=task --priority=1
bd create --title="Add module-level documentation" --type=docs --priority=2
bd create --title="Clean up dead code in theme.rs" --type=chore --priority=3
bd create --title="Add CLI integration tests" --type=task --priority=3
```
