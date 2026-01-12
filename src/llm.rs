use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

const OLLAMA_BASE_URL: &str = "http://localhost:11434";

#[derive(Debug, Clone)]
pub struct OllamaClient {
    client: Client,
    model: String,
}

#[derive(Debug, Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    system: Option<String>,
    context: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
    #[allow(dead_code)]
    done: bool,
    #[allow(dead_code)]
    context: Option<Vec<i64>>,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
    #[allow(dead_code)]
    done: bool,
}

impl OllamaClient {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            model: model.to_string(),
        }
    }

    pub async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(|s| s.to_string()),
            context: None,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", OLLAMA_BASE_URL))
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Ollama")?;

        let result: GenerateResponse = response.json().await?;
        Ok(result.response)
    }

    pub async fn generate_streaming(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: true,
            system: system.map(|s| s.to_string()),
            context: None,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", OLLAMA_BASE_URL))
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Ollama")?;

        let mut stream = response.bytes_stream();
        let mut full_response = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if let Ok(text) = std::str::from_utf8(&chunk) {
                buffer.push_str(text);

                // Process complete JSON objects in buffer
                for line in buffer.lines() {
                    if let Ok(resp) = serde_json::from_str::<GenerateResponse>(line) {
                        print!("{}", resp.response);
                        io::stdout().flush()?;
                        full_response.push_str(&resp.response);
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

    pub async fn chat(&self, messages: &[ChatMessage], stream: bool) -> Result<String> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            stream,
        };

        if stream {
            let response = self
                .client
                .post(format!("{}/api/chat", OLLAMA_BASE_URL))
                .json(&request)
                .send()
                .await
                .context("Failed to connect to Ollama")?;

            let mut stream = response.bytes_stream();
            let mut full_response = String::new();
            let mut buffer = String::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                if let Ok(text) = std::str::from_utf8(&chunk) {
                    buffer.push_str(text);

                    // Process complete lines
                    let lines: Vec<&str> = buffer.lines().collect();
                    for line in &lines {
                        if let Ok(resp) = serde_json::from_str::<ChatResponse>(line) {
                            if let Some(msg) = resp.message {
                                print!("{}", msg.content);
                                io::stdout().flush()?;
                                full_response.push_str(&msg.content);
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
            println!();

            Ok(full_response)
        } else {
            let response = self
                .client
                .post(format!("{}/api/chat", OLLAMA_BASE_URL))
                .json(&request)
                .send()
                .await
                .context("Failed to connect to Ollama")?;

            let result: ChatResponse = response.json().await?;
            Ok(result.message.map(|m| m.content).unwrap_or_default())
        }
    }

    pub async fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", OLLAMA_BASE_URL))
            .send()
            .await
            .is_ok()
    }
}
