use anyhow::Result;
use std::collections::HashMap;

use crate::embeddings::{cosine_similarity, EmbeddingClient};
use crate::storage::CodebaseIndex;

/// Configuration for RAG retrieval
#[derive(Debug, Clone)]
pub struct RagConfig {
    /// Number of top results to retrieve
    pub top_k: usize,
    /// Minimum similarity threshold (0.0 - 1.0)
    pub min_similarity: f32,
    /// Chunk size for splitting large files
    pub chunk_size: usize,
    /// Overlap between chunks
    pub chunk_overlap: usize,
    /// Weight for semantic search (vs keyword)
    pub semantic_weight: f32,
    /// Enable reranking of results
    pub enable_rerank: bool,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k: 10,
            min_similarity: 0.3,
            chunk_size: 1000,
            chunk_overlap: 200,
            semantic_weight: 0.7,
            enable_rerank: true,
        }
    }
}

/// A chunk of code with metadata
#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub file_path: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub embedding: Option<Vec<f32>>,
}

/// Search result with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk: CodeChunk,
    pub score: f32,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    Semantic,
    Keyword,
    Hybrid,
}

/// Improved RAG retriever with hybrid search
pub struct RagRetriever {
    config: RagConfig,
    embedding_client: EmbeddingClient,
}

