# gww - Git Worktree Wrapper

Minimal git worktree manager with an interactive picker.

## Usage

- `gww checkout|co [branch]` - Checkout a branch into a worktree (fuzzy select when omitted).
- `gww checkout -b <branch>` - Create a branch if it does not exist.
- `gww list|ls` - Show worktrees (raw `git worktree list` output).
- `gww remove|rm [branch]` - Remove a worktree (fuzzy select when omitted).
- `gww autocd` - Emit a shell wrapper for auto-cd behavior.

Worktree root is set by `WORKTREE_ROOT`, defaulting to `$HOME/devel/worktrees`.
Worktrees are stored under `$WORKTREE_ROOT/<repo>/<branch>`.

## Auto-cd

Add to your shell config:

```sh
source <(gww autocd)
```

When the wrapper is sourced, `gww checkout` prints `GWW_CD:<path>` on success
and the wrapper `cd`s into that path.

## Examples

```sh
gww checkout

gww checkout my-branch

gww checkout -b new-branch

gww list

gww remove
```

## AI Note

This project is an experiment in vibe coding, although it seems useful in its own right.
