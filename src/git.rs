use anyhow::{Context, Result};
use std::process::Command;
use std::path::Path;

/// Represents a parsed git diff hunk
#[derive(Debug, Clone)]
pub struct DiffHunk {
    pub file_path: String,
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub content: String,
}

/// Statistics about a file change
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
    pub additions: u32,
    pub deletions: u32,
    pub old_path: Option<String>, // For renames
}

/// File status in git
#[derive(Debug, Clone, PartialEq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unknown,
}

impl FileStatus {
    pub fn from_char(c: char) -> Self {
        match c {
            'A' => FileStatus::Added,
            'M' => FileStatus::Modified,
            'D' => FileStatus::Deleted,
            'R' => FileStatus::Renamed,
            'C' => FileStatus::Copied,
            _ => FileStatus::Unknown,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            FileStatus::Added => "added",
            FileStatus::Modified => "modified",
            FileStatus::Deleted => "deleted",
            FileStatus::Renamed => "renamed",
            FileStatus::Copied => "copied",
            FileStatus::Unknown => "unknown",
        }
    }
}

/// Represents a git commit
#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

/// Analysis of a diff
#[derive(Debug, Clone)]
pub struct DiffAnalysis {
    pub files: Vec<FileChange>,
    pub hunks: Vec<DiffHunk>,
    pub total_additions: u32,
    pub total_deletions: u32,
    pub summary: String,
}

/// Git operations wrapper
pub struct GitOps {
    repo_path: String,
}

impl GitOps {
    pub fn new<P: AsRef<Path>>(repo_path: P) -> Self {
        Self {
            repo_path: repo_path.as_ref().to_string_lossy().to_string(),
        }
    }

    /// Get the current working directory as a git repo
    pub fn current_dir() -> Result<Self> {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        Ok(Self::new(cwd))
    }

    /// Check if the path is a git repository
    pub fn is_git_repo(&self) -> bool {
        Command::new("git")
            .args(["-C", &self.repo_path, "rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get the staged diff
    pub fn get_staged_diff(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path, "diff", "--cached"])
            .output()
            .context("Failed to run git diff --cached")?;

        if !output.status.success() {
            anyhow::bail!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get the unstaged diff
    pub fn get_unstaged_diff(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path, "diff"])
            .output()
            .context("Failed to run git diff")?;

        if !output.status.success() {
            anyhow::bail!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get diff between two refs
    pub fn get_diff_between(&self, base: &str, head: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path, "diff", &format!("{}...{}", base, head)])
            .output()
            .context("Failed to run git diff")?;

        if !output.status.success() {
            anyhow::bail!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get list of staged files with their status
    pub fn get_staged_files(&self) -> Result<Vec<FileChange>> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path, "diff", "--cached", "--numstat", "--name-status"])
            .output()
            .context("Failed to run git diff --cached --numstat")?;

        if !output.status.success() {
            anyhow::bail!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        self.parse_numstat_output(&String::from_utf8_lossy(&output.stdout))
    }

    /// Get commits between two refs
    pub fn get_commits_between(&self, base: &str, head: &str) -> Result<Vec<Commit>> {
        let output = Command::new("git")
            .args([
                "-C", &self.repo_path,
                "log",
                "--format=%H|%h|%an|%ad|%s",
                "--date=short",
                &format!("{}..{}", base, head),
            ])
            .output()
            .context("Failed to run git log")?;

        if !output.status.success() {
            anyhow::bail!(
                "git log failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let mut commits = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let parts: Vec<&str> = line.splitn(5, '|').collect();
            if parts.len() == 5 {
                commits.push(Commit {
                    hash: parts[0].to_string(),
                    short_hash: parts[1].to_string(),
                    author: parts[2].to_string(),
                    date: parts[3].to_string(),
                    message: parts[4].to_string(),
                });
            }
        }

        Ok(commits)
    }

    /// Get the current branch name
    pub fn get_current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path, "branch", "--show-current"])
            .output()
            .context("Failed to run git branch")?;

