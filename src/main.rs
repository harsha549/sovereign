mod llm;
mod deepseek;
mod storage;
mod agents;
mod embeddings;
mod sync;
mod daemon;
mod watcher;
mod rag;
mod git;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;

use agents::Orchestrator;
use llm::LlmBackend;

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

    /// Model to use (default: qwen2.5-coder:14b for Ollama, deepseek-chat for DeepSeek)
    #[arg(short, long)]
    model: Option<String>,

    /// LLM backend to use (ollama, deepseek)
    #[arg(short, long, default_value = "ollama")]
    backend: String,

    /// API key for DeepSeek (can also use DEEPSEEK_API_KEY env var)
    #[arg(long)]
    api_key: Option<String>,

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

    /// Start background daemon
    Daemon {
        /// Use TCP instead of Unix socket
        #[arg(long)]
        tcp: bool,

        /// TCP port (default: 7655)
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable WebSocket server for real-time streaming
        #[arg(long)]
        websocket: bool,

        /// WebSocket port (default: 7656)
        #[arg(long, default_value = "7656")]
        ws_port: u16,

        /// Watch directories for auto-reindex
        #[arg(short, long)]
        watch: Vec<PathBuf>,
    },

    /// Watch directories for changes and auto-reindex
    Watch {
        /// Directories to watch
        paths: Vec<PathBuf>,
    },

    /// Serve the web UI dashboard
    Serve {
        /// Port to serve web UI on (default: 7657)
        #[arg(short, long, default_value = "7657")]
        port: u16,

        /// Path to web-ui directory (default: ./web-ui)
        #[arg(long)]
        dir: Option<PathBuf>,
    },

    /// Generate a commit message for staged changes
    Commit,

    /// Generate a PR summary for the current branch
    PrSummary,
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

    // Parse backend
    let backend = LlmBackend::from_str(&cli.backend).unwrap_or_else(|| {
        eprintln!("{}", format!("Unknown backend: {}. Using 'ollama'.", cli.backend).yellow());
        LlmBackend::Ollama
    });

    // Determine default model based on backend
    let model = cli.model.unwrap_or_else(|| {
        match backend {
            LlmBackend::Ollama => "qwen2.5-coder:14b".to_string(),
            LlmBackend::DeepSeek => "deepseek-chat".to_string(),
        }
    });

    // Check if backend is available
    let test_client = llm::LlmClient::new(backend, &model, cli.api_key.as_deref());
    match test_client {
        Ok(client) => {
            if !client.is_available().await {
                match backend {
                    LlmBackend::Ollama => {
                        eprintln!("{}", "Error: Ollama is not running.".red());
                        eprintln!("Start Ollama with: {}", "brew services start ollama".cyan());
                        eprintln!("Or run: {}", "ollama serve".cyan());
                    }
                    LlmBackend::DeepSeek => {
                        eprintln!("{}", "Error: Cannot connect to DeepSeek API.".red());
                        eprintln!("Check your API key and internet connection.");
                    }
                }
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Error initializing LLM client: {}", e).red());
            std::process::exit(1);
        }
    }

    match cli.command {
        Some(Commands::Chat { path }) => {
            run_chat(&model, backend, cli.api_key.as_deref(), &data_dir, path).await?;
        }

        Some(Commands::Index { path }) => {
            let mut orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
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
            let orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
            // Need to have indexed first
            println!("{}", "Searching...".cyan());
            let result = orchestrator.chat_agent.llm.generate(&query, None).await?;
            println!("{}", result);
        }

        Some(Commands::Ask { question, path }) => {
            let mut orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir.clone())?;

            if let Some(p) = path {
                orchestrator.index_codebase(&p)?;
            }

            println!("{}", "Thinking...".cyan());
            let result = orchestrator.process_command(&format!("/ask {}", question)).await?;
            println!("\n{}", result);
        }

        Some(Commands::Generate { request }) => {
            let orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
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

            let orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
            println!("{}", "Explaining...".cyan());
            // explain_code uses streaming which prints directly to stdout
            orchestrator.code_agent.explain_code(&code, None).await?;
            println!();
        }

        Some(Commands::Stats) => {
            let orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
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
            let orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
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

        Some(Commands::Daemon { tcp, port, websocket, ws_port, watch }) => {
            println!("{}", BANNER.cyan());
            println!("{}", "Starting Sovereign daemon...".green());

            let mut daemon = daemon::Daemon::new(&model, backend, cli.api_key.as_deref(), data_dir.clone())?;

            // Start file watcher if paths provided
            if !watch.is_empty() {
                println!("Starting file watcher...");
                daemon.start_watcher(watch).await?;
            }

            // Start WebSocket server if enabled (runs in background)
            if websocket {
                let daemon_clone = daemon.clone();
                tokio::spawn(async move {
                    if let Err(e) = daemon_clone.start_websocket(Some(ws_port)).await {
                        eprintln!("WebSocket server error: {}", e);
                    }
                });
            }

            // Start the daemon server
            if tcp {
                daemon.start_tcp(port).await?;
            } else {
                #[cfg(unix)]
                {
                    daemon.start_unix().await?;
                }
                #[cfg(not(unix))]
                {
                    daemon.start_tcp(port).await?;
                }
            }
        }

        Some(Commands::Watch { paths }) => {
            if paths.is_empty() {
                eprintln!("{}", "Error: No paths to watch specified".red());
                std::process::exit(1);
            }

            println!("{}", BANNER.cyan());
            println!("{}", "Starting Sovereign with file watcher...".green());

            // Start daemon with watcher enabled
            let mut daemon = daemon::Daemon::new(&model, backend, cli.api_key.as_deref(), data_dir.clone())?;
            daemon.start_watcher(paths).await?;

            println!("{}", "Watching for changes. Press Ctrl+C to stop.".green());

            // Keep running until interrupted
            tokio::signal::ctrl_c().await?;
            println!("\n{}", "Stopped watching.".yellow());
        }

        Some(Commands::Commit) => {
            let orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
            println!("{}", "Analyzing staged changes...".cyan());
            match orchestrator.git_agent.commit_message_for_staged().await {
                Ok(message) => {
                    println!("\n{}\n", "Suggested commit message:".green());
                    println!("{}", message);
                }
                Err(e) => {
                    println!("{}", format!("Error: {}", e).red());
                }
            }
        }

        Some(Commands::PrSummary) => {
            let orchestrator = Orchestrator::new(&model, backend, cli.api_key.as_deref(), data_dir)?;
            println!("{}", "Analyzing branch changes...".cyan());
            match orchestrator.git_agent.pr_summary_for_branch().await {
                Ok(summary) => {
                    println!("\n{}\n", "PR Summary:".green());
                    println!("{}", summary);
                }
                Err(e) => {
                    println!("{}", format!("Error: {}", e).red());
                }
            }
        }

        Some(Commands::Serve { port, dir }) => {
            println!("{}", BANNER.cyan());
            println!("{}", "Starting Sovereign Web UI server...".green());

            // Determine web-ui directory
            let web_ui_dir = dir.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("web-ui")
            });

            if !web_ui_dir.exists() {
                eprintln!("{}", format!("Error: web-ui directory not found at {}", web_ui_dir.display()).red());
                eprintln!("Make sure the web-ui directory exists or specify the path with --dir");
                std::process::exit(1);
            }

            println!("Serving: {}", web_ui_dir.display().to_string().green());
            println!("URL:     {}", format!("http://localhost:{}", port).cyan());
            println!();
            println!("{}", "Press Ctrl+C to stop.".bright_black());

            // Start simple HTTP server for static files
            serve_web_ui(&web_ui_dir, port).await?;
        }

        None => {
            // Default to chat mode
            run_chat(&model, backend, cli.api_key.as_deref(), &data_dir, None).await?;
        }
    }

    Ok(())
}

