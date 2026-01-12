use anyhow::Result;
use crate::llm::OllamaClient;
use crate::storage::{CodebaseIndex, MemoryStore};
use crate::storage::memory::MemoryType;

const CODE_SYSTEM_PROMPT: &str = r#"You are an expert code assistant running locally on the user's machine.
You have access to their codebase and can help with:
- Writing new code
- Explaining existing code
- Refactoring and optimization
- Bug fixing
- Code review

Always be concise but thorough. When writing code:
1. Follow the existing code style in the project
2. Add comments only where logic is complex
3. Consider edge cases
4. Suggest tests if appropriate

You have access to the following context about the codebase.
"#;

pub struct CodeAgent {
    llm: OllamaClient,
    memory: MemoryStore,
}

impl CodeAgent {
    pub fn new(llm: OllamaClient, memory: MemoryStore) -> Self {
        Self { llm, memory }
    }

    pub async fn generate_code(
        &self,
        request: &str,
        context: Option<&str>,
        language: Option<&str>,
    ) -> Result<String> {
        let mut prompt = String::new();

        // Add language context
        if let Some(lang) = language {
            prompt.push_str(&format!("Language: {}\n\n", lang));
        }

        // Add code context if provided
        if let Some(ctx) = context {
            prompt.push_str(&format!("Existing code context:\n```\n{}\n```\n\n", ctx));
        }

        // Add relevant memories
        if let Ok(memories) = self.memory.get_by_type(MemoryType::CodePattern, 5) {
            if !memories.is_empty() {
                prompt.push_str("Relevant patterns from this project:\n");
                for mem in memories {
                    prompt.push_str(&format!("- {}\n", mem.content));
                }
                prompt.push_str("\n");
            }
        }

        prompt.push_str(&format!("Request: {}\n\nProvide the code:", request));

        let response = self.llm.generate_streaming(&prompt, Some(CODE_SYSTEM_PROMPT)).await?;

        // Store this interaction as a memory
        self.memory.remember(
            &format!("Code request: {} -> Generated code", request),
            MemoryType::Conversation,
            None,
            vec!["code".to_string(), "generation".to_string()],
            0.6,
        )?;

        Ok(response)
    }

    pub async fn explain_code(&self, code: &str, language: Option<&str>) -> Result<String> {
        let mut prompt = String::new();

        if let Some(lang) = language {
            prompt.push_str(&format!("Language: {}\n\n", lang));
        }

        prompt.push_str(&format!(
            "Explain the following code in detail:\n```\n{}\n```\n\nExplanation:",
            code
        ));

        let system = "You are an expert code explainer. Provide clear, educational explanations that help developers understand code. Break down complex logic into simple steps.";

        self.llm.generate_streaming(&prompt, Some(system)).await
    }

    pub async fn refactor_code(
        &self,
        code: &str,
        instructions: &str,
        language: Option<&str>,
    ) -> Result<String> {
        let mut prompt = String::new();

        if let Some(lang) = language {
            prompt.push_str(&format!("Language: {}\n\n", lang));
        }

        prompt.push_str(&format!(
            "Original code:\n```\n{}\n```\n\nRefactoring instructions: {}\n\nRefactored code:",
            code, instructions
        ));

        let system = "You are an expert code refactorer. Improve code quality while maintaining functionality. Focus on readability, performance, and best practices.";

        let response = self.llm.generate_streaming(&prompt, Some(system)).await?;

        // Store refactoring pattern
        self.memory.remember(
            &format!("Refactoring: {}", instructions),
            MemoryType::CodePattern,
            None,
            vec!["refactor".to_string()],
            0.7,
        )?;

        Ok(response)
    }

    pub async fn fix_bug(
        &self,
        code: &str,
        bug_description: &str,
        language: Option<&str>,
    ) -> Result<String> {
        let mut prompt = String::new();

        if let Some(lang) = language {
            prompt.push_str(&format!("Language: {}\n\n", lang));
        }

        prompt.push_str(&format!(
            "Buggy code:\n```\n{}\n```\n\nBug description: {}\n\nFixed code with explanation:",
            code, bug_description
        ));

        let system = "You are an expert debugger. Identify the root cause of bugs and provide fixed code with clear explanations of what was wrong and how you fixed it.";

        self.llm.generate_streaming(&prompt, Some(system)).await
    }

    pub async fn review_code(&self, code: &str, language: Option<&str>) -> Result<String> {
        let mut prompt = String::new();

        if let Some(lang) = language {
            prompt.push_str(&format!("Language: {}\n\n", lang));
        }

        prompt.push_str(&format!(
            "Review the following code:\n```\n{}\n```\n\nProvide a code review covering:\n1. Code quality\n2. Potential bugs\n3. Performance issues\n4. Security concerns\n5. Suggestions for improvement\n\nReview:",
            code
        ));

        let system = "You are a senior code reviewer. Provide constructive, actionable feedback that helps improve code quality. Be specific and cite line numbers when relevant.";

        self.llm.generate_streaming(&prompt, Some(system)).await
    }

    pub async fn write_tests(&self, code: &str, language: Option<&str>) -> Result<String> {
        let mut prompt = String::new();

        if let Some(lang) = language {
            prompt.push_str(&format!("Language: {}\n\n", lang));
        }

        prompt.push_str(&format!(
            "Write comprehensive tests for the following code:\n```\n{}\n```\n\nTests:",
            code
        ));

        let system = "You are a test engineer. Write thorough unit tests that cover edge cases, error conditions, and normal operation. Use the standard testing framework for the language.";

        self.llm.generate_streaming(&prompt, Some(system)).await
    }
}
