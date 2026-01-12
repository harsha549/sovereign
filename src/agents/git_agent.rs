use anyhow::Result;
use crate::llm::LlmClient;
use crate::git::{GitOps, DiffAnalysis, Commit, FileChange, FileStatus};

const GIT_SYSTEM_PROMPT: &str = r#"You are an expert git assistant running locally on the user's machine.
You help with:
- Writing clear, conventional commit messages
- Summarizing code changes for pull requests
- Analyzing diffs to understand code modifications

When writing commit messages:
1. Use conventional commits format: type(scope): description
2. Types: feat, fix, docs, style, refactor, test, chore, perf
3. Keep the first line under 72 characters
4. Add a body if the change is complex
5. Be specific about what changed and why

When writing PR summaries:
1. Provide a clear overview of all changes
2. Group related commits logically
3. Highlight breaking changes
4. Mention any testing considerations
"#;

/// Analysis result from examining a diff
#[derive(Debug, Clone)]
pub struct DiffInsights {
    pub change_type: ChangeType,
    pub affected_areas: Vec<String>,
    pub complexity: ChangeComplexity,
    pub breaking_potential: bool,
    pub suggested_reviewers: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Feature,
    BugFix,
    Refactor,
    Documentation,
    Test,
    Style,
    Performance,
    Chore,
    Mixed,
}

