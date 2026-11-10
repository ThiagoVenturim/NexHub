//! Git worktree management for isolated development floors.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Output;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FloorError {
    #[error("invalid floor name '{0}': names must match ^[a-z0-9][a-z0-9-]{{0,40}}$")]
    InvalidName(String),

    #[error("floor path is outside allowed directory: {0:?}")]
    UnsafePath(PathBuf),

    #[error("git execution failed: {0}")]
    Git(String),

    #[error("worktree for floor '{0}' has uncommitted changes")]
    DirtyWorktree(String),

    #[error("floor '{0}' has commits not pushed to its upstream")]
    UnpushedCommits(String),

    #[error("merge conflict on floor: {0:?}")]
    Conflict(Vec<String>),

    #[error("repository path invalid or has no parent: {0:?}")]
    InvalidRepoPath(PathBuf),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Metadata describing an active development floor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorInfo {
    pub name: String,
    pub branch: String,
    pub path: PathBuf,
    pub head_commit: Option<String>,
}

static FLOOR_NAME_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]{0,40}$").unwrap());

/// Validates that a floor name satisfies security constraints.
pub fn validate_floor_name(name: &str) -> Result<(), FloorError> {
    if !FLOOR_NAME_REGEX.is_match(name) {
        return Err(FloorError::InvalidName(name.to_string()));
    }
    Ok(())
}

/// Unified entry point for executing git commands.
pub fn run_git<P: AsRef<Path>, I, S>(dir: P, args: I) -> Result<Output, FloorError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(dir.as_ref());
    cmd.args(args);
    let output = cmd.output()?;
    Ok(output)
}

/// Computes the filesystem path for a floor worktree and verifies directory containment.
pub fn resolve_floor_path(repo_path: &Path, name: &str) -> Result<PathBuf, FloorError> {
    validate_floor_name(name)?;

    let repo_canonical = dunce_canonicalize(repo_path)?;
    let parent = repo_canonical
        .parent()
        .ok_or_else(|| FloorError::InvalidRepoPath(repo_canonical.clone()))?;
    let repo_name = repo_canonical
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| FloorError::InvalidRepoPath(repo_canonical.clone()))?;

    let floors_dir = parent.join(".nexhub-floors");
    let floor_path = floors_dir.join(format!("{repo_name}--{name}"));

    // Ensure path remains inside floors_dir
    if !floor_path.starts_with(&floors_dir) {
        return Err(FloorError::UnsafePath(floor_path));
    }

    Ok(floor_path)
}

fn dunce_canonicalize(path: &Path) -> Result<PathBuf, FloorError> {
    let p = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        Ok(PathBuf::from(stripped))
    } else {
        Ok(p)
    }
}

/// Creates a new floor worktree and branch `layer/<name>`.
pub fn create_floor(
    repo_path: &Path,
    name: &str,
    base_ref: Option<&str>,
) -> Result<FloorInfo, FloorError> {
    validate_floor_name(name)?;
    let floor_path = resolve_floor_path(repo_path, name)?;

    if let Some(parent) = floor_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Try fetching origin; ignore failure (e.g. offline or no remote)
    let _ = run_git(repo_path, ["fetch", "origin"]);

    let branch_name = format!("layer/{name}");
    let floor_path_str = floor_path.to_string_lossy().to_string();

    // Check if the branch already exists
    let check_branch = run_git(
        repo_path,
        [
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ],
    )?;

    let out = if check_branch.status.success() {
        run_git(
            repo_path,
            ["worktree", "add", &floor_path_str, &branch_name],
        )?
    } else {
        let base = base_ref.unwrap_or("HEAD");
        run_git(
            repo_path,
            ["worktree", "add", "-b", &branch_name, &floor_path_str, base],
        )?
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(FloorError::Git(stderr.trim().to_string()));
    }

    // Retrieve head commit hash
    let rev_out = run_git(&floor_path, ["rev-parse", "HEAD"])?;
    let head_commit = if rev_out.status.success() {
        Some(String::from_utf8_lossy(&rev_out.stdout).trim().to_string())
    } else {
        None
    };

    Ok(FloorInfo {
        name: name.to_string(),
        branch: branch_name,
        path: floor_path,
        head_commit,
    })
}

/// Lists all floors associated with the repository.
pub fn list_floors(repo_path: &Path) -> Result<Vec<FloorInfo>, FloorError> {
    let out = run_git(repo_path, ["worktree", "list", "--porcelain"])?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(FloorError::Git(stderr.trim().to_string()));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut floors = Vec::new();

    let mut current_path: Option<PathBuf> = None;
    let mut current_commit: Option<String> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
                if let Some(floor_name) = branch.strip_prefix("refs/heads/layer/") {
                    floors.push(FloorInfo {
                        name: floor_name.to_string(),
                        branch: format!("layer/{floor_name}"),
                        path,
                        head_commit: current_commit.take(),
                    });
                }
            }
            current_path = None;
            current_commit = None;
            current_branch = None;
            continue;
        }

        if let Some(p) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(p.trim()));
        } else if let Some(c) = line.strip_prefix("HEAD ") {
            current_commit = Some(c.trim().to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            current_branch = Some(b.trim().to_string());
        }
    }

    // Flush last entry if stdout didn't end with a blank line
    if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
        if let Some(floor_name) = branch.strip_prefix("refs/heads/layer/") {
            floors.push(FloorInfo {
                name: floor_name.to_string(),
                branch: format!("layer/{floor_name}"),
                path,
                head_commit: current_commit.take(),
            });
        }
    }

    Ok(floors)
}

