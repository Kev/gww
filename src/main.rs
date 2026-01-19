use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use console::style;
use dialoguer::{Confirm, FuzzySelect};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

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
    #[command(hide = true)]
    Timechooser,
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
    summary: BranchSummary,
    is_current: bool,
}

#[derive(Debug, Clone)]
struct BranchSummary {
    timestamp_label: String,
    author: String,
    subject: String,
}

#[derive(Debug, Clone)]
struct BranchMeta {
    timestamp_unix: i64,
    summary: BranchSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchSource {
    Local,
    Remote,
    Worktree,
}

fn main() -> Result<()> {
    configure_colors();
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
        Commands::Timechooser => timechooser(),
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

fn timechooser() -> Result<()> {
    ensure_git_repo()?;
    let start = Instant::now();
    let worktrees = list_worktrees_info()?;
    let local_branches = list_local_branches()?;
    let remote_branches = list_remote_branches()?;
    let candidates = build_branch_candidates(&worktrees, &local_branches, &remote_branches)?;
    let elapsed = start.elapsed();

    println!(
        "Built {} branch entries in {:.2?}",
        candidates.len(),
        elapsed
    );
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

fn sort_by_recent<I>(names: I, meta: &HashMap<String, BranchMeta>) -> Vec<String>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut unique: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for name in names {
        let name = name.as_ref().to_string();
        if seen.insert(name.clone()) {
            unique.push(name);
        }
    }

    unique.sort_by(|a, b| {
        let a_ts = meta.get(a).map(|info| info.timestamp_unix).unwrap_or(0);
        let b_ts = meta.get(b).map(|info| info.timestamp_unix).unwrap_or(0);
        b_ts.cmp(&a_ts).then_with(|| a.cmp(b))
    });
    unique
}

fn batch_branch_metadata() -> Result<HashMap<String, BranchMeta>> {
    let output = git_output([
        "for-each-ref",
        "refs/heads",
        "refs/remotes",
        "--format=%(refname:short)%x1f%(committerdate:unix)%x1f%(committerdate:iso8601-strict)%x1f%(authorname)%x1f%(subject)",
    ])?;
    let mut map = HashMap::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\x1f');
        let refname = parts.next().unwrap_or("").trim().to_string();
        if refname.is_empty() {
            continue;
        }
        let timestamp_unix = parts
            .next()
            .and_then(|value| value.trim().parse::<i64>().ok())
            .unwrap_or(0);
        let timestamp_label = parts.next().unwrap_or("").trim().to_string();
        let author = parts.next().unwrap_or("").trim().to_string();
        let subject = parts.next().unwrap_or("").trim().to_string();
        map.insert(
            refname,
            BranchMeta {
                timestamp_unix,
                summary: BranchSummary {
                    timestamp_label,
                    author,
                    subject,
                },
            },
        );
    }
    Ok(map)
}

fn placeholder_summary() -> BranchSummary {
    BranchSummary {
        timestamp_label: "unknown time".to_string(),
        author: "unknown author".to_string(),
        subject: "unknown subject".to_string(),
    }
}

fn format_branch_item(info: &BranchInfo) -> String {
    let label = match info.source {
        BranchSource::Worktree => "T",
        BranchSource::Local => "L",
        BranchSource::Remote => "R",
    };
    let marker = if info.is_current { "*" } else { " " };
    let tag = format!("[{label}{marker}]");

    let subject = format!("\"{}\"", info.summary.subject);
    let author = format!("[{}]", info.summary.author);
    let timestamp = format!("({})", info.summary.timestamp_label);

    if is_color_enabled() {
        let tag = style(tag).cyan().bold();
        let subject = style(subject).magenta();
        let author = style(author).yellow();
        let timestamp = style(timestamp).dim();
        format!("{} {} {} {} {}", tag, info.name, subject, author, timestamp)
    } else {
        format!(
            "{tag:<4} {} {} {} {}",
            info.name, subject, author, timestamp
        )
    }
}

fn is_color_enabled() -> bool {
    env::var("GWW_NO_COLOUR").is_err()
}

fn configure_colors() {
    if is_color_enabled() {
        console::set_colors_enabled(true);
    } else {
        console::set_colors_enabled(false);
    }
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
        .filter(|s| s.contains('/'))
        .filter(|s| !s.ends_with("/HEAD"))
        .collect();
    Ok(branches)
}

fn current_branch() -> Result<Option<String>> {
    let output = git_output(["rev-parse", "--abbrev-ref", "HEAD"])?;
    let name = output.lines().next().unwrap_or("").trim();
    if name.is_empty() || name == "HEAD" {
        Ok(None)
    } else {
        Ok(Some(name.to_string()))
    }
}

fn select_branch(
    worktrees: &[WorktreeInfo],
    locals: &[String],
    remotes: &[String],
) -> Result<String> {
    let candidates = build_branch_candidates(worktrees, locals, remotes)?;

    if candidates.is_empty() {
        anyhow::bail!("No branches found");
    }

    let items: Vec<String> = candidates.iter().map(|c| format_branch_item(c)).collect();

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

fn build_branch_candidates(
    worktrees: &[WorktreeInfo],
    locals: &[String],
    remotes: &[String],
) -> Result<Vec<BranchInfo>> {
    let mut candidates: Vec<BranchInfo> = Vec::new();
    let worktree_set: HashSet<String> = worktrees
        .iter()
        .filter_map(|wt| wt.branch.clone())
        .collect();
    let meta = batch_branch_metadata()?;

    let current_branch = current_branch()?;
    let mut worktree_names = sort_by_recent(&worktree_set, &meta);
    if let Some(current) = current_branch.as_ref() {
        if let Some(pos) = worktree_names.iter().position(|name| name == current) {
            let current_name = worktree_names.remove(pos);
            worktree_names.insert(0, current_name);
        }
    }
    let local_names = sort_by_recent(locals, &meta);
    let remote_names = sort_by_recent(remotes, &meta);

    for name in worktree_names {
        let summary = meta
            .get(&name)
            .map(|info| info.summary.clone())
            .unwrap_or_else(placeholder_summary);

        candidates.push(BranchInfo {
            is_current: current_branch.as_deref() == Some(&name),
            summary,
            name,
            source: BranchSource::Worktree,
        });
    }

    for name in local_names {
        if !worktree_set.contains(&name) {
            let summary = meta
                .get(&name)
                .map(|info| info.summary.clone())
                .unwrap_or_else(placeholder_summary);

            candidates.push(BranchInfo {
                is_current: current_branch.as_deref() == Some(&name),
                summary,
                name,
                source: BranchSource::Local,
            });
        }
    }

    for name in remote_names {
        let local_name = strip_remote_prefix(&name);
        let has_local = locals.iter().any(|local| local == &local_name);
        if !worktree_set.contains(&local_name) && !has_local {
            let summary = meta
                .get(&name)
                .map(|info| info.summary.clone())
                .unwrap_or_else(placeholder_summary);
            candidates.push(BranchInfo {
                is_current: current_branch.as_deref() == Some(&local_name),
                summary,
                name,
                source: BranchSource::Remote,
            });
        }
    }

    Ok(candidates)
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
