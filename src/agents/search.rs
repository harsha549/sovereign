use anyhow::Result;
use crate::llm::OllamaClient;
use crate::storage::CodebaseIndex;
use crate::embeddings::{EmbeddingClient, cosine_similarity, find_similar};

pub struct SearchAgent {
    llm: OllamaClient,
    embedding_client: EmbeddingClient,
}

impl SearchAgent {
    pub fn new(llm: OllamaClient) -> Self {
        Self {
            llm,
            embedding_client: EmbeddingClient::new(),
        }
    }

    pub async fn semantic_search(
        &self,
        index: &CodebaseIndex,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        // First, try direct text search
        let direct_results = index.search(query, limit)?;

        // Also try symbol search
        let symbol_results = index.search_by_symbol(query, limit)?;

        // Try embedding-based search if embeddings exist
        let embedding_results = self.embedding_search(index, query, limit).await.ok();

        // Combine and deduplicate results
        let mut results: Vec<SearchResult> = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        // Add embedding results first (higher relevance)
        if let Some(emb_results) = embedding_results {
            for (path, relevance) in emb_results {
                if seen_paths.insert(path.clone()) {
                    // Get file details from index by exact path
                    if let Ok(Some(file)) = index.get_file(&path) {
                        results.push(SearchResult {
                            path: file.relative_path.clone(),
                            language: file.language.clone(),
                            symbols: file.symbols.clone(),
                            relevance,
                            snippet: None,
                        });
                    }
                }
            }
        }

        // Add direct text search results
        for file in direct_results.into_iter().chain(symbol_results.into_iter()) {
            if seen_paths.insert(file.path.clone()) {
                results.push(SearchResult {
                    path: file.relative_path,
                    language: file.language,
                    symbols: file.symbols,
                    relevance: 0.5, // Lower relevance for text match
                    snippet: None,
                });
            }
        }

        // Sort by relevance
        results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        Ok(results)
    }

    async fn embedding_search(
        &self,
        index: &CodebaseIndex,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, f32)>> {
        // Get query embedding
        let query_embedding = self.embedding_client.embed(query).await?;

        // Get all stored embeddings
        let all_embeddings = index.get_all_embeddings()?;

        if all_embeddings.is_empty() {
            return Ok(vec![]);
        }

        // Find similar
        let similar = find_similar(&query_embedding, &all_embeddings, limit);
        Ok(similar)
    }

    pub async fn index_embeddings(&self, index: &CodebaseIndex) -> Result<usize> {
        let files = index.list_files(None, 1000)?;
        let mut count = 0;

        for file in files {
            // Skip if already has embedding
            if index.has_embedding(&file.path) {
                continue;
            }

            // Get file content
            if let Ok(Some(content)) = index.get_file_content(&file.path) {
                // Create embedding text: path + symbols + first 1000 chars
                let embed_text = format!(
                    "{}\n{}\n{}",
                    file.relative_path,
                    file.symbols.join(" "),
                    content.chars().take(1000).collect::<String>()
                );

                // Get embedding
                if let Ok(embedding) = self.embedding_client.embed(&embed_text).await {
                    index.store_embedding(&file.path, &embedding)?;
                    count += 1;

                    if count % 10 == 0 {
                        println!("  Embedded {} files...", count);
                    }
                }
            }
        }

        Ok(count)
    }

    pub async fn find_symbol(&self, index: &CodebaseIndex, symbol: &str) -> Result<Vec<SearchResult>> {
        let files = index.search_by_symbol(symbol, 20)?;

        let results = files
            .into_iter()
            .map(|f| SearchResult {
                path: f.relative_path,
                language: f.language,
                symbols: f.symbols.into_iter().filter(|s| s.contains(symbol)).collect(),
                relevance: 1.0,
                snippet: None,
            })
            .collect();

        Ok(results)
    }

    pub async fn answer_question(
        &self,
        index: &CodebaseIndex,
        question: &str,
    ) -> Result<String> {
        // Use semantic search to find relevant files
        let results = self.semantic_search(index, question, 5).await?;

        let mut context = String::new();
        for result in &results {
            if let Ok(Some(content)) = index.get_file_content(&result.path) {
                // Take first 500 chars of each file
                let snippet = content.chars().take(500).collect::<String>();
                context.push_str(&format!("\n--- {} (relevance: {:.2}) ---\n{}\n",
                    result.path, result.relevance, snippet));
            }
        }

        let prompt = format!(
            "Based on the following code from the project:\n{}\n\nAnswer this question: {}\n\nAnswer:",
            context, question
        );

        let system = "You are a code expert answering questions about a codebase. Be specific and reference file names and code when relevant.";

        self.llm.generate_streaming(&prompt, Some(system)).await
    }

    pub async fn summarize_file(&self, index: &CodebaseIndex, path: &str) -> Result<String> {
        let content = index.get_file_content(path)?
            .ok_or_else(|| anyhow::anyhow!("File not found in index"))?;

        let prompt = format!(
            "Summarize the following code file:\n```\n{}\n```\n\nProvide a brief summary covering:\n1. Purpose of the file\n2. Main components/functions\n3. Dependencies\n4. Key logic\n\nSummary:",
            content
        );

        let system = "You are a code documentation expert. Provide clear, concise summaries that help developers understand code quickly.";

        self.llm.generate_streaming(&prompt, Some(system)).await
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: String,
    pub language: String,
    pub symbols: Vec<String>,
    pub relevance: f32,
    pub snippet: Option<String>,
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}) [{:.0}%]", self.path, self.language, self.relevance * 100.0)?;
        if !self.symbols.is_empty() {
            write!(f, " - {}", self.symbols.join(", "))?;
        }
        Ok(())
    }
}