async fn run_chat(
    model: &str,
    backend: LlmBackend,
    api_key: Option<&str>,
    data_dir: &PathBuf,
    codebase_path: Option<PathBuf>,
) -> Result<()> {
    println!("{}", BANNER.cyan());
    println!(
        "{}",
        format!("Sovereign v{} - Local-First Code Assistant", VERSION).bright_white()
    );
    println!("{}", "Your code never leaves your machine.".bright_black());
    println!();
    println!("Model: {}", model.green());
    println!("Backend: {}", backend.as_str().green());
    println!("Data:  {}", data_dir.display().to_string().green());
    println!();
    println!("Type {} for commands, or just chat!", "/help".cyan());
    println!("{}", "─".repeat(50).bright_black());

    let mut orchestrator = Orchestrator::new(model, backend, api_key, data_dir.clone())?;

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

/// Serve static files from the web-ui directory
async fn serve_web_ui(dir: &PathBuf, port: u16) -> Result<()> {
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;

    loop {
        match listener.accept().await {
            Ok((mut stream, _)) => {
                let dir = dir.clone();
                tokio::spawn(async move {
                    let mut buffer = [0; 4096];
                    if let Ok(n) = stream.read(&mut buffer).await {
                        let request = String::from_utf8_lossy(&buffer[..n]);

                        // Parse the request path
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("/");

                        // Serve the file
                        let file_path = if path == "/" {
                            dir.join("index.html")
                        } else {
                            dir.join(path.trim_start_matches('/'))
                        };

                        let (status, content_type, body) = if file_path.exists() && file_path.is_file() {
                            let content_type = match file_path.extension().and_then(|e| e.to_str()) {
                                Some("html") => "text/html; charset=utf-8",
                                Some("css") => "text/css; charset=utf-8",
                                Some("js") => "application/javascript; charset=utf-8",
                                Some("json") => "application/json",
                                Some("png") => "image/png",
                                Some("jpg") | Some("jpeg") => "image/jpeg",
                                Some("svg") => "image/svg+xml",
                                Some("ico") => "image/x-icon",
                                _ => "application/octet-stream",
                            };

                            match std::fs::read(&file_path) {
                                Ok(content) => ("200 OK", content_type, content),
                                Err(_) => ("500 Internal Server Error", "text/plain", b"Error reading file".to_vec()),
                            }
                        } else {
                            ("404 Not Found", "text/plain", b"File not found".to_vec())
                        };

                        let response = format!(
                            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
                            status,
                            content_type,
                            body.len()
                        );

                        let _ = stream.write_all(response.as_bytes()).await;
                        let _ = stream.write_all(&body).await;
                    }
                });
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}

