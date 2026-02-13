# gww - Git Worktree Wrapper

Minimal git worktree manager with an interactive picker. Allows quick worktree creation, switching and deletion, optionally using a branch chooser with fuzzy-search. Just generally useful if you use multiple worktrees for a project, and reduces the friction so you might consider doing so if you previously haven't. It attempts to be as uncomplicated to use as is reasonable.

## Origin / Motivation / Why

I was using [tree-me](https://github.com/haacked/dotfiles/blob/main/bin/tree-me) to manage my git worktrees easily, and had a few things I would have liked it to do that I felt would be tricky to do in shellscript (especially the branch chooser/fuzzy search). [Phil Haack's explanation of the motivation for tree-me](https://haacked.com/archive/2025/11/21/tree-me/) applies to gww, and he obviously deserves the credit for coming up with the model that gww follows. Notably I don't need the github workflows that tree-me has, so I've not put them in gww.
This started as an experiment in vibe coding, but quickly became useful and replaced tree-me for my daily workflow.

## Requirements

- Rust toolchain (see `rust-toolchain.toml`)
- Git 2.5+ (for `git worktree`)
- Bash or Zsh for `autocd`

## Usage

- `gww checkout|co [branch]` - Checkout a branch into a worktree (fuzzy select when omitted).
- `gww <branch>` - Shortcut for `gww checkout <branch>`.
- `gww checkout -b <branch>` - Create a branch if it does not exist.
- `gww list|ls` - Show worktrees (raw `git worktree list` output).
- `gww remove|rm [branch]` - Remove a worktree (fuzzy select when omitted).
- `gww autocd` - Emit a shell wrapper for auto-cd behavior.

Worktree root is set by `WORKTREE_ROOT`, defaulting to `$HOME/devel/worktrees`.
Worktrees are stored under `$WORKTREE_ROOT/<repo>/<branch>`.

## Examples

- `gww checkout -b tmp/Kev/2026-02-13-update-docs` - create a new branch and check it out in a new worktree.
- `gww main` - change working directory to the worktree that has the 'main' branch checked out.
- `gww` - show worktree/branch picker and change working directory to a worktree for the chosen branch, creating the worktree if necessary.
- `gww rm tmp/Kev/2026-02-13-update-docs` - delete the worktree for the given branch.
- `gww rm` - show worktree picker and delete the chosen worktree.
- `gww ls` - list all worktrees.

## Configuration

- `WORKTREE_ROOT` - Base directory for worktrees.
- `GWW_NO_COLOUR` - Disable ANSI colors when set.
- `GWW_SUBMODULE_ON_CHECKOUT` - Initialize submodules recursively when set.

## Auto-cd

Add to your shell config:

```sh
source <(gww autocd)
```

When the wrapper is sourced, `gww checkout` prints `GWW_CD:<path>` on success
and the wrapper `cd`s into that path.

## Development

## Build

```sh
cargo build
```

### Install a development build

```sh
cargo install --path .
```
