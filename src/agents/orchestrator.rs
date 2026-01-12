use anyhow::Result;
use std::path::PathBuf;

use crate::llm::OllamaClient;
use crate::storage::{CodebaseIndex, MemoryStore, CrdtMemoryStore};
use crate::sync::P2PSync;
use super::{CodeAgent, SearchAgent, ChatAgent};

const SYNC_PORT: u16 = 7654;

pub struct Orchestrator {
    pub code_agent: CodeAgent,
    pub search_agent: SearchAgent,
    pub chat_agent: ChatAgent,
    pub codebase: Option<CodebaseIndex>,
    pub memory: MemoryStore,
    pub crdt_memory: CrdtMemoryStore,
    pub p2p_sync: P2PSync,
    data_dir: PathBuf,
}

impl Orchestrator {
    pub fn new(model: &str, data_dir: PathBuf) -> Result<Self> {
        let _llm = OllamaClient::new(model);
        let memory = MemoryStore::new(&data_dir)?;
        let crdt_memory = CrdtMemoryStore::new(&data_dir)?;
        let p2p_sync = P2PSync::new(data_dir.clone(), SYNC_PORT);

        let code_llm = OllamaClient::new(model);
        let code_memory = MemoryStore::new(&data_dir)?;
        let code_agent = CodeAgent::new(code_llm, code_memory);

        let search_llm = OllamaClient::new(model);
        let search_agent = SearchAgent::new(search_llm);

        let chat_llm = OllamaClient::new(model);
        let chat_memory = MemoryStore::new(&data_dir)?;
        let chat_agent = ChatAgent::new(chat_llm, chat_memory);

        Ok(Self {
            code_agent,
            search_agent,
            chat_agent,
            codebase: None,
            memory,
            crdt_memory,
            p2p_sync,
            data_dir,
        })
    }

