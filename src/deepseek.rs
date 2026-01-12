use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";

/// DeepSeek model options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekModel {
    /// General-purpose chat model
    DeepSeekChat,
    /// Specialized model for code generation and understanding
    DeepSeekCoder,
}

impl DeepSeekModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeepSeekModel::DeepSeekChat => "deepseek-chat",
            DeepSeekModel::DeepSeekCoder => "deepseek-coder",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "deepseek-chat" | "chat" => Some(DeepSeekModel::DeepSeekChat),
            "deepseek-coder" | "coder" => Some(DeepSeekModel::DeepSeekCoder),
            _ => None,
        }
    }
}

impl Default for DeepSeekModel {
    fn default() -> Self {
        DeepSeekModel::DeepSeekChat
    }
}

#[derive(Debug, Clone)]
pub struct DeepSeekClient {
    client: Client,
    api_key: String,
    model: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[allow(dead_code)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ChatMessage>,
    delta: Option<DeltaMessage>,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeltaMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[allow(dead_code)]
    prompt_tokens: u32,
    #[allow(dead_code)]
    completion_tokens: u32,
    #[allow(dead_code)]
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    error_type: Option<String>,
}

impl DeepSeekClient {
    /// Create a new DeepSeek client
    ///
    /// # Arguments
    /// * `api_key` - DeepSeek API key
    /// * `model` - Model name (e.g., "deepseek-chat", "deepseek-coder")
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Create a new DeepSeek client from environment variable
    pub fn from_env(model: &str) -> Result<Self> {
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .context("DEEPSEEK_API_KEY environment variable not set")?;
        Ok(Self::new(&api_key, model))
    }

    /// Get the current model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Switch to a different model
    pub fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    /// List available models
    pub fn list_models() -> Vec<String> {
        vec![
            "deepseek-chat".to_string(),
            "deepseek-coder".to_string(),
        ]
    }

    /// Generate a response (non-streaming)
    pub async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let mut messages = Vec::new();

        if let Some(sys) = system {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        self.chat(&messages, false).await
    }

    /// Generate a response with streaming output
    pub async fn generate_streaming(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let mut messages = Vec::new();

        if let Some(sys) = system {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        });

        self.chat(&messages, true).await
    }

    /// Chat with the model
    pub async fn chat(&self, messages: &[ChatMessage], stream: bool) -> Result<String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            stream,
            temperature: None,
            max_tokens: None,
        };

        if stream {
            self.chat_streaming(&request).await
        } else {
            self.chat_non_streaming(&request).await
        }
    }

    async fn chat_non_streaming(&self, request: &ChatRequest) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/chat/completions", DEEPSEEK_BASE_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .context("Failed to connect to DeepSeek API")?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&body) {
                anyhow::bail!("DeepSeek API error: {}", error_response.error.message);
            }
            anyhow::bail!("DeepSeek API error ({}): {}", status, body);
        }

        let result: ChatResponse = serde_json::from_str(&body)
            .context("Failed to parse DeepSeek response")?;

        Ok(result
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone())
            .unwrap_or_default())
    }

    async fn chat_streaming(&self, request: &ChatRequest) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/chat/completions", DEEPSEEK_BASE_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .context("Failed to connect to DeepSeek API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&body) {
                anyhow::bail!("DeepSeek API error: {}", error_response.error.message);
            }
            anyhow::bail!("DeepSeek API error ({}): {}", status, body);
        }

        let mut stream = response.bytes_stream();
        let mut full_response = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if let Ok(text) = std::str::from_utf8(&chunk) {
                buffer.push_str(text);

                // Process SSE data lines
                for line in buffer.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // SSE format: data: {...}
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            continue;
                        }

                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                            for choice in chunk.choices {
                                if let Some(delta) = choice.delta {
                                    if let Some(content) = delta.content {
                                        print!("{}", content);
                                        io::stdout().flush()?;
                                        full_response.push_str(&content);
                                    }
                                }
                            }
                        }
                    }
                }

                // Keep incomplete line in buffer
                if !buffer.ends_with('\n') {
                    if let Some(last_newline) = buffer.rfind('\n') {
                        buffer = buffer[last_newline + 1..].to_string();
                    }
                } else {
                    buffer.clear();
                }
            }
        }
        println!();

        Ok(full_response)
    }

    /// Check if the API is available and the key is valid
    pub async fn is_available(&self) -> bool {
        // Make a minimal request to check connectivity
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            stream: false,
            temperature: Some(0.0),
            max_tokens: Some(1),
        };

        self.client
            .post(format!("{}/chat/completions", DEEPSEEK_BASE_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Chat with streaming that returns a receiver for chunks instead of printing
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<String>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);

        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            stream: true,
            temperature: None,
            max_tokens: None,
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", DEEPSEEK_BASE_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to connect to DeepSeek API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&body) {
                anyhow::bail!("DeepSeek API error: {}", error_response.error.message);
            }
            anyhow::bail!("DeepSeek API error ({}): {}", status, body);
        }

        let mut stream = response.bytes_stream();

        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                if let Ok(chunk) = chunk {
                    if let Ok(text) = std::str::from_utf8(&chunk) {
                        buffer.push_str(text);

                        // Process SSE data lines
                        for line in buffer.lines() {
                            let line = line.trim();
                            if line.is_empty() {
                                continue;
                            }

                            // SSE format: data: {...}
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    continue;
                                }

                                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                                    for choice in chunk.choices {
                                        if let Some(delta) = choice.delta {
                                            if let Some(content) = delta.content {
                                                if tx.send(content).await.is_err() {
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Clear processed content
                        if buffer.ends_with('\n') {
                            buffer.clear();
                        } else if let Some(last_newline) = buffer.rfind('\n') {
                            buffer = buffer[last_newline + 1..].to_string();
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_from_str() {
        assert_eq!(DeepSeekModel::from_str("deepseek-chat"), Some(DeepSeekModel::DeepSeekChat));
        assert_eq!(DeepSeekModel::from_str("chat"), Some(DeepSeekModel::DeepSeekChat));
        assert_eq!(DeepSeekModel::from_str("deepseek-coder"), Some(DeepSeekModel::DeepSeekCoder));
        assert_eq!(DeepSeekModel::from_str("coder"), Some(DeepSeekModel::DeepSeekCoder));
        assert_eq!(DeepSeekModel::from_str("unknown"), None);
    }

    #[test]
    fn test_model_as_str() {
        assert_eq!(DeepSeekModel::DeepSeekChat.as_str(), "deepseek-chat");
        assert_eq!(DeepSeekModel::DeepSeekCoder.as_str(), "deepseek-coder");
    }

    #[test]
    fn test_list_models() {
        let models = DeepSeekClient::list_models();
        assert!(models.contains(&"deepseek-chat".to_string()));
        assert!(models.contains(&"deepseek-coder".to_string()));
    }
}
