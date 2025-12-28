# Bridle TUI Improvement Plan

## Context

The TUI currently has several UX issues that make first-run experience confusing:

1. Empty profile lists for installed harnesses (user doesn't know if it's working)
2. Help advertised but not implemented (`?` does nothing)
3. Profile creation punts to CLI (defeats TUI purpose)
4. No guidance when things are empty

### How Harness Detection Works

The TUI shows **ALL harnesses** that Bridle supports (`HarnessKind::ALL` = `[ClaudeCode, OpenCode, Goose]`), not just installed ones.

The `get-harness` crate provides `InstallationStatus` for each:

| Status           | Meaning                              |
| ---------------- | ------------------------------------ |
| `FullyInstalled` | Binary exists AND config dir exists  |
| `BinaryOnly`     | Binary exists but no config dir      |
| `ConfigOnly`     | Config dir exists but no binary      |
| `NotInstalled`   | Neither exists                       |

---

## Beads

### Bead 1: Auto-Bootstrap Profiles from Current Harness Config

**Type:** `feature`  
**Priority:** P1 (high)  
**Depends on:** -

#### Problem

When a user launches `bridle tui` for the first time, they see empty profile lists for installed harnesses. This is confusing because:

- The user's harness IS configured (e.g., Claude Code has `~/.claude/settings.json`)
- Bridle just doesn't know about it yet
- User has no idea if Bridle is working or broken

#### Solution

On TUI launch (or `bridle init`), detect installed harnesses with existing configs and auto-create a `default` profile from their current configuration.

#### Scope

- Add `ProfileManager::create_from_current_if_missing()` method
- Call it during `App::new()` in TUI initialization
- Also integrate into `bridle init` flow
- Profile name: `default` (or make it configurable)

#### Acceptance Criteria

- [ ] User runs `bridle tui` -> sees `default` profile for each installed harness with config
- [ ] `default` profile contains snapshot of current harness config
- [ ] Idempotent: running twice doesn't duplicate profiles
- [ ] Works for Claude Code and OpenCode
- [ ] Skips harnesses that are `BinaryOnly`, `ConfigOnly` (empty), or `NotInstalled`

---

### Bead 2: Implement Help Modal (`?` Key)

**Type:** `bug` (advertised but broken)  
**Priority:** P2 (medium)  
**Depends on:** -

#### Problem

Status bar shows "Press ? for help" on startup, but pressing `?` does nothing. This is a broken promise to the user.

#### Solution

Implement a help popup/modal showing all keybindings.

#### Scope

- Add `KeyCode::Char('?')` handler in `handle_key()`
- Add `show_help: bool` state to `App`
- Create `render_help_modal()` function
- Modal renders centered overlay with keybinding reference
- Dismiss with `?`, `Esc`, or `q`

#### Acceptance Criteria

- [ ] Pressing `?` shows help overlay
- [ ] Pressing `?` again or `Esc` dismisses it
- [ ] Help lists all available commands with descriptions
- [ ] Modal is visually distinct (border, different background)

---

### Bead 3: Inline Profile Creation in TUI

**Type:** `feature`  
**Priority:** P2 (medium)  
**Depends on:** -

#### Problem

Pressing `n` just shows "Use CLI: bridle profile create". This defeats the purpose of a TUI.

#### Solution

Implement inline profile creation with text input:

1. `n` -> creates profile from current harness config, prompts for name
2. (Optional) `N` (shift) -> creates empty profile, prompts for name

#### Scope

- Add `InputMode` enum: `Normal`, `CreatingProfile`
- Add input state to `App`: `input_mode`, `input_buffer`, `input_cursor`
- Handle text input in `handle_key()` when in input mode
- Render input field at bottom of profile pane when active
- Wire up to existing `ProfileManager::create_from_current()` / `create_profile()`
- Show error in status bar if name invalid or already exists

#### Acceptance Criteria

- [ ] `n` enters input mode with prompt "Profile name: "
- [ ] User types name, presses Enter -> profile created from current config
- [ ] Backspace works for editing
- [ ] `Esc` cancels input mode
- [ ] Error shown in status bar if name invalid or already exists
- [ ] New profile appears in list immediately after creation

