# Contributing to Bridle

Thanks for your interest in contributing! This document covers the basics.

## Quick Start

```bash
git clone https://github.com/neiii/bridle.git
cd bridle
cargo build
cargo test
```

## Before You Contribute

- **Bug?** → Open an issue using the bug template
- **Feature idea?** → Start a [Discussion](https://github.com/neiii/bridle/discussions/categories/ideas) first
- **Question?** → Use [Q&A Discussions](https://github.com/neiii/bridle/discussions/categories/q-a)

## Development Workflow

### Building & Testing

```bash
# Check for errors (fast)
cargo check

# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Quality gates (run before committing)
cargo fmt -- --check && cargo clippy -- -D warnings && cargo test
```

### Code Style

- Run `cargo fmt` before committing
- All `clippy` warnings must be resolved
- Follow existing patterns in the codebase

### Commit Messages

Keep them concise and descriptive:
- `fix: handle empty profile names`
- `feat: add amp harness support`
- `docs: update installation instructions`

## Pull Requests

1. Fork the repo and create a branch from `master`
2. Make your changes
3. Run the quality gates: `cargo fmt -- --check && cargo clippy -- -D warnings && cargo test`
4. Open a PR with a clear description of what and why

### PR Checklist

- [ ] Code compiles without warnings (`cargo clippy`)
- [ ] Tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Commit messages are clear

## Project Structure

```
src/
├── cli/        # CLI commands and output
├── config/     # Configuration management
├── harness/    # Harness definitions (claude, opencode, goose, amp, copilot)
├── install/    # Skill discovery and installation
└── tui/        # Terminal UI
```

## Publishing & Releases

Release automation is handled by [cargo-dist](https://github.com/axodotdev/cargo-dist) and requires the following GitHub Actions secrets:

| Secret | Purpose |
|--------|---------|
| `HOMEBREW_TAP_TOKEN` | GitHub PAT with write access to `neiii/homebrew-bridle` for Homebrew formula publishing |
| `NPM_TOKEN` | npm access token for publishing `bridle-ai` to npm registry |
| `DISCORD_TEST_WEBHOOK` | Discord webhook for test release notifications |
| `DISCORD_RELEASE_WEBHOOK` | Discord webhook for release notifications |

### Setting up NPM_TOKEN

1. Go to [npmjs.com](https://www.npmjs.com/) → Access Tokens → Generate New Token
2. Select **Granular Access Token**
3. Set permissions: **Packages and scopes** → **Read and write**
4. Select packages: **All packages** (required for first publish; can narrow after)
5. Add as GitHub repo secret named `NPM_TOKEN`

### Preflight checks before first npm release

```bash
npm whoami                    # Confirm you're logged in
npm view bridle-ai            # Should return 404 (not taken)
```

## Response Times

I maintain this project in my spare time. Please allow 1-3 days for responses on issues and PRs.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
