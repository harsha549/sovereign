use anyhow::Result;
use crate::llm::{OllamaClient, ChatMessage};
use crate::storage::MemoryStore;
use crate::storage::memory::MemoryType;

const CHAT_SYSTEM_PROMPT: &str = r#"You are Sovereign, a local-first AI code assistant.
You run entirely on the user's machine - their code never leaves their device.

You can help with:
- Writing and explaining code
- Debugging and code review
- Architecture discussions
- General programming questions

You have persistent memory across sessions and can learn the user's preferences and patterns.
Be helpful, concise, and always prioritize code quality and best practices.

Important: You are running locally via Ollama. This means:
- Complete privacy - no data sent to external servers
- Works offline
- The user owns all data and interactions
"#;

pub struct ChatAgent {
    pub llm: OllamaClient,
    memory: MemoryStore,
    conversation: Vec<ChatMessage>,
    project_context: Option<String>,
}

impl ChatAgent {
    pub fn new(llm: OllamaClient, memory: MemoryStore) -> Self {
        let mut conversation = vec![ChatMessage {
            role: "system".to_string(),
            content: CHAT_SYSTEM_PROMPT.to_string(),
        }];

        Self {
            llm,
            memory,
            conversation,
            project_context: None,
        }
    }

    pub fn set_project_context(&mut self, context: String) {
        self.project_context = Some(context.clone());

        // Add context to system message
        let system_with_context = format!(
            "{}\n\nCurrent project context:\n{}",
            CHAT_SYSTEM_PROMPT, context
        );

        if !self.conversation.is_empty() {
            self.conversation[0].content = system_with_context;
        }
    }

    pub fn add_memory_context(&mut self) {
        // Add recent memories to context
        if let Ok(memories) = self.memory.get_recent(5) {
            if !memories.is_empty() {
                let memory_context: String = memories
                    .iter()
                    .map(|m| format!("- {}", m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let system = &mut self.conversation[0].content;
                system.push_str(&format!(
                    "\n\nRecent memories:\n{}",
                    memory_context
                ));
            }
        }

        // Add user preferences
        if let Ok(preferences) = self.memory.get_by_type(MemoryType::Preference, 5) {
            if !preferences.is_empty() {
                let pref_context: String = preferences
                    .iter()
                    .map(|m| format!("- {}", m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let system = &mut self.conversation[0].content;
                system.push_str(&format!(
                    "\n\nUser preferences:\n{}",
                    pref_context
                ));
            }
        }
    }

    pub async fn chat(&mut self, message: &str) -> Result<String> {
        // Add user message
        self.conversation.push(ChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        });

        // Get response
        let response = self.llm.chat(&self.conversation, true).await?;

        // Add assistant response to conversation
        self.conversation.push(ChatMessage {
            role: "assistant".to_string(),
            content: response.clone(),
        });

        // Store conversation in memory (condensed)
        self.memory.remember(
            &format!("User: {} | Assistant: {}",
                message.chars().take(100).collect::<String>(),
                response.chars().take(100).collect::<String>()
            ),
            MemoryType::Conversation,
            None,
            vec!["chat".to_string()],
            0.5,
        )?;

        // Detect and store preferences
        self.detect_preferences(message, &response)?;

        Ok(response)
    }

    fn detect_preferences(&self, user_msg: &str, _response: &str) -> Result<()> {
        let preference_keywords = [
            ("prefer", 0.8),
            ("always use", 0.9),
            ("i like", 0.7),
            ("don't like", 0.7),
            ("never use", 0.9),
            ("my style", 0.8),
        ];

        let lower_msg = user_msg.to_lowercase();
        for (keyword, importance) in preference_keywords {
            if lower_msg.contains(keyword) {
                self.memory.remember(
                    &format!("Preference: {}", user_msg),
                    MemoryType::Preference,
                    None,
                    vec!["preference".to_string()],
                    importance,
                )?;
                break;
            }
        }

        Ok(())
    }

    pub fn clear_conversation(&mut self) {
        self.conversation.truncate(1); // Keep system message
    }

    pub fn conversation_length(&self) -> usize {
        self.conversation.len() - 1 // Exclude system message
    }
}