---

### Bead 4: Empty State UX for Profile Pane

**Type:** `enhancement`  
**Priority:** P3 (low)  
**Depends on:** Bead 1 (handles happy path; this handles edge cases)

#### Problem

When a harness has no profiles, the profile pane is empty with no guidance. After Bead 1, this only happens in edge cases, but those edge cases need handling.

#### Edge Cases (after Bead 1)

| Scenario               | InstallationStatus    | Config Exists? | Message to Show                            |
| ---------------------- | --------------------- | -------------- | ------------------------------------------ |
| Not installed          | `NotInstalled`        | No             | "Not installed"                            |
| Binary only, never run | `BinaryOnly`          | No             | "Run harness once to generate config"      |
| User deleted config    | `ConfigOnly` or empty | No             | "No config found"                          |
| Normal (handled by B1) | `FullyInstalled`      | Yes            | (auto-bootstrapped, won't be empty)        |

#### Solution

Show contextual placeholder text in the profile pane based on harness state.

#### Scope

- Modify `render_profile_pane()` to detect empty profile list
- Check `harness.installation_status()` and config existence
- Render appropriate message centered in the pane

#### Acceptance Criteria

- [ ] Empty profile pane shows contextual message based on harness state
- [ ] Message differs for: not installed, binary only, config missing
- [ ] Message disappears once profiles exist
- [ ] Messages are helpful and actionable (tell user what to do)

---

### Bead 5: Show Harness Tracking Status Indicators

**Type:** `enhancement`  
**Priority:** P3 (low)  
**Depends on:** -

#### Problem

User can't tell at a glance which harnesses Bridle is actively managing vs. which have configs that haven't been captured yet.

#### Solution

In the harness pane, show visual indicators:

| Indicator | Meaning                                    |
| --------- | ------------------------------------------ |
| `*`       | Has active Bridle profile                  |
| `+`       | Has config but no Bridle profile yet       |
| `-`       | Binary only (no config)                    |
| ` `       | Not installed                              |

#### Scope

- Modify `render_harness_pane()` to show status indicator
- Check `harness.installation_status()` and `harness.mcp().exists()`
- Cross-reference with `bridle_config.active_profile_for()`
- Update legend in help modal (Bead 2)

#### Acceptance Criteria

- [ ] Harness list shows indicator before each harness name
- [ ] User can distinguish: tracked, untracked+has config, binary only, not installed
- [ ] Indicators explained in help modal

---

## Dependency Graph

```
Bead 1 (Auto-Bootstrap) ─┐
                         ├─> Bead 4 (Empty State) [handles edge cases after B1]
                         │
Bead 2 (Help Modal) ─────┼─> Bead 5 (Status Indicators) [legend in help]
                         │
Bead 3 (Inline Create) ──┘
```

**Recommended implementation order:** 1 -> 2 -> 3 -> 4 -> 5

---

## Summary Table

| Bead | Title                      | Type        | Priority | Depends On | Effort |
| ---- | -------------------------- | ----------- | -------- | ---------- | ------ |
| 1    | Auto-Bootstrap Profiles    | feature     | P1       | -          | Medium |
| 2    | Implement Help Modal       | bug         | P2       | -          | Small  |
| 3    | Inline Profile Creation    | feature     | P2       | -          | Medium |
| 4    | Empty State UX             | enhancement | P3       | 1          | Small  |
| 5    | Harness Status Indicators  | enhancement | P3       | -          | Small  |

---

## Creating Beads

To create these as actual beads:

```bash
# Bead 1
bd create --title="Auto-bootstrap profiles from current harness config" --type=feature --priority=1

# Bead 2
bd create --title="Implement help modal (? key)" --type=bug --priority=2

# Bead 3
bd create --title="Inline profile creation in TUI" --type=feature --priority=2

# Bead 4
bd create --title="Empty state UX for profile pane" --type=enhancement --priority=3

# Bead 5
bd create --title="Show harness tracking status indicators" --type=enhancement --priority=3

# Add dependency: Bead 4 depends on Bead 1
bd dep add <bead4-id> <bead1-id>
```
