use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::Path;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
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
    messages: Vec<ChatMessageRequest>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Clone)]
struct ChatMessageRequest {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
    #[allow(dead_code)]
    done: bool,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
    #[allow(dead_code)]
    modified_at: Option<String>,
    #[allow(dead_code)]
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelInfo>,
}

/// Image data for multi-modal requests
#[derive(Debug, Clone)]
pub struct ImageInput {
    /// Base64 encoded image data
    pub data: String,
}

impl ImageInput {
    /// Create from a file path
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read image file: {}", path.display()))?;
        Ok(Self {
            data: base64_encode(&data),
        })
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            data: base64_encode(bytes),
        }
    }

    /// Create from base64 string
    pub fn from_base64(data: String) -> Self {
        Self { data }
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let n = match chunk.len() {
            3 => ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32),
            2 => ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8),
            1 => (chunk[0] as u32) << 16,
            _ => unreachable!(),
        };

        result.push(CHARSET[(n >> 18) as usize & 63] as char);
        result.push(CHARSET[(n >> 12) as usize & 63] as char);

        if chunk.len() > 1 {
            result.push(CHARSET[(n >> 6) as usize & 63] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARSET[n as usize & 63] as char);
        } else {
            result.push('=');
        }
    }

    result
}

impl OllamaClient {
    pub fn new(model: &str) -> Self {
        Self {
            client: Client::new(),
            model: model.to_string(),
        }
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
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let response = self
            .client
            .get(format!("{}/api/tags", OLLAMA_BASE_URL))
            .send()
            .await
            .context("Failed to connect to Ollama")?;

        let result: ModelsResponse = response.json().await?;
        Ok(result.models.into_iter().map(|m| m.name).collect())
    }

    /// Check if current model supports vision (images)
    pub fn is_vision_model(&self) -> bool {
        let vision_models = [
            "llava",
            "llava-llama3",
            "llava-phi3",
            "bakllava",
            "moondream",
            "minicpm-v",
        ];
        vision_models.iter().any(|vm| self.model.starts_with(vm))
    }

    pub async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        self.generate_with_images(prompt, system, None).await
    }

    /// Generate with optional images (for vision models)
    pub async fn generate_with_images(
        &self,
        prompt: &str,
        system: Option<&str>,
        images: Option<&[ImageInput]>,
    ) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            system: system.map(|s| s.to_string()),
            context: None,
            images: images.map(|imgs| imgs.iter().map(|i| i.data.clone()).collect()),
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
        self.generate_streaming_with_images(prompt, system, None).await
    }

    /// Generate with streaming and optional images
    pub async fn generate_streaming_with_images(
        &self,
        prompt: &str,
        system: Option<&str>,
        images: Option<&[ImageInput]>,
    ) -> Result<String> {
        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: true,
            system: system.map(|s| s.to_string()),
            context: None,
            images: images.map(|imgs| imgs.iter().map(|i| i.data.clone()).collect()),
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
        self.chat_with_images(messages, stream, None).await
    }

    /// Chat with optional images in the last message (for vision models)
    pub async fn chat_with_images(
        &self,
        messages: &[ChatMessage],
        stream: bool,
        images: Option<&[ImageInput]>,
    ) -> Result<String> {
        // Convert messages to request format, adding images to last user message
        let messages_req: Vec<ChatMessageRequest> = messages
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let is_last_user = i == messages.len() - 1 && m.role == "user";
                ChatMessageRequest {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    images: if is_last_user {
                        images.map(|imgs| imgs.iter().map(|i| i.data.clone()).collect())
                    } else {
                        None
                    },
                }
            })
            .collect();

        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages_req,
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

    /// Analyze an image and describe its contents
    pub async fn analyze_image(&self, image: &ImageInput, prompt: Option<&str>) -> Result<String> {
        let default_prompt = "Describe this image in detail. If it contains code, explain what the code does.";
        let prompt = prompt.unwrap_or(default_prompt);

        self.generate_with_images(prompt, None, Some(&[image.clone()])).await
    }

    /// Analyze code from a screenshot
    pub async fn analyze_code_screenshot(&self, image: &ImageInput) -> Result<String> {
        let prompt = r#"Analyze this code screenshot. Provide:
1. The programming language
2. A summary of what the code does
3. Any potential issues or improvements
4. Key functions or classes visible"#;

        self.generate_with_images(prompt, None, Some(&[image.clone()])).await
    }

    pub async fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", OLLAMA_BASE_URL))
            .send()
            .await
            .is_ok()
    }
}