/// Removes a floor worktree. Refuses if uncommitted or unpushed changes exist unless `force` is true.
/// Never deletes the underlying branch.
pub fn remove_floor(repo_path: &Path, name: &str, force: bool) -> Result<(), FloorError> {
    validate_floor_name(name)?;
    let floor_path = resolve_floor_path(repo_path, name)?;

    if !force && floor_path.exists() {
        let status_out = run_git(&floor_path, ["status", "--porcelain"])?;
        let status_text = String::from_utf8_lossy(&status_out.stdout);
        if !status_text.trim().is_empty() {
            return Err(FloorError::DirtyWorktree(name.to_string()));
        }

        let upstream_out = run_git(&floor_path, ["rev-list", "@{u}..HEAD"])?;
        if upstream_out.status.success() {
            let unpushed = String::from_utf8_lossy(&upstream_out.stdout);
            if !unpushed.trim().is_empty() {
                return Err(FloorError::UnpushedCommits(name.to_string()));
            }
        }
    }

    let floor_path_str = floor_path.to_string_lossy().to_string();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&floor_path_str);

    let rm_out = run_git(repo_path, args)?;
    if !rm_out.status.success() {
        let stderr = String::from_utf8_lossy(&rm_out.stderr);
        return Err(FloorError::Git(stderr.trim().to_string()));
    }

    Ok(())
}

/// Merges a floor branch into a target branch inside a temporary worktree.
/// On merge conflict, aborts and returns the list of conflicting files. Never pushes.
pub fn merge_floor(repo_path: &Path, name: &str, into: &str) -> Result<(), FloorError> {
    validate_floor_name(name)?;

    let temp_dir = tempfile::tempdir()?;
    let temp_path_str = temp_dir.path().to_string_lossy().to_string();

    let add_out = run_git(repo_path, ["worktree", "add", &temp_path_str, into])?;
    if !add_out.status.success() {
        let stderr = String::from_utf8_lossy(&add_out.stderr);
        return Err(FloorError::Git(stderr.trim().to_string()));
    }

    let branch_name = format!("layer/{name}");
    let merge_out = run_git(temp_dir.path(), ["merge", "--no-ff", &branch_name])?;

    if !merge_out.status.success() {
        let diff_out = run_git(temp_dir.path(), ["diff", "--name-only", "--diff-filter=U"])?;
        let conflicts: Vec<String> = String::from_utf8_lossy(&diff_out.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let _ = run_git(temp_dir.path(), ["merge", "--abort"]);
        let _ = run_git(repo_path, ["worktree", "remove", "--force", &temp_path_str]);
        return Err(FloorError::Conflict(conflicts));
    }

    let rm_out = run_git(repo_path, ["worktree", "remove", &temp_path_str])?;
    if !rm_out.status.success() {
        let _ = run_git(repo_path, ["worktree", "remove", "--force", &temp_path_str]);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_floor_name() {
        assert!(validate_floor_name("core").is_ok());
        assert!(validate_floor_name("feature-123").is_ok());

        assert!(matches!(
            validate_floor_name("../x"),
            Err(FloorError::InvalidName(_))
        ));
        assert!(matches!(
            validate_floor_name("A B"),
            Err(FloorError::InvalidName(_))
        ));
        assert!(matches!(
            validate_floor_name(""),
            Err(FloorError::InvalidName(_))
        ));
        assert!(matches!(
            validate_floor_name("-invalid"),
            Err(FloorError::InvalidName(_))
        ));
    }

    #[test]
    fn test_floor_lifecycle_in_git_repo() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("my-repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        // 1. Initialize git repo and create initial commit
        run_git(&repo_path, ["init"]).unwrap();
        run_git(&repo_path, ["config", "user.name", "Tester"]).unwrap();
        run_git(&repo_path, ["config", "user.email", "tester@example.com"]).unwrap();

        let readme = repo_path.join("README.md");
        std::fs::write(&readme, "Initial repo").unwrap();
        run_git(&repo_path, ["add", "README.md"]).unwrap();
        run_git(&repo_path, ["commit", "-m", "initial commit"]).unwrap();

        // 2. Create floor
        let floor = create_floor(&repo_path, "core", Some("HEAD")).unwrap();
        assert_eq!(floor.name, "core");
        assert_eq!(floor.branch, "layer/core");
        assert!(floor.path.exists());

        // 3. List floors
        let floors = list_floors(&repo_path).unwrap();
        assert_eq!(floors.len(), 1);
        assert_eq!(floors[0].name, "core");
        assert_eq!(floors[0].branch, "layer/core");

        // 4. Dirty worktree: refuse removal
        let dirty_file = floor.path.join("scratch.txt");
        std::fs::write(&dirty_file, "dirty changes").unwrap();

        let err = remove_floor(&repo_path, "core", false);
        assert!(matches!(err, Err(FloorError::DirtyWorktree(_))));

        // Clean up dirty file
        std::fs::remove_file(&dirty_file).unwrap();

        // 5. Clean worktree: removal succeeds
        remove_floor(&repo_path, "core", false).unwrap();
        assert!(!floor.path.exists());

        // 6. Branch still exists after removal
        let branch_check = run_git(
            &repo_path,
            ["show-ref", "--verify", "--quiet", "refs/heads/layer/core"],
        )
        .unwrap();
        assert!(branch_check.status.success());
    }
}
