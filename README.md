# gww - Git Worktree Wrapper

Minimal git worktree manager with an interactive picker.

## Install

```sh
cargo install --path .
```

## Development builds

```sh
cargo build
```

## Requirements

- Rust toolchain (see `rust-toolchain.toml`)
- Git 2.5+ (for `git worktree`)
- Bash or Zsh for `autocd`

## Usage

- `gww checkout|co [branch]` - Checkout a branch into a worktree (fuzzy select when omitted).
- `gww checkout -b <branch>` - Create a branch if it does not exist.
- `gww list|ls` - Show worktrees (raw `git worktree list` output).
- `gww remove|rm [branch]` - Remove a worktree (fuzzy select when omitted).
- `gww autocd` - Emit a shell wrapper for auto-cd behavior.

Worktree root is set by `WORKTREE_ROOT`, defaulting to `$HOME/devel/worktrees`.
Worktrees are stored under `$WORKTREE_ROOT/<repo>/<branch>`.

## Configuration

- `WORKTREE_ROOT` - Base directory for worktrees.
- `GWW_NO_COLOUR` - Disable ANSI colors when set.

## Auto-cd

Add to your shell config:

```sh
source <(gww autocd)
```

When the wrapper is sourced, `gww checkout` prints `GWW_CD:<path>` on success
and the wrapper `cd`s into that path.

## Origin

This is an experiment in vibe coding, although it seems useful in its own right. I previously used [tree-me](https://github.com/haacked/dotfiles/blob/main/bin/tree-me) and found it useful, so most of the inspiration for gww comes from tree-me's behaviour, and the things I thought it could do a bit better for my use (mostly the branch chooser with fuzzy search).
