use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dialoguer::{Confirm, FuzzySelect};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CD_PREFIX: &str = "GWW_CD:";

#[derive(Parser)]
#[command(name = "gww", about = "Git worktree wrapper", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Checkout a branch in a worktree
    #[command(alias = "co")]
    Checkout {
        /// Branch name to checkout
        branch: Option<String>,
        /// Create branch if it does not exist
        #[arg(short = 'b')]
        create: bool,
    },
    /// List worktrees
    #[command(alias = "ls")]
    List,
    /// Remove a worktree
    #[command(alias = "rm")]
    Remove {
        /// Branch name to remove
        branch: Option<String>,
    },
    /// Output shell function for auto-cd
    Autocd,
}

#[derive(Debug, Clone)]
struct WorktreeInfo {
    path: PathBuf,
    branch: Option<String>,
}

#[derive(Debug, Clone)]
struct BranchInfo {
    name: String,
    source: BranchSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchSource {
    Local,
    Remote,
    Worktree,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = match cli.command {
        Some(command) => command,
        None => {
            eprintln!(
                "No command provided; defaulting to `checkout`. Use `gww --help` for options."
            );
            return checkout(None, false);
        }
    };

    match command {
        Commands::Checkout { branch, create } => checkout(branch, create),
        Commands::List => list_worktrees(),
        Commands::Remove { branch } => remove_worktree(branch),
        Commands::Autocd => autocd(),
    }
}

fn checkout(branch: Option<String>, create: bool) -> Result<()> {
    ensure_git_repo()?;
    let worktrees = list_worktrees_info()?;
    let local_branches = list_local_branches()?;
    let remote_branches = list_remote_branches()?;

    let selected_branch = match branch {
        Some(branch) => branch,
        None => select_branch(&worktrees, &local_branches, &remote_branches)?,
    };

    if let Some(existing) = worktree_for_branch(&worktrees, &selected_branch) {
        emit_cd(&existing.path);
        return Ok(());
    }

    if local_branches.iter().any(|b| b == &selected_branch) {
        ensure_branch_or_prompt(&selected_branch, create, None)?;
        let path = worktree_path_for_branch(&selected_branch)?;
        git_worktree_add(&path, Some(&selected_branch), None)?;
        emit_cd(&path);
        return Ok(());
    }

    if let Some(remote_ref) = match_remote_branch(&selected_branch, &remote_branches) {
        let local_name = strip_remote_prefix(&remote_ref);
        if let Some(existing) = worktree_for_branch(&worktrees, &local_name) {
            emit_cd(&existing.path);
            return Ok(());
        }
        ensure_branch_or_prompt(&local_name, create, Some(&remote_ref))?;
        let path = worktree_path_for_branch(&local_name)?;
        git_worktree_add(&path, Some(&local_name), Some(&remote_ref))?;
        emit_cd(&path);
        return Ok(());
    }

    ensure_branch_or_prompt(&selected_branch, create, None)?;
    let path = worktree_path_for_branch(&selected_branch)?;
    git_worktree_add(&path, Some(&selected_branch), None)?;
    emit_cd(&path);
    Ok(())
}

fn list_worktrees() -> Result<()> {
    let output = git_output(["worktree", "list"])?;
    print!("{}", output);
    Ok(())
}

fn remove_worktree(branch: Option<String>) -> Result<()> {
    ensure_git_repo()?;
    let worktrees = list_worktrees_info()?;
    let selected_branch = match branch {
        Some(branch) => branch,
        None => select_worktree_branch(&worktrees)?,
    };
    let worktree = worktree_for_branch(&worktrees, &selected_branch)
        .with_context(|| format!("No worktree found for branch '{selected_branch}'"))?;
    git_worktree_remove(&worktree.path)?;
    Ok(())
}

fn autocd() -> Result<()> {
    let script = format!(
        "gww() {{\n    local output\n    output=$(command gww \"$@\")\n    local exit_code=$?\n    echo \"$output\"\n    if [ $exit_code -eq 0 ]; then\n        local cd_path\n        cd_path=$(echo \"$output\" | grep \"^{prefix}\" | cut -d: -f2-)\n        [ -n \"$cd_path\" ] && cd \"$cd_path\"\n    fi\n    return $exit_code\n}}\n\n_gww_cd() {{\n    local output\n    output=$(command gww checkout \"$@\")\n    local exit_code=$?\n    if [ $exit_code -ne 0 ]; then\n        echo \"$output\"\n        return $exit_code\n    fi\n    local cd_path\n    cd_path=$(echo \"$output\" | grep \"^{prefix}\" | cut -d: -f2-)\n    [ -n \"$cd_path\" ] && cd \"$cd_path\"\n}}\n",
        prefix = CD_PREFIX
    );

    print!("{}", script);
    Ok(())
}

fn ensure_git_repo() -> Result<()> {
    git_output(["rev-parse", "--show-toplevel"]).context("Not a git repository")?;
    Ok(())
}

fn git_output<I, S>(args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git").args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(stderr.trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn list_worktrees_info() -> Result<Vec<WorktreeInfo>> {
    let output = git_output(["worktree", "list", "--porcelain"])?;
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in output.lines() {
        if line.starts_with("worktree ") {
            if let Some(path) = current_path.take() {
                worktrees.push(WorktreeInfo {
                    path,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(PathBuf::from(line.trim_start_matches("worktree ")));
        } else if line.starts_with("branch ") {
            let branch = line.trim_start_matches("branch ").trim();
            current_branch = branch.strip_prefix("refs/heads/").map(|b| b.to_string());
        }
    }
    if let Some(path) = current_path {
        worktrees.push(WorktreeInfo {
            path,
            branch: current_branch,
        });
    }

    Ok(worktrees)
}

fn list_local_branches() -> Result<Vec<String>> {
    let output = git_output(["for-each-ref", "refs/heads", "--format=%(refname:short)"])?;
    Ok(output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn list_remote_branches() -> Result<Vec<String>> {
    let output = git_output(["for-each-ref", "refs/remotes", "--format=%(refname:short)"])?;
    let branches = output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| !s.ends_with("/HEAD"))
        .collect();
    Ok(branches)
}

fn select_branch(
    worktrees: &[WorktreeInfo],
    locals: &[String],
    remotes: &[String],
) -> Result<String> {
    let mut candidates: Vec<BranchInfo> = Vec::new();
    for worktree in worktrees.iter().filter_map(|wt| wt.branch.clone()) {
        candidates.push(BranchInfo {
            name: worktree,
            source: BranchSource::Worktree,
        });
    }
    for local in locals {
        if !candidates.iter().any(|c| c.name == *local) {
            candidates.push(BranchInfo {
                name: local.clone(),
                source: BranchSource::Local,
            });
        }
    }
    for remote in remotes {
        if !candidates.iter().any(|c| c.name == *remote) {
            candidates.push(BranchInfo {
                name: remote.clone(),
                source: BranchSource::Remote,
            });
        }
    }

    candidates.sort_by(|a, b| a.name.cmp(&b.name));

    if candidates.is_empty() {
        anyhow::bail!("No branches found");
    }

    let items: Vec<String> = candidates
        .iter()
        .map(|c| match c.source {
            BranchSource::Worktree => format!("{:<4} {}", "[WT]", c.name),
            BranchSource::Local => format!("{:<4} {}", "[L]", c.name),
            BranchSource::Remote => format!("{:<4} {}", "[R]", c.name),
        })
        .collect();

    let selection = FuzzySelect::new()
        .with_prompt("Select branch")
        .items(&items)
        .default(0)
        .interact_opt()?;

    let Some(selection) = selection else {
        anyhow::bail!("Selection cancelled");
    };

    Ok(candidates[selection].name.clone())
}

fn select_worktree_branch(worktrees: &[WorktreeInfo]) -> Result<String> {
    let mut branches: Vec<String> = worktrees
        .iter()
        .filter_map(|wt| wt.branch.clone())
        .collect();
    branches.sort();
    branches.dedup();

    if branches.is_empty() {
        anyhow::bail!("No worktrees found");
    }

    let selection = FuzzySelect::new()
        .with_prompt("Select worktree")
        .items(&branches)
        .default(0)
        .interact_opt()?;

    let Some(selection) = selection else {
        anyhow::bail!("Selection cancelled");
    };

    Ok(branches[selection].clone())
}

fn worktree_for_branch<'a>(
    worktrees: &'a [WorktreeInfo],
    branch: &str,
) -> Option<&'a WorktreeInfo> {
    worktrees
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(branch))
}

fn match_remote_branch(branch: &str, remotes: &[String]) -> Option<String> {
    if remotes.iter().any(|b| b == branch) {
        return Some(branch.to_string());
    }

    for remote in remotes {
        if strip_remote_prefix(remote) == branch {
            return Some(remote.clone());
        }
    }

    None
}

fn strip_remote_prefix(branch: &str) -> String {
    branch
        .split_once('/')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| branch.to_string())
}

fn ensure_branch_or_prompt(branch: &str, create: bool, remote: Option<&str>) -> Result<()> {
    if branch_exists(branch) {
        return Ok(());
    }

    if let Some(remote_ref) = remote {
        if remote_branch_exists(remote_ref) {
            return Ok(());
        }
    }

    if create {
        return Ok(());
    }

    let should_create = Confirm::new()
        .with_prompt(format!("Branch '{branch}' does not exist. Create it?"))
        .default(true)
        .interact()?;

    if should_create {
        Ok(())
    } else {
        anyhow::bail!("Branch '{branch}' does not exist")
    }
}

fn remote_branch_exists(branch: &str) -> bool {
    Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/remotes/{branch}"),
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn branch_exists(branch: &str) -> bool {
    Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn worktree_path_for_branch(branch: &str) -> Result<PathBuf> {
    let root = worktree_root()?;
    let repo = repo_name_stem()?;
    Ok(root.join(repo).join(branch))
}

fn worktree_root() -> Result<PathBuf> {
    if let Ok(root) = env::var("WORKTREE_ROOT") {
        return Ok(PathBuf::from(root));
    }
    let home = env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join("devel").join("worktrees"))
}

fn repo_name_stem() -> Result<String> {
    if let Ok(url) = git_output(["remote", "get-url", "origin"]) {
        if let Some(stem) = repo_name_from_url(url.trim()) {
            return Ok(stem);
        }
    }
    let root = git_output(["rev-parse", "--show-toplevel"])?;
    let path = Path::new(root.trim());
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .context("Unable to determine repository name")
}

fn repo_name_from_url(url: &str) -> Option<String> {
    let cleaned = url.trim_end_matches('/');
    let name = cleaned.rsplit('/').next()?;
    Some(name.trim_end_matches(".git").to_string())
}

fn git_worktree_add(path: &Path, branch: Option<&str>, remote: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.arg("worktree").arg("add").arg(path);

    if let Some(remote_branch) = remote {
        let local_branch = branch.context("local branch required for remote")?;
        cmd.arg("-b").arg(local_branch).arg(remote_branch);
    } else if let Some(branch) = branch {
        if branch_exists(branch) {
            cmd.arg(branch);
        } else {
            cmd.arg("-b").arg(branch);
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let status = cmd.status().context("Failed to run git worktree add")?;
    if !status.success() {
        anyhow::bail!("git worktree add failed");
    }
    Ok(())
}

fn git_worktree_remove(path: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["worktree", "remove"])
        .arg(path)
        .status()
        .context("Failed to run git worktree remove")?;
    if !status.success() {
        anyhow::bail!("git worktree remove failed");
    }
    Ok(())
}

fn emit_cd(path: &Path) {
    println!("{CD_PREFIX}{}", path.display());
}
