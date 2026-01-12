mod llm;
mod storage;
mod agents;
mod embeddings;
mod sync;
mod rag;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;

use agents::Orchestrator;

const BANNER: &str = r#"
  ____                            _
 / ___|  _____   _____ _ __ ___(_) __ _ _ __
 \___ \ / _ \ \ / / _ \ '__/ _ \ |/ _` | '_ \
  ___) | (_) \ V /  __/ | |  __/ | (_| | | | |
 |____/ \___/ \_/ \___|_|  \___|_|\__, |_| |_|
                                  |___/
"#;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "sovereign")]
#[command(about = "Local-first AI code assistant - your code never leaves your machine")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Model to use (default: qwen2.5-coder:14b)
    #[arg(short, long, default_value = "qwen2.5-coder:14b")]
    model: String,

    /// Data directory for storage
    #[arg(short, long)]
    data_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive chat mode
    Chat {
        /// Path to codebase to index
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Index a codebase
    Index {
        /// Path to codebase
        path: PathBuf,
    },

    /// Search the indexed codebase
    Search {
        /// Search query
        query: String,
    },

    /// Ask a question about the codebase
    Ask {
        /// Question to ask
        question: String,

        /// Path to codebase
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Generate code
    Generate {
        /// Code generation request
        request: String,
    },

    /// Explain code from stdin or file
    Explain {
        /// File to explain (or use stdin)
        file: Option<PathBuf>,
    },

    /// Show codebase statistics
    Stats,

    /// Show stored memories
    Memory {
        /// Number of memories to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine data directory
    let data_dir = cli.data_dir.unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sovereign")
    });

    std::fs::create_dir_all(&data_dir)?;

    // Check if Ollama is available
    let test_client = llm::OllamaClient::new(&cli.model);
    if !test_client.is_available().await {
        eprintln!("{}", "Error: Ollama is not running.".red());
        eprintln!("Start Ollama with: {}", "brew services start ollama".cyan());
        eprintln!("Or run: {}", "ollama serve".cyan());
        std::process::exit(1);
    }

    match cli.command {
        Some(Commands::Chat { path }) => {
            run_chat(&cli.model, &data_dir, path).await?;
        }

        Some(Commands::Index { path }) => {
            let mut orchestrator = Orchestrator::new(&cli.model, data_dir)?;
            println!("{}", "Indexing codebase...".cyan());
            let count = orchestrator.index_codebase(&path)?;
            println!("{}", format!("Indexed {} files.", count).green());

            if let Some(stats) = orchestrator.get_codebase_stats() {
                println!("\nStatistics:");
                println!("  Files: {}", stats.total_files);
                println!("  Lines: {}", stats.total_lines);
                println!("  Languages:");
                for (lang, count) in &stats.languages {
                    println!("    {}: {} files", lang, count);
                }
            }
        }

        Some(Commands::Search { query }) => {
            let orchestrator = Orchestrator::new(&cli.model, data_dir)?;
            // Need to have indexed first
            println!("{}", "Searching...".cyan());
            let result = orchestrator.chat_agent.llm.generate(&query, None).await?;
            println!("{}", result);
        }

        Some(Commands::Ask { question, path }) => {
            let mut orchestrator = Orchestrator::new(&cli.model, data_dir.clone())?;

            if let Some(p) = path {
                orchestrator.index_codebase(&p)?;
            }

            println!("{}", "Thinking...".cyan());
            let result = orchestrator.process_command(&format!("/ask {}", question)).await?;
            println!("\n{}", result);
        }

        Some(Commands::Generate { request }) => {
            let orchestrator = Orchestrator::new(&cli.model, data_dir)?;
            println!("{}", "Generating...".cyan());
            // generate_code uses streaming which prints directly to stdout
            orchestrator.code_agent.generate_code(&request, None, None).await?;
            println!();
        }

        Some(Commands::Explain { file }) => {
            let code = if let Some(f) = file {
                std::fs::read_to_string(f)?
            } else {
                // Read from stdin
                use std::io::Read;
                let mut buffer = String::new();
                std::io::stdin().read_to_string(&mut buffer)?;
                buffer
            };

            let orchestrator = Orchestrator::new(&cli.model, data_dir)?;
            println!("{}", "Explaining...".cyan());
            // explain_code uses streaming which prints directly to stdout
            orchestrator.code_agent.explain_code(&code, None).await?;
            println!();
        }

        Some(Commands::Stats) => {
            let orchestrator = Orchestrator::new(&cli.model, data_dir)?;
            if let Some(stats) = orchestrator.get_codebase_stats() {
                println!("Codebase Statistics:");
                println!("  Files: {}", stats.total_files);
                println!("  Lines: {}", stats.total_lines);
                println!("  Languages:");
                for (lang, count) in &stats.languages {
                    println!("    {}: {} files", lang, count);
                }
            } else {
                println!("No codebase indexed. Run: sovereign index <path>");
            }
        }

        Some(Commands::Memory { limit }) => {
            let orchestrator = Orchestrator::new(&cli.model, data_dir)?;
            let memories = orchestrator.memory.get_recent(limit)?;

            if memories.is_empty() {
                println!("No memories stored yet.");
            } else {
                println!("Recent Memories:");
                for mem in memories {
                    println!(
                        "  [{}] {}",
                        mem.memory_type.as_str().cyan(),
                        mem.content.chars().take(80).collect::<String>()
                    );
                }
            }
        }

        None => {
            // Default to chat mode
            run_chat(&cli.model, &data_dir, None).await?;
        }
    }

    Ok(())
}

async fn run_chat(model: &str, data_dir: &PathBuf, codebase_path: Option<PathBuf>) -> Result<()> {
    println!("{}", BANNER.cyan());
    println!(
        "{}",
        format!("Sovereign v{} - Local-First Code Assistant", VERSION).bright_white()
    );
    println!("{}", "Your code never leaves your machine.".bright_black());
    println!();
    println!("Model: {}", model.green());
    println!("Data:  {}", data_dir.display().to_string().green());
    println!();
    println!("Type {} for commands, or just chat!", "/help".cyan());
    println!("{}", "─".repeat(50).bright_black());

    let mut orchestrator = Orchestrator::new(model, data_dir.clone())?;

    // Index codebase if provided
    if let Some(path) = codebase_path {
        println!("\n{}", "Indexing codebase...".cyan());
        let count = orchestrator.index_codebase(&path)?;
        println!("{}\n", format!("Indexed {} files.", count).green());
    }

    // Add memory context to chat
    orchestrator.chat_agent.add_memory_context();

    // Setup readline
    let mut rl = DefaultEditor::new()?;
    let history_path = data_dir.join("history.txt");
    let _ = rl.load_history(&history_path);

    loop {
        let prompt = format!("{} ", "sovereign>".bright_cyan());
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line);

                // Handle special commands
                if line == "/quit" || line == "/exit" || line == "/q" {
                    println!("{}", "Goodbye!".green());
                    break;
                }

                if line.starts_with("/index ") {
                    let path = PathBuf::from(line.trim_start_matches("/index ").trim());
                    println!("{}", "Indexing...".cyan());
                    match orchestrator.index_codebase(&path) {
                        Ok(count) => println!("{}", format!("Indexed {} files.", count).green()),
                        Err(e) => println!("{}", format!("Error: {}", e).red()),
                    }
                    continue;
                }

                // Process command
                println!();
                match orchestrator.process_command(line).await {
                    Ok(response) => {
                        if !response.is_empty() && !line.starts_with('/') {
                            // Response was already streamed for chat
                        } else if !response.is_empty() {
                            println!("{}", response);
                        }
                    }
                    Err(e) => {
                        println!("{}", format!("Error: {}", e).red());
                    }
                }
                println!();
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "Ctrl-C pressed. Use /quit to exit.".yellow());
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "Goodbye!".green());
                break;
            }
            Err(err) => {
                println!("{}", format!("Error: {:?}", err).red());
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history(&history_path);

    Ok(())
}