impl ChangeType {
    pub fn as_str(&self) -> &str {
        match self {
            ChangeType::Feature => "feat",
            ChangeType::BugFix => "fix",
            ChangeType::Refactor => "refactor",
            ChangeType::Documentation => "docs",
            ChangeType::Test => "test",
            ChangeType::Style => "style",
            ChangeType::Performance => "perf",
            ChangeType::Chore => "chore",
            ChangeType::Mixed => "chore",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeComplexity {
    Trivial,   // Simple changes like typos, formatting
    Simple,    // Single file, small changes
    Moderate,  // Multiple files, clear purpose
    Complex,   // Many files, significant changes
    Major,     // Large refactors, architectural changes
}

impl ChangeComplexity {
    pub fn as_str(&self) -> &str {
        match self {
            ChangeComplexity::Trivial => "trivial",
            ChangeComplexity::Simple => "simple",
            ChangeComplexity::Moderate => "moderate",
            ChangeComplexity::Complex => "complex",
            ChangeComplexity::Major => "major",
        }
    }
}

pub struct GitAgent {
    llm: LlmClient,
}

impl GitAgent {
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// Generate a commit message for the given diff
    pub async fn generate_commit_message(&self, diff: &str) -> Result<String> {
        if diff.trim().is_empty() {
            return Ok("No changes staged for commit.".to_string());
        }

        let analysis = self.analyze_diff_locally(diff);

        let prompt = format!(
            r#"Generate a git commit message for the following changes.

Diff summary: {}
Files changed: {}
Change type detected: {}
Complexity: {}

Full diff:
```
{}
```

Write a conventional commit message. First line should be: type(scope): short description
If needed, add a blank line and then a body explaining the why.
Only output the commit message, nothing else."#,
            analysis.summary,
            analysis.affected_areas.join(", "),
            analysis.change_type.as_str(),
            analysis.complexity.as_str(),
            truncate_diff(diff, 4000)
        );

        self.llm.generate(&prompt, Some(GIT_SYSTEM_PROMPT)).await
    }

    /// Generate a PR summary from a list of commits
    pub async fn generate_pr_summary(&self, commits: &[Commit], diff: &str) -> Result<String> {
        if commits.is_empty() {
            return Ok("No commits found for PR summary.".to_string());
        }

        let analysis = self.analyze_diff_locally(diff);

        let commits_text: String = commits
            .iter()
            .map(|c| format!("- {} ({}): {}", c.short_hash, c.date, c.message))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"Generate a pull request summary for the following changes.

Commits ({} total):
{}

Diff summary: {}
Files changed: {}
Total additions: approximately {}
Total deletions: approximately {}

Full diff (truncated):
```
{}
```

Write a PR summary with:
1. A brief overview paragraph
2. A "Changes" section with bullet points
3. A "Testing" section with recommended test scenarios
4. Any "Breaking Changes" if applicable

Format using markdown."#,
            commits.len(),
            commits_text,
            analysis.summary,
            analysis.affected_areas.join(", "),
            count_additions(diff),
            count_deletions(diff),
            truncate_diff(diff, 3000)
        );

        self.llm.generate(&prompt, Some(GIT_SYSTEM_PROMPT)).await
    }

    /// Analyze a diff to understand the changes
    pub async fn analyze_diff(&self, diff: &str) -> Result<DiffInsights> {
        // First do local analysis
        let mut insights = self.analyze_diff_locally(diff);

        // Enhance with LLM if diff is substantial
        if diff.len() > 100 {
            let prompt = format!(
                r#"Analyze this git diff and provide insights.

Diff:
```
{}
```

Respond with a JSON object:
{{
    "change_type": "feat|fix|refactor|docs|test|style|perf|chore",
    "breaking_potential": true|false,
    "summary": "Brief one-line summary of changes"
}}

Only output the JSON, nothing else."#,
                truncate_diff(diff, 3000)
            );

            if let Ok(response) = self.llm.generate(&prompt, Some(GIT_SYSTEM_PROMPT)).await {
                // Try to parse the response
                if let Some(change_type) = extract_json_field(&response, "change_type") {
                    insights.change_type = match change_type.as_str() {
                        "feat" => ChangeType::Feature,
                        "fix" => ChangeType::BugFix,
                        "refactor" => ChangeType::Refactor,
                        "docs" => ChangeType::Documentation,
                        "test" => ChangeType::Test,
                        "style" => ChangeType::Style,
                        "perf" => ChangeType::Performance,
                        "chore" => ChangeType::Chore,
                        _ => insights.change_type,
                    };
                }

                if let Some(breaking) = extract_json_field(&response, "breaking_potential") {
                    insights.breaking_potential = breaking == "true";
                }

                if let Some(summary) = extract_json_field(&response, "summary") {
                    insights.summary = summary;
                }
            }
        }

        Ok(insights)
    }

    /// Perform local analysis of a diff without LLM
    fn analyze_diff_locally(&self, diff: &str) -> DiffInsights {
        let git_ops = GitOps::current_dir().unwrap_or_else(|_| GitOps::new("."));
        let analysis = git_ops.parse_diff(diff).unwrap_or_else(|_| DiffAnalysis {
            files: Vec::new(),
            hunks: Vec::new(),
            total_additions: 0,
            total_deletions: 0,
            summary: String::new(),
        });

        let change_type = detect_change_type(&analysis.files, diff);
        let complexity = detect_complexity(&analysis);
        let affected_areas = extract_affected_areas(&analysis.files);
        let breaking_potential = detect_breaking_changes(diff);

        DiffInsights {
            change_type,
            affected_areas,
            complexity,
            breaking_potential,
            suggested_reviewers: Vec::new(),
            summary: analysis.summary,
        }
    }

    /// Get staged diff and generate commit message
    pub async fn commit_message_for_staged(&self) -> Result<String> {
        let git_ops = GitOps::current_dir()?;

        if !git_ops.is_git_repo() {
            return Ok("Not a git repository.".to_string());
        }

        let diff = git_ops.get_staged_diff()?;
        self.generate_commit_message(&diff).await
    }

    /// Generate PR summary for current branch
    pub async fn pr_summary_for_branch(&self) -> Result<String> {
        let git_ops = GitOps::current_dir()?;

        if !git_ops.is_git_repo() {
            return Ok("Not a git repository.".to_string());
        }

        let current_branch = git_ops.get_current_branch()?;
        let default_branch = git_ops.get_default_branch()?;

        if current_branch == default_branch {
            return Ok(format!(
                "You're on the {} branch. Create a feature branch first.",
                default_branch
            ));
        }

        let commits = git_ops.get_commits_between(&default_branch, &current_branch)?;
        let diff = git_ops.get_diff_between(&default_branch, &current_branch)?;

        self.generate_pr_summary(&commits, &diff).await
    }
}

/// Detect the type of change based on file patterns and content
fn detect_change_type(files: &[FileChange], diff: &str) -> ChangeType {
    let diff_lower = diff.to_lowercase();

    // Check for documentation
    if files.iter().all(|f| {
        f.path.ends_with(".md")
            || f.path.ends_with(".txt")
            || f.path.ends_with(".rst")
            || f.path.contains("docs/")
    }) {
        return ChangeType::Documentation;
    }

    // Check for tests
    if files.iter().all(|f| {
        f.path.contains("test")
            || f.path.contains("spec")
            || f.path.ends_with("_test.rs")
            || f.path.ends_with("_test.go")
            || f.path.ends_with(".test.js")
            || f.path.ends_with(".test.ts")
    }) {
        return ChangeType::Test;
    }

    // Check for style/formatting only
    if diff_lower.contains("formatting")
        || diff_lower.contains("whitespace")
        || files.iter().all(|f| f.additions == f.deletions)
    {
        return ChangeType::Style;
    }

    // Check for bug fixes
    if diff_lower.contains("fix")
        || diff_lower.contains("bug")
        || diff_lower.contains("issue")
        || diff_lower.contains("error")
    {
        return ChangeType::BugFix;
    }

    // Check for new features
    if files.iter().any(|f| f.status == FileStatus::Added)
        || diff_lower.contains("add")
        || diff_lower.contains("implement")
        || diff_lower.contains("feature")
    {
        return ChangeType::Feature;
    }

    // Check for refactoring
    if diff_lower.contains("refactor")
        || diff_lower.contains("rename")
        || diff_lower.contains("move")
        || diff_lower.contains("extract")
    {
        return ChangeType::Refactor;
    }

    // Check for performance
    if diff_lower.contains("performance")
        || diff_lower.contains("optimize")
        || diff_lower.contains("cache")
        || diff_lower.contains("speed")
    {
        return ChangeType::Performance;
    }

    // Default to chore for misc changes
    ChangeType::Chore
}

/// Detect the complexity of changes
fn detect_complexity(analysis: &DiffAnalysis) -> ChangeComplexity {
    let file_count = analysis.files.len();
    let total_changes = analysis.total_additions + analysis.total_deletions;

    match (file_count, total_changes) {
        (0, _) => ChangeComplexity::Trivial,
        (1, 0..=10) => ChangeComplexity::Trivial,
        (1, 11..=50) => ChangeComplexity::Simple,
        (1, _) => ChangeComplexity::Moderate,
        (2..=5, 0..=100) => ChangeComplexity::Moderate,
        (2..=5, _) => ChangeComplexity::Complex,
        (6..=15, _) => ChangeComplexity::Complex,
        _ => ChangeComplexity::Major,
    }
}

/// Extract affected areas from file paths
fn extract_affected_areas(files: &[FileChange]) -> Vec<String> {
    let mut areas = std::collections::HashSet::new();

    for file in files {
        // Extract first directory or file extension as area
        if let Some(first_part) = file.path.split('/').next() {
            if first_part.contains('.') {
                // It's a root file, use extension
                if let Some(ext) = file.path.rsplit('.').next() {
                    areas.insert(ext.to_string());
                }
            } else {
                areas.insert(first_part.to_string());
            }
        }
    }

    areas.into_iter().collect()
}

/// Detect potential breaking changes
fn detect_breaking_changes(diff: &str) -> bool {
    let patterns = [
        "BREAKING",
        "breaking change",
        "incompatible",
        "removed",
        "deprecated",
        "pub fn.*->", // Signature changes
        "pub struct",  // Public type changes
    ];

    let diff_lower = diff.to_lowercase();
    patterns.iter().any(|p| diff_lower.contains(&p.to_lowercase()))
}

/// Truncate a diff to a maximum length
fn truncate_diff(diff: &str, max_len: usize) -> String {
    if diff.len() <= max_len {
        diff.to_string()
    } else {
        format!(
            "{}\n\n... (truncated, {} more characters)",
            &diff[..max_len],
            diff.len() - max_len
        )
    }
}

/// Count additions in a diff
fn count_additions(diff: &str) -> usize {
    diff.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .count()
}

/// Count deletions in a diff
fn count_deletions(diff: &str) -> usize {
    diff.lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .count()
}

/// Extract a field from a simple JSON response
fn extract_json_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!(r#""{}":\s*"?([^",\}}]+)"?"#, field);
    let re = regex_lite(pattern.as_str());
    re.and_then(|r| {
        r.captures(json)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim_matches('"').to_string())
    })
}

/// Simple regex-lite implementation without the regex crate
fn regex_lite(pattern: &str) -> Option<SimpleRegex> {
    Some(SimpleRegex {
        pattern: pattern.to_string(),
    })
}

struct SimpleRegex {
    pattern: String,
}

impl SimpleRegex {
    fn captures<'a>(&self, text: &'a str) -> Option<SimpleCaptures<'a>> {
        // Simple pattern matching for our specific use case
        // Looking for: "field": "value" or "field": value
        let field_name = self.pattern
            .strip_prefix(r#"""#)?
            .split(r#"":\s*"?([^",\}"#)
            .next()?;

        let search_pattern = format!(r#""{}":"#, field_name);
        let start_idx = text.find(&search_pattern)?;
        let value_start = start_idx + search_pattern.len();

        let remaining = &text[value_start..];
        let remaining = remaining.trim_start();

        let (value, _) = if remaining.starts_with('"') {
            // Quoted string
            let after_quote = &remaining[1..];
            let end_quote = after_quote.find('"')?;
            (&after_quote[..end_quote], end_quote + 2)
        } else {
            // Unquoted value (bool, number)
            let end = remaining.find(|c: char| c == ',' || c == '}' || c.is_whitespace())?;
            (&remaining[..end], end)
        };

        Some(SimpleCaptures {
            value: value.to_string(),
        })
    }
}

struct SimpleCaptures<'a> {
    value: String,
    #[allow(dead_code)]
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> SimpleCaptures<'a> {
    fn get(&self, idx: usize) -> Option<SimpleMatch> {
        if idx == 1 {
            Some(SimpleMatch {
                value: self.value.clone(),
            })
        } else {
            None
        }
    }
}

struct SimpleMatch {
    value: String,
}

impl SimpleMatch {
    fn as_str(&self) -> &str {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_diff() {
        let diff = "a".repeat(100);
        let truncated = truncate_diff(&diff, 50);
        assert!(truncated.len() < diff.len());
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn test_count_additions_deletions() {
        let diff = r#"
+added line
+another added
-removed line
 unchanged
+++not a real addition
---not a real deletion
"#;
        assert_eq!(count_additions(diff), 2);
        assert_eq!(count_deletions(diff), 1);
    }

    #[test]
    fn test_detect_complexity() {
        let trivial = DiffAnalysis {
            files: vec![],
            hunks: vec![],
            total_additions: 0,
            total_deletions: 0,
            summary: String::new(),
        };
        assert_eq!(detect_complexity(&trivial), ChangeComplexity::Trivial);
    }
}