    pub fn index_codebase(&mut self, path: &PathBuf) -> Result<usize> {
        println!("  Indexing codebase at {:?}...", path);
        let index = CodebaseIndex::new(&self.data_dir, path)?;
        let count = index.index_directory(true)?;
        self.codebase = Some(index);

        // Update chat agent with project context
        if let Some(ref idx) = self.codebase {
            if let Ok(stats) = idx.get_stats() {
                let context = format!(
                    "Project: {} files, {} lines of code. Languages: {}",
                    stats.total_files,
                    stats.total_lines,
                    stats.languages.iter()
                        .take(5)
                        .map(|(l, c)| format!("{} ({})", l, c))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                self.chat_agent.set_project_context(context);
            }
        }

        Ok(count)
    }

    pub fn get_codebase_stats(&self) -> Option<crate::storage::codebase::CodebaseStats> {
        self.codebase.as_ref().and_then(|c| c.get_stats().ok())
    }

    pub async fn process_command(&mut self, input: &str) -> Result<String> {
        let input = input.trim();

        // Parse command
        if input.starts_with('/') {
            return self.handle_command(input).await;
        }

        // Default to chat
        self.chat_agent.chat(input).await
    }

    async fn handle_command(&mut self, input: &str) -> Result<String> {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0];
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match cmd {
            "/search" | "/s" => {
                if let Some(ref index) = self.codebase {
                    let results = self.search_agent.semantic_search(index, args, 10).await?;
                    if results.is_empty() {
                        Ok("No results found.".to_string())
                    } else {
                        Ok(results.iter()
                            .map(|r| format!("  {} ({})", r.path, r.language))
                            .collect::<Vec<_>>()
                            .join("\n"))
                    }
                } else {
                    Ok("No codebase indexed. Use /index <path> first.".to_string())
                }
            }

            "/symbol" | "/sym" => {
                if let Some(ref index) = self.codebase {
                    let results = self.search_agent.find_symbol(index, args).await?;
                    if results.is_empty() {
                        Ok("No symbols found.".to_string())
                    } else {
                        Ok(results.iter()
                            .map(|r| format!("  {}: {}", r.path, r.symbols.join(", ")))
                            .collect::<Vec<_>>()
                            .join("\n"))
                    }
                } else {
                    Ok("No codebase indexed. Use /index <path> first.".to_string())
                }
            }

            "/ask" | "/q" => {
                if let Some(ref index) = self.codebase {
                    self.search_agent.answer_question(index, args).await
                } else {
                    Ok("No codebase indexed. Use /index <path> first.".to_string())
                }
            }

            "/explain" | "/e" => {
                self.code_agent.explain_code(args, None).await
            }

            "/generate" | "/gen" | "/g" => {
                self.code_agent.generate_code(args, None, None).await
            }

            "/review" | "/r" => {
                self.code_agent.review_code(args, None).await
            }

            "/test" | "/t" => {
                self.code_agent.write_tests(args, None).await
            }

            "/fix" => {
                // Parse: /fix <bug description> ```code```
                if let Some(code_start) = args.find("```") {
                    let bug_desc = &args[..code_start].trim();
                    let code = args[code_start..]
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();
                    self.code_agent.fix_bug(code, bug_desc, None).await
                } else {
                    Ok("Usage: /fix <bug description> ```code```".to_string())
                }
            }

            "/refactor" | "/ref" => {
                // Parse: /refactor <instructions> ```code```
                if let Some(code_start) = args.find("```") {
                    let instructions = &args[..code_start].trim();
                    let code = args[code_start..]
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();
                    self.code_agent.refactor_code(code, instructions, None).await
                } else {
                    Ok("Usage: /refactor <instructions> ```code```".to_string())
                }
            }

            "/read" | "/cat" => {
                if let Some(ref index) = self.codebase {
                    if let Ok(Some(content)) = index.get_file_content(args) {
                        Ok(content)
                    } else {
                        Ok(format!("File not found: {}", args))
                    }
                } else {
                    Ok("No codebase indexed.".to_string())
                }
            }

            "/summarize" | "/sum" => {
                if let Some(ref index) = self.codebase {
                    self.search_agent.summarize_file(index, args).await
                } else {
                    Ok("No codebase indexed.".to_string())
                }
            }

            "/embed" => {
                if let Some(ref index) = self.codebase {
                    println!("  Building embeddings for semantic search...");
                    let count = self.search_agent.index_embeddings(index).await?;
                    Ok(format!("Created embeddings for {} files.", count))
                } else {
                    Ok("No codebase indexed. Use /index <path> first.".to_string())
                }
            }

            "/stats" => {
                if let Some(stats) = self.get_codebase_stats() {
                    let mut output = format!(
                        "Codebase Statistics:\n  Files: {}\n  Lines: {}\n  Languages:\n",
                        stats.total_files, stats.total_lines
                    );
                    for (lang, count) in &stats.languages {
                        output.push_str(&format!("    {}: {} files\n", lang, count));
                    }
                    if let Some(last) = stats.last_indexed {
                        output.push_str(&format!("  Last indexed: {}", last));
                    }
                    Ok(output)
                } else {
                    Ok("No codebase indexed.".to_string())
                }
            }

            "/memory" | "/mem" => {
                let memories = self.memory.get_recent(10)?;
                if memories.is_empty() {
                    Ok("No memories stored yet.".to_string())
                } else {
                    Ok(memories.iter()
                        .map(|m| format!("  [{}] {}", m.memory_type.as_str(), m.content.chars().take(80).collect::<String>()))
                        .collect::<Vec<_>>()
                        .join("\n"))
                }
            }

            "/sync-export" => {
                let export_path = self.data_dir.join("sync_export.automerge");
                let bytes = self.crdt_memory.export();
                std::fs::write(&export_path, bytes)?;
                Ok(format!("Exported CRDT memories to: {}", export_path.display()))
            }

            "/sync-import" => {
                if args.is_empty() {
                    Ok("Usage: /sync-import <path-to-automerge-file>".to_string())
                } else {
                    let import_path = PathBuf::from(args);
                    if import_path.exists() {
                        let bytes = std::fs::read(&import_path)?;
                        self.crdt_memory.merge(&bytes)?;
                        let count = self.crdt_memory.count()?;
                        Ok(format!("Merged successfully. Total memories: {}", count))
                    } else {
                        Ok(format!("File not found: {}", args))
                    }
                }
            }

            "/sync-status" => {
                let count = self.crdt_memory.count()?;
                let heads = self.crdt_memory.get_heads();
                let conn_info = self.p2p_sync.connection_info();
                Ok(format!(
                    "CRDT Memory Status:\n  Memories: {}\n  Document heads: {}\n  Data dir: {}\n\nP2P Sync:\n  {}",
                    count,
                    heads.len(),
                    self.data_dir.display(),
                    conn_info
                ))
            }

            "/sync-pull" => {
                if args.is_empty() {
                    Ok("Usage: /sync-pull <host:port>".to_string())
                } else {
                    match self.p2p_sync.pull_from_peer(args).await {
                        Ok((data, result)) => {
                            if !data.is_empty() {
                                self.crdt_memory.merge(&data)?;
                                Ok(format!("{}\nMerged into local CRDT.", result))
                            } else {
                                Ok("Received empty data from peer.".to_string())
                            }
                        }
                        Err(e) => Ok(format!("Pull failed: {}", e))
                    }
                }
            }

            "/sync-push" => {
                if args.is_empty() {
                    Ok("Usage: /sync-push <host:port>".to_string())
                } else {
                    match self.p2p_sync.push_to_peer(args).await {
                        Ok(result) => Ok(format!("{}", result)),
                        Err(e) => Ok(format!("Push failed: {}", e))
                    }
                }
            }

            "/sync-live" => {
                if args.is_empty() {
                    Ok("Usage: /sync-live <host:port>".to_string())
                } else {
                    match self.p2p_sync.sync_with_peer(args).await {
                        Ok((data, result)) => {
                            if !data.is_empty() {
                                self.crdt_memory.merge(&data)?;
                                Ok(format!("{}\nBidirectional sync complete.", result))
                            } else {
                                Ok(format!("{}\nNo remote data to merge.", result))
                            }
                        }
                        Err(e) => Ok(format!("Sync failed: {}", e))
                    }
                }
            }

            "/clear" => {
                self.chat_agent.clear_conversation();
                Ok("Conversation cleared.".to_string())
            }

            "/help" | "/h" => {
                Ok(HELP_TEXT.to_string())
            }

            _ => {
                Ok(format!("Unknown command: {}. Type /help for available commands.", cmd))
            }
        }
    }
}

const HELP_TEXT: &str = r#"
Sovereign - Local-First Code Assistant

COMMANDS:
  /search, /s <query>      Search codebase (uses embeddings if available)
  /symbol, /sym <name>     Find symbol definitions
  /ask, /q <question>      Ask about codebase
  /read, /cat <file>       Read file content
  /summarize, /sum <file>  Summarize a file
  /embed                   Build embeddings for semantic search
  /stats                   Show codebase statistics

  /generate, /g <desc>     Generate code
  /explain, /e <code>      Explain code
  /review, /r <code>       Review code
  /test, /t <code>         Generate tests
  /fix <desc> ```code```   Fix a bug
  /refactor <desc> ```code```  Refactor code

  /memory, /mem            Show recent memories
  /clear                   Clear conversation
  /help, /h                Show this help

SYNC (Local-First):
  /sync-export             Export CRDT memories for sync
  /sync-import <file>      Import and merge CRDT memories
  /sync-status             Show CRDT and P2P sync status
  /sync-pull <host:port>   Pull memories from a peer
  /sync-push <host:port>   Push memories to a peer
  /sync-live <host:port>   Bidirectional sync with a peer

Or just type naturally to chat!
"#;
