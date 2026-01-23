# AGENTS.md

Purpose

- Quick onboarding for agentic contributors to gww.
- Focus on build/lint/test commands and repo style conventions.

Repository summary

- Rust CLI for managing git worktrees.
- Single binary crate; main logic in `src/main.rs`.
- Uses clap derive for CLI, anyhow for error handling.

Important project files

- `Cargo.toml`: crate metadata and dependencies.
- `rust-toolchain.toml`: required Rust toolchain (1.92.0).
- `src/main.rs`: CLI implementation and unit tests.
- `README.md`: usage and installation notes.

Cursor/Copilot rules

- No `.cursorrules`, `.cursor/rules/`, or `.github/copilot-instructions.md` found.
- If rules are added later, mirror them here verbatim.

Build commands

- `cargo build`: dev build.
- `cargo build --release`: optimized build.
- `cargo install --path .`: local install for manual testing.

Lint/format commands

- `cargo fmt --all`: format code (rustfmt).
- `cargo clippy --all-targets --all-features -- -D warnings`: lint.
- If clippy is not installed: `rustup component add clippy`.
- If rustfmt is not installed: `rustup component add rustfmt`.

Test commands

- `cargo test`: run all unit tests.
- `cargo test <test_name>`: run a single test by name.
- `cargo test tests::sort_by_recent_orders_by_timestamp_and_dedups`:
  example for a single unit test in `src/main.rs`.
- `cargo test -- --nocapture`: see test output.

Pre-commit expectations

- Always run format, lint, and tests before committing.
- No warnings allowed; clippy must be clean.
- Keep commits scoped to related changes only.

Coding style conventions

Imports

- Group std imports after external crate imports.
- Keep groups alphabetical within each group when practical.
- Prefer explicit imports over glob imports.

Formatting

- Use rustfmt defaults; do not hand-align beyond rustfmt output.
- Keep lines readable; split long format strings and arrays.
- Prefer trailing commas in multiline structs and arrays.

Naming

- Functions/variables: `snake_case`.
- Types/traits/enums: `PascalCase`.
- Constants: `SCREAMING_SNAKE_CASE`.
- Clap subcommands: enum variants in `PascalCase`.
- Prefer clear naming over abbreviations or generic names.

Types and APIs

- Favor concrete types over `dyn` unless required.
- Use `Option<T>` for optional values; avoid sentinel strings.
- Use `Result<T>` for fallible operations; propagate with `?`.
- Prefer `Path`/`PathBuf` for filesystem paths.
- Always prefer legibility and maintainability.

Error handling

- Use `anyhow::Result` for fallible functions.
- Add context with `Context`/`with_context` around IO and git calls.
- Use `anyhow::bail!` for user-facing errors.
- Avoid `unwrap`/`expect` outside tests.

CLI behavior

- Preserve existing CLI surface (commands, aliases, outputs).
- Keep stdout for intended output and stderr for errors.
- Respect `GWW_NO_COLOUR` and existing color behavior.

Git command usage

- All git calls go through `std::process::Command` or `git_output`.
- Keep arguments explicit; avoid shell expansion.
- Preserve existing error messaging on git failures.

Data handling patterns

- Keep branch metadata parsing tolerant of missing fields.

Testing conventions

- Unit tests live in `src/main.rs` under `#[cfg(test)]`.
- Name tests with behavior-oriented, descriptive names.
- Keep tests deterministic and independent of external git state.

Documentation

- Use doc comments (`///`) for non-obvious functions.
- Keep help text consistent with README and clap docs.

Extending the codebase

- If adding new modules, keep `main.rs` focused on wiring.
- Add small helpers near related functionality.
- Update README and this file when behavior changes.

Agent-specific commit rules (from user)

- Commit only after build/lint/test succeed.
- Commit subject must end with `[AI]`.
- Commit message must include prompts used:
  `Prompt: <text>` on the first line of each prompt.
- Keep commits scoped and separate unrelated changes.
- Commit after each change is complete.

Examples

- Single-test run: `cargo test tests::strip_remote_prefix_handles_remote_and_local_names`.
- Lint with warnings as errors: `cargo clippy --all-targets --all-features -- -D warnings`.
- Format: `cargo fmt --all`.

Notes

- `WORKTREE_ROOT` controls where worktrees are stored.
- `GWW_NO_COLOUR` disables ANSI colors.
- Target worktree layout: `$WORKTREE_ROOT/<repo>/<branch>`.