        if !output.status.success() {
            anyhow::bail!(
                "git branch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get the default branch (main or master)
    pub fn get_default_branch(&self) -> Result<String> {
        // Try to get from remote
        let output = Command::new("git")
            .args(["-C", &self.repo_path, "symbolic-ref", "refs/remotes/origin/HEAD"])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let refname = String::from_utf8_lossy(&out.stdout);
                if let Some(branch) = refname.trim().strip_prefix("refs/remotes/origin/") {
                    return Ok(branch.to_string());
                }
            }
        }

        // Fall back to checking if main or master exists
        for branch in ["main", "master"] {
            let output = Command::new("git")
                .args(["-C", &self.repo_path, "rev-parse", "--verify", branch])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    return Ok(branch.to_string());
                }
            }
        }

        Ok("main".to_string()) // Default fallback
    }

    /// Get the merge base between current branch and default branch
    pub fn get_merge_base(&self, branch1: &str, branch2: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["-C", &self.repo_path, "merge-base", branch1, branch2])
            .output()
            .context("Failed to run git merge-base")?;

        if !output.status.success() {
            anyhow::bail!(
                "git merge-base failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Parse git diff output to extract hunks
    pub fn parse_diff(&self, diff: &str) -> Result<DiffAnalysis> {
        let mut files = Vec::new();
        let mut hunks = Vec::new();
        let mut current_file: Option<String> = None;
        let mut current_hunk: Option<DiffHunk> = None;
        let mut total_additions = 0u32;
        let mut total_deletions = 0u32;

        for line in diff.lines() {
            // New file header
            if line.starts_with("diff --git") {
                // Save previous hunk
                if let Some(hunk) = current_hunk.take() {
                    hunks.push(hunk);
                }

                // Extract file path from "diff --git a/path b/path"
                if let Some(b_path) = line.split(" b/").last() {
                    current_file = Some(b_path.to_string());
                }
            }
            // File status indicators
            else if line.starts_with("new file mode") {
                if let Some(ref path) = current_file {
                    files.push(FileChange {
                        path: path.clone(),
                        status: FileStatus::Added,
                        additions: 0,
                        deletions: 0,
                        old_path: None,
                    });
                }
            } else if line.starts_with("deleted file mode") {
                if let Some(ref path) = current_file {
                    files.push(FileChange {
                        path: path.clone(),
                        status: FileStatus::Deleted,
                        additions: 0,
                        deletions: 0,
                        old_path: None,
                    });
                }
            } else if line.starts_with("rename from") {
                // Will be handled with "rename to"
            } else if line.starts_with("rename to") {
                if let Some(ref path) = current_file {
                    files.push(FileChange {
                        path: path.clone(),
                        status: FileStatus::Renamed,
                        additions: 0,
                        deletions: 0,
                        old_path: None,
                    });
                }
            }
            // Hunk header: @@ -start,count +start,count @@
            else if line.starts_with("@@") {
                // Save previous hunk
                if let Some(hunk) = current_hunk.take() {
                    hunks.push(hunk);
                }

                // Parse hunk header
                if let Some((old_info, rest)) = line.strip_prefix("@@ -").and_then(|s| s.split_once(' ')) {
                    if let Some((new_info, _)) = rest.strip_prefix('+').and_then(|s| s.split_once(' ')) {
                        let (old_start, old_count) = parse_hunk_range(old_info);
                        let (new_start, new_count) = parse_hunk_range(new_info);

                        current_hunk = Some(DiffHunk {
                            file_path: current_file.clone().unwrap_or_default(),
                            old_start,
                            old_count,
                            new_start,
                            new_count,
                            content: String::new(),
                        });
                    }
                }
            }
            // Content lines
            else if let Some(ref mut hunk) = current_hunk {
                hunk.content.push_str(line);
                hunk.content.push('\n');

                if line.starts_with('+') && !line.starts_with("+++") {
                    total_additions += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    total_deletions += 1;
                }
            }
            // Modified files (catch-all for files we haven't categorized)
            else if line.starts_with("---") || line.starts_with("+++") {
                if let Some(ref path) = current_file {
                    // Only add if not already added
                    if !files.iter().any(|f| f.path == *path) {
                        files.push(FileChange {
                            path: path.clone(),
                            status: FileStatus::Modified,
                            additions: 0,
                            deletions: 0,
                            old_path: None,
                        });
                    }
                }
            }
        }

        // Save last hunk
        if let Some(hunk) = current_hunk {
            hunks.push(hunk);
        }

        // Update file statistics
        for hunk in &hunks {
            if let Some(file) = files.iter_mut().find(|f| f.path == hunk.file_path) {
                for line in hunk.content.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        file.additions += 1;
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        file.deletions += 1;
                    }
                }
            }
        }

        // Generate summary
        let summary = generate_diff_summary(&files, total_additions, total_deletions);

        Ok(DiffAnalysis {
            files,
            hunks,
            total_additions,
            total_deletions,
            summary,
        })
    }

    /// Parse the numstat/name-status output
    fn parse_numstat_output(&self, output: &str) -> Result<Vec<FileChange>> {
        let mut files = Vec::new();
        let lines: Vec<&str> = output.lines().collect();

        for line in lines {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            // Name-status format: M path or R100 old new
            if parts.len() >= 2 && parts[0].len() <= 4 {
                let status_char = parts[0].chars().next().unwrap_or('M');
                let status = FileStatus::from_char(status_char);

                let (path, old_path) = if status == FileStatus::Renamed && parts.len() >= 3 {
                    (parts[2].to_string(), Some(parts[1].to_string()))
                } else {
                    (parts[1].to_string(), None)
                };

                files.push(FileChange {
                    path,
                    status,
                    additions: 0,
                    deletions: 0,
                    old_path,
                });
            }
        }

        Ok(files)
    }
}

