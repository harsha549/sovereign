use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OLLAMA_BASE_URL: &str = "http://localhost:11434";
const EMBEDDING_MODEL: &str = "nomic-embed-text";

#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    client: Client,
    model: String,
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl EmbeddingClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            model: EMBEDDING_MODEL.to_string(),
        }
    }

    pub fn with_model(model: &str) -> Self {
        Self {
            client: Client::new(),
            model: model.to_string(),
        }
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/api/embeddings", OLLAMA_BASE_URL))
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Ollama for embeddings")?;

        let result: EmbeddingResponse = response.json().await?;
        Ok(result.embedding)
    }

    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            let embedding = self.embed(text).await?;
            embeddings.push(embedding);
        }
        Ok(embeddings)
    }
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

/// Find most similar items from a collection
pub fn find_similar(
    query_embedding: &[f32],
    embeddings: &[(String, Vec<f32>)],
    top_k: usize,
) -> Vec<(String, f32)> {
    let mut similarities: Vec<(String, f32)> = embeddings
        .iter()
        .map(|(id, emb)| (id.clone(), cosine_similarity(query_embedding, emb)))
        .collect();

    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    similarities.truncate(top_k);
    similarities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }
}
