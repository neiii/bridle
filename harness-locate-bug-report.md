# harness-locate Bug Report

## Summary

Multiple issues discovered while implementing profile management in bridle. The harness-locate library returns incorrect or unusable paths for Goose harness resources.

## Environment

- harness-locate version: (as specified in Cargo.toml)
- Affected harnesses: Goose, potentially AMP Code
- Platform: macOS

## Issue 1: Goose skills() returns path outside config directory

### Description

`harness.skills(&Scope::Global)` for Goose returns `Ok(Some(dir))` with a path that is NOT inside `~/.config/goose/`. This causes profile management tools to:
1. Look for skills in the wrong location
2. Copy skills to the wrong destination
3. Display "(none)" even when skills exist in the correct location

### Expected Behavior

`harness.skills(&Scope::Global)` should return:
- `Ok(None)` if Goose doesn't support skills
- `Ok(Some(dir))` with `dir.path` pointing to `~/.config/goose/skills/` if it does

### Actual Behavior

Returns `Ok(Some(dir))` where `dir.path` points to a location that doesn't correspond to where Goose actually stores skills.

### Impact

- Profile switching cannot correctly copy skills directories
- Display logic cannot find skills even when they exist
- Users see "(none)" for skills when files exist in `~/.config/goose/skills/`

## Issue 2: Inconsistent resource path resolution

### Description

When harness-locate returns a directory via `agents()`, `commands()`, or `skills()`, the returned path should be relative to the harness config directory (`config_dir()`). Currently:

- The path returned may be absolute and point to system locations
- The `file_name()` of the path may not match what users expect to create in their config directory

### Expected Behavior

For a harness with `config_dir() = ~/.config/goose/`:
- `skills(&Scope::Global)` should return path like `~/.config/goose/skills/` or similar
- The subdir name should be consistent (e.g., always "skills" not sometimes "skill")

### Workaround in bridle

bridle now uses `config_dir().join(subdir_name)` instead of the returned `dir.path` when copying resources. This ensures resources end up in the correct location.

## Issue 3: Commands returns path for Goose but directory structure doesn't match

### Description

`harness.commands(&Scope::Global)` for Goose returns a directory structure, but:
- The expected structure may not match what Goose actually uses
- Goose supports both `commands/` and `recipes/` directories

### Suggestion

Consider either:
1. Returning `Ok(None)` for resources the harness doesn't officially support
2. Documenting the expected directory structure clearly
3. Adding a way to query "does this harness support X resource type?"

## Reproduction Steps

```rust
use harness_locate::{Harness, HarnessKind, Scope};

fn main() {
    let harness = Harness::new(HarnessKind::Goose).unwrap();
    let config_dir = harness.config_dir().unwrap();
    
    println!("config_dir: {:?}", config_dir);
    
    if let Ok(Some(skills_dir)) = harness.skills(&Scope::Global) {
        println!("skills path: {:?}", skills_dir.path);
        println!("is inside config_dir: {}", skills_dir.path.starts_with(&config_dir));
    }
}
```

## Suggested Fix

1. Ensure all resource paths returned are relative to or inside `config_dir()`
2. Return `Ok(None)` for resources the harness doesn't support rather than guessing
3. Add documentation about which harnesses support which resource types

## Related

See also: `harness-locate-bugs.md` in bridle repo for additional known issues.