/// Parse a hunk range like "10,5" or "10" into (start, count)
fn parse_hunk_range(s: &str) -> (u32, u32) {
    if let Some((start, count)) = s.split_once(',') {
        (
            start.parse().unwrap_or(0),
            count.parse().unwrap_or(1),
        )
    } else {
        (s.parse().unwrap_or(0), 1)
    }
}

/// Generate a human-readable summary of the diff
fn generate_diff_summary(files: &[FileChange], additions: u32, deletions: u32) -> String {
    let file_count = files.len();
    let added_count = files.iter().filter(|f| f.status == FileStatus::Added).count();
    let modified_count = files.iter().filter(|f| f.status == FileStatus::Modified).count();
    let deleted_count = files.iter().filter(|f| f.status == FileStatus::Deleted).count();
    let renamed_count = files.iter().filter(|f| f.status == FileStatus::Renamed).count();

    let mut parts = Vec::new();
    if added_count > 0 {
        parts.push(format!("{} added", added_count));
    }
    if modified_count > 0 {
        parts.push(format!("{} modified", modified_count));
    }
    if deleted_count > 0 {
        parts.push(format!("{} deleted", deleted_count));
    }
    if renamed_count > 0 {
        parts.push(format!("{} renamed", renamed_count));
    }

    format!(
        "{} files changed ({}) with {} additions and {} deletions",
        file_count,
        parts.join(", "),
        additions,
        deletions
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hunk_range() {
        assert_eq!(parse_hunk_range("10,5"), (10, 5));
        assert_eq!(parse_hunk_range("10"), (10, 1));
        assert_eq!(parse_hunk_range("0,0"), (0, 0));
    }

    #[test]
    fn test_file_status() {
        assert_eq!(FileStatus::from_char('A'), FileStatus::Added);
        assert_eq!(FileStatus::from_char('M'), FileStatus::Modified);
        assert_eq!(FileStatus::from_char('D'), FileStatus::Deleted);
        assert_eq!(FileStatus::from_char('R'), FileStatus::Renamed);
        assert_eq!(FileStatus::from_char('X'), FileStatus::Unknown);
    }

    #[test]
    fn test_generate_diff_summary() {
        let files = vec![
            FileChange {
                path: "file1.rs".to_string(),
                status: FileStatus::Added,
                additions: 10,
                deletions: 0,
                old_path: None,
            },
            FileChange {
                path: "file2.rs".to_string(),
                status: FileStatus::Modified,
                additions: 5,
                deletions: 3,
                old_path: None,
            },
        ];

        let summary = generate_diff_summary(&files, 15, 3);
        assert!(summary.contains("2 files changed"));
        assert!(summary.contains("1 added"));
        assert!(summary.contains("1 modified"));
    }
}
