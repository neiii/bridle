# harness-locate Library Issues

This document tracks bugs and design issues discovered in the `harness-locate` crate (v0.2.2) that affect bridle's functionality. These require upstream fixes rather than bridle workarounds.

---

## Issue 1: Inconsistent Directory Structure Between Skills and Commands (OpenCode)

**Bead**: bridle-98z  
**Severity**: Medium  
**Discovered**: 2025-12-29

### Problem

OpenCode uses inconsistent directory structures for skills vs commands:

- **Skills**: Nested structure - `skill/*/SKILL.md`
- **Commands**: Flat structure - `command/*.md`

This inconsistency causes confusion when users create profile directories. If a user creates `command/my-cmd/index.md` (following the skills pattern), it won't be found because harness-locate expects flat `command/*.md` files.

### Evidence

Global OpenCode command directory (flat structure):
```
~/.config/opencode/command/
  coach.md
  create.md
  finish.md
  ...
```

Global OpenCode skill directory (nested structure):
```
~/.config/opencode/skill/
  my-skill/
    SKILL.md
```

### Impact

- Users creating profile directories may use wrong structure
- No clear documentation on expected structure per resource type
- bridle correctly uses harness-locate's DirectoryStructure, but the inconsistency is confusing

### Proposed Solution

1. **Option A**: Standardize on nested structure for both skills and commands
2. **Option B**: Add clearer documentation in harness-locate about expected structures
3. **Option C**: Support both flat and nested structures for all resource types

---

## Issue 2: Singular vs Plural Directory Names Across Harnesses

**Bead**: bridle-98z (related)  
**Severity**: Low  
**Discovered**: 2025-12-29

### Problem

Different harnesses use different naming conventions:

| Harness | Skills Dir | Commands Dir |
|---------|-----------|--------------|
| OpenCode | `skill/` (singular) | `command/` (singular) |
| Claude Code | N/A | `commands/` (plural) |
| Goose | N/A | N/A |

### Impact

- Users must know the correct naming for each harness
- Profile creation with wrong names silently fails to find resources

### Proposed Solution

Document the naming conventions clearly, or normalize to consistent naming across harnesses.

---

## Issue 3: OpenCode Agents are Both Config-Based and Directory-Based

**Bead**: bridle-957  
**Severity**: Medium  
**Discovered**: 2025-12-29

### Problem

OpenCode has TWO types of agents:

1. **Config-based agents**: Defined in `opencode.jsonc` under `agent.general`, `agent.build`, etc.
   ```json
   "agent": { "general": { "model": "anthropic/claude-sonnet-4-5" } }
   ```

2. **Directory-based agents**: Custom agent definitions at `~/.config/opencode/agent/*.md`
   ```
   ~/.config/opencode/agent/
     codebase-analyzer.md
     codebase-pattern-finder.md
   ```

harness-locate's `agents(&Scope::Global)` only returns the directory path, with no way to access config-based agent information.

### Evidence

```bash
# Config-based agent in opencode.jsonc
"agent": { "general": { "model": "anthropic/claude-sonnet-4-5" } }

# Directory-based agents also exist
ls ~/.config/opencode/agent/
# codebase-analyzer.md  codebase-pattern-finder.md
```

### Impact

- bridle shows "Agents: (directory not found)" when profile lacks agent directory
- Config-based agents (the more common case) are completely invisible
- Users see confusing "directory not found" when agents ARE configured in the JSON

### Proposed Solution

1. Add `agents_config()` method to return config-based agent information
2. OR modify `agents()` to return a richer type indicating both config and directory agents
3. OR document that `agents()` only covers directory-based custom agents

---

## Issue 3b: OpenCode Commands are Both Config-Based and Directory-Based

**Bead**: (same as Issue 3)  
**Severity**: Medium  
**Discovered**: 2025-12-29

### Problem

Similar to agents, OpenCode supports TWO types of commands:

1. **Config-based commands**: Defined in `opencode.jsonc` under `command` object
   ```json
   "command": {
     "test": { "template": "Run tests", "agent": "build" },
     "review": { "template": "Review code", "agent": "oracle" }
   }
   ```

2. **Directory-based commands**: Custom slash commands at `~/.config/opencode/command/*.md`

harness-locate's `commands(&Scope::Global)` only returns the directory path.

### Impact

- Config-based commands (commonly used) are invisible to consumers
- bridle had to add custom parsing of `command` object from opencode.jsonc

### Workaround Applied in bridle

bridle now parses the `command` object keys from opencode.jsonc and merges with directory-based commands. This duplicates parsing logic that harness-locate should ideally provide.

### Proposed Solution

Same as Issue 3 - harness-locate should expose config-based commands, or document that only directory-based commands are returned.

---

## Issue 4: No Profile-Scoped Directory Resolution

**Bead**: bridle-98z (related)  
**Severity**: Medium  
**Discovered**: 2025-12-29

### Problem

`harness.skills(&Scope::Global)` and `harness.commands(&Scope::Global)` only return global paths. There's no `Scope::Profile(path)` variant to get the expected directory structure for a custom profile location.

bridle works around this by:
1. Calling `harness.skills(&Scope::Global)` to get the DirectoryStructure
2. Extracting just the directory name (e.g., "skill")
3. Looking for that directory in the profile path

This works but is fragile - it assumes profiles mirror the global structure.

### Proposed Solution

Add `Scope::Custom(PathBuf)` or `Scope::Profile(PathBuf)` to allow querying expected paths within arbitrary directories.

---

## Issue 5: Wrong Config Filename for OpenCode

**Bead**: bridle-0c2  
**Severity**: High  
**Discovered**: 2025-12-29

### Problem

harness-locate returns `opencode.json` as the config filename, but OpenCode actually uses `opencode.jsonc` (JSON with Comments).

### Evidence

```rust
// harness-locate returns:
harness.config(&Scope::Global) // -> Some("~/.config/opencode/opencode.json")

// But the actual file is:
~/.config/opencode/opencode.jsonc
```

### Impact

- Config file lookup fails silently
- Theme, model, and other settings not parsed
- Profile creation from current fails to copy config

### Proposed Solution

Update harness-locate to return the correct filename `opencode.jsonc` for OpenCode harness.

---