impl RagRetriever {
    pub fn new(config: RagConfig) -> Self {
        Self {
            config,
            embedding_client: EmbeddingClient::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(RagConfig::default())
    }

    /// Split content into overlapping chunks
    pub fn chunk_content(&self, content: &str, file_path: &str, language: &str) -> Vec<CodeChunk> {
        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::new();

        if lines.is_empty() {
            return chunks;
        }

        // Try to chunk at natural boundaries (functions, classes)
        let boundaries = find_code_boundaries(&lines, language);

        if !boundaries.is_empty() {
            // Use natural boundaries for chunking
            for window in boundaries.windows(2) {
                let start = window[0];
                let end = window[1].min(lines.len());

                let chunk_content: String = lines[start..end].join("\n");
                if !chunk_content.trim().is_empty() {
                    chunks.push(CodeChunk {
                        file_path: file_path.to_string(),
                        content: chunk_content,
                        start_line: start + 1,
                        end_line: end,
                        language: language.to_string(),
                        embedding: None,
                    });
                }
            }
        } else {
            // Fall back to fixed-size chunking with overlap
            let mut start = 0;
            while start < lines.len() {
                let end = (start + self.config.chunk_size).min(lines.len());
                let chunk_content: String = lines[start..end].join("\n");

                if !chunk_content.trim().is_empty() {
                    chunks.push(CodeChunk {
                        file_path: file_path.to_string(),
                        content: chunk_content,
                        start_line: start + 1,
                        end_line: end,
                        language: language.to_string(),
                        embedding: None,
                    });
                }

                if end >= lines.len() {
                    break;
                }
                start = end.saturating_sub(self.config.chunk_overlap);
            }
        }

        chunks
    }

    /// Perform hybrid search (semantic + keyword)
    pub async fn search(
        &self,
        query: &str,
        index: &CodebaseIndex,
    ) -> Result<Vec<SearchResult>> {
        // Get semantic results
        let semantic_results = self.semantic_search(query, index).await?;

        // Get keyword results
        let keyword_results = self.keyword_search(query, index)?;

        // Merge and deduplicate
        let merged = self.merge_results(semantic_results, keyword_results);

        // Rerank if enabled
        let final_results = if self.config.enable_rerank {
            self.rerank_results(query, merged)
        } else {
            merged
        };

        Ok(final_results
            .into_iter()
            .filter(|r| r.score >= self.config.min_similarity)
            .take(self.config.top_k)
            .collect())
    }

    /// Semantic search using embeddings
    pub async fn semantic_search(
        &self,
        query: &str,
        index: &CodebaseIndex,
    ) -> Result<Vec<SearchResult>> {
        // Get query embedding
        let query_embedding = self.embedding_client.embed(query).await?;

        // Get all files with embeddings
        let files = index.search_semantic(&query_embedding, self.config.top_k * 2)?;

        let results: Vec<SearchResult> = files
            .into_iter()
            .map(|(file, score)| {
                let content = std::fs::read_to_string(&file.path).unwrap_or_default();
                let language = detect_language(&file.path);

                SearchResult {
                    chunk: CodeChunk {
                        file_path: file.path.clone(),
                        content,
                        start_line: 1,
                        end_line: file.lines,
                        language,
                        embedding: file.embedding.clone(),
                    },
                    score,
                    match_type: MatchType::Semantic,
                }
            })
            .collect();

        Ok(results)
    }

    /// Keyword search using text matching
    pub fn keyword_search(
        &self,
        query: &str,
        index: &CodebaseIndex,
    ) -> Result<Vec<SearchResult>> {
        let keywords: Vec<&str> = query.split_whitespace().collect();
        let files = index.get_all_files()?;

        let mut results = Vec::new();

        for file in files {
            let content = match std::fs::read_to_string(&file.path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let score = calculate_keyword_score(&content, &keywords);

            if score > 0.0 {
                let language = detect_language(&file.path);
                results.push(SearchResult {
                    chunk: CodeChunk {
                        file_path: file.path.clone(),
                        content,
                        start_line: 1,
                        end_line: file.lines,
                        language,
                        embedding: file.embedding.clone(),
                    },
                    score,
                    match_type: MatchType::Keyword,
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(self.config.top_k * 2);

        Ok(results)
    }

    /// Merge semantic and keyword results
    fn merge_results(
        &self,
        semantic: Vec<SearchResult>,
        keyword: Vec<SearchResult>,
    ) -> Vec<SearchResult> {
        let mut scores: HashMap<String, (f32, f32)> = HashMap::new();
        let mut chunks: HashMap<String, CodeChunk> = HashMap::new();

        // Add semantic scores
        for result in &semantic {
            let key = format!("{}:{}", result.chunk.file_path, result.chunk.start_line);
            scores.entry(key.clone()).or_insert((0.0, 0.0)).0 = result.score;
            chunks.insert(key, result.chunk.clone());
        }

        // Add keyword scores
        for result in &keyword {
            let key = format!("{}:{}", result.chunk.file_path, result.chunk.start_line);
            scores.entry(key.clone()).or_insert((0.0, 0.0)).1 = result.score;
            chunks.entry(key).or_insert(result.chunk.clone());
        }

        // Combine scores
        let semantic_weight = self.config.semantic_weight;
        let keyword_weight = 1.0 - semantic_weight;

        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .map(|(key, (sem, kw))| {
                let combined_score = semantic_weight * sem + keyword_weight * kw;
                let match_type = if sem > 0.0 && kw > 0.0 {
                    MatchType::Hybrid
                } else if sem > 0.0 {
                    MatchType::Semantic
                } else {
                    MatchType::Keyword
                };

                SearchResult {
                    chunk: chunks.remove(&key).unwrap(),
                    score: combined_score,
                    match_type,
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Rerank results based on query relevance
    fn rerank_results(&self, query: &str, mut results: Vec<SearchResult>) -> Vec<SearchResult> {
        // Simple reranking based on query term density and position
        let query_terms: Vec<&str> = query.split_whitespace().collect();

        for result in &mut results {
            let content_lower = result.chunk.content.to_lowercase();
            let mut boost = 0.0;

            for term in &query_terms {
                let term_lower = term.to_lowercase();

                // Boost for exact matches
                if content_lower.contains(&term_lower) {
                    boost += 0.1;
                }

                // Boost for term in function/class names
                if is_in_definition(&result.chunk.content, &term_lower) {
                    boost += 0.2;
                }
            }

            // Boost for smaller, more focused chunks
            let size_factor = 1.0 / (1.0 + (result.chunk.content.len() as f32 / 5000.0));
            boost += size_factor * 0.1;

            result.score = (result.score + boost).min(1.0);
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Build context string from search results
    pub fn build_context(&self, results: &[SearchResult], max_tokens: usize) -> String {
        let mut context = String::new();
        let mut token_count = 0;

        for result in results {
            // Approximate tokens as words / 0.75
            let chunk_tokens = result.chunk.content.split_whitespace().count() * 4 / 3;

            if token_count + chunk_tokens > max_tokens {
                break;
            }

            context.push_str(&format!(
                "\n--- {} (lines {}-{}) ---\n{}\n",
                result.chunk.file_path,
                result.chunk.start_line,
                result.chunk.end_line,
                result.chunk.content
            ));

            token_count += chunk_tokens;
        }

        context
    }
}

/// Find natural code boundaries (functions, classes)
fn find_code_boundaries(lines: &[&str], language: &str) -> Vec<usize> {
    let mut boundaries = vec![0];

    let patterns: Vec<&str> = match language {
        "rust" => vec!["fn ", "impl ", "struct ", "enum ", "trait ", "mod "],
        "python" => vec!["def ", "class ", "async def "],
        "javascript" | "typescript" => vec!["function ", "class ", "const ", "export "],
        "java" | "kotlin" => vec!["public ", "private ", "protected ", "class ", "interface "],
        "go" => vec!["func ", "type ", "package "],
        _ => vec!["fn ", "function ", "def ", "class "],
    };

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if patterns.iter().any(|p| trimmed.starts_with(p)) {
            if i > 0 {
                boundaries.push(i);
            }
        }
    }

    boundaries.push(lines.len());
    boundaries
}

/// Calculate keyword match score
fn calculate_keyword_score(content: &str, keywords: &[&str]) -> f32 {
    if keywords.is_empty() {
        return 0.0;
    }

    let content_lower = content.to_lowercase();
    let mut matches = 0;
    let mut total_occurrences = 0;

    for keyword in keywords {
        let keyword_lower = keyword.to_lowercase();
        if content_lower.contains(&keyword_lower) {
            matches += 1;
            total_occurrences += content_lower.matches(&keyword_lower).count();
        }
    }

    let match_ratio = matches as f32 / keywords.len() as f32;
    let occurrence_boost = (total_occurrences as f32).ln_1p() / 10.0;

    (match_ratio + occurrence_boost).min(1.0)
}

/// Check if term appears in a code definition
fn is_in_definition(content: &str, term: &str) -> bool {
    let def_patterns = [
        format!("fn {}", term),
        format!("function {}", term),
        format!("def {}", term),
        format!("class {}", term),
        format!("struct {}", term),
        format!("const {}", term),
        format!("let {}", term),
        format!("var {}", term),
    ];

    let content_lower = content.to_lowercase();
    def_patterns.iter().any(|p| content_lower.contains(p))
}

/// Detect programming language from file path
fn detect_language(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "jsx" | "tsx" => "react",
        "java" => "java",
        "kt" => "kotlin",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" => "cpp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "scala" => "scala",
        "cs" => "csharp",
        _ => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_content() {
        let retriever = RagRetriever::with_defaults();
        let content = "fn main() {\n    println!(\"Hello\");\n}\n\nfn other() {\n    // code\n}";
        let chunks = retriever.chunk_content(content, "test.rs", "rust");

        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_keyword_score() {
        let content = "fn calculate_total(items: Vec<Item>) -> f32";
        let keywords = vec!["calculate", "total"];
        let score = calculate_keyword_score(content, &keywords);

        assert!(score > 0.0);
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("src/main.rs"), "rust");
        assert_eq!(detect_language("app.py"), "python");
        assert_eq!(detect_language("index.ts"), "typescript");
    }
}
