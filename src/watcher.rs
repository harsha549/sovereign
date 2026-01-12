use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

const DEBOUNCE_DELAY: Duration = Duration::from_millis(500);

/// Message sent to orchestrator for reindexing
pub struct IndexMessage {
    pub path: PathBuf,
    pub response_tx: oneshot::Sender<Result<String, String>>,
}

/// File watcher for automatic re-indexing on file changes
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    watched_paths: HashSet<PathBuf>,
}

impl FileWatcher {
    pub fn new(request_tx: mpsc::Sender<super::daemon::OrchestratorMessage>) -> Result<Self> {
        let (tx, mut rx) = mpsc::channel::<Event>(100);

        // Spawn the event processor
        tokio::spawn(async move {
            let mut pending_paths: HashSet<PathBuf> = HashSet::new();
            let mut last_event = Instant::now();

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        for path in event.paths {
                            if should_index(&path) {
                                pending_paths.insert(path);
                            }
                        }
                        last_event = Instant::now();
                    }
                    _ = tokio::time::sleep(DEBOUNCE_DELAY) => {
                        if !pending_paths.is_empty() && last_event.elapsed() >= DEBOUNCE_DELAY {
                            // Process pending changes
                            process_changes(&request_tx, &pending_paths).await;
                            pending_paths.clear();
                        }
                    }
                }
            }
        });

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        let _ = tx.blocking_send(event);
                    }
                    _ => {}
                }
            }
        })?;

        Ok(Self {
            watcher,
            watched_paths: HashSet::new(),
        })
    }

    /// Watch a directory for changes
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        let canonical = path.canonicalize()?;

        if self.watched_paths.contains(&canonical) {
            return Ok(());
        }

        self.watcher.watch(&canonical, RecursiveMode::Recursive)?;
        self.watched_paths.insert(canonical.clone());

        println!("  Watching: {}", canonical.display());
        Ok(())
    }

    /// Stop watching a directory
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        let canonical = path.canonicalize()?;

        if !self.watched_paths.contains(&canonical) {
            return Ok(());
        }

        self.watcher.unwatch(&canonical)?;
        self.watched_paths.remove(&canonical);

        println!("  Stopped watching: {}", canonical.display());
        Ok(())
    }

    /// Get list of watched paths
    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths.iter().cloned().collect()
    }
}

fn should_index(path: &Path) -> bool {
    // Skip hidden files and directories
    if path.file_name()
        .map(|n| n.to_string_lossy().starts_with('.'))
        .unwrap_or(false)
    {
        return false;
    }

    // Skip common non-code directories
    let skip_dirs = [
        "node_modules",
        "target",
        "build",
        "dist",
        ".git",
        "__pycache__",
        "venv",
        ".venv",
    ];

    for component in path.components() {
        if let std::path::Component::Normal(name) = component {
            if skip_dirs.contains(&name.to_string_lossy().as_ref()) {
                return false;
            }
        }
    }

    // Only index code files
    let code_extensions = [
        "rs", "py", "js", "ts", "jsx", "tsx", "java", "kt", "go", "c", "cpp", "h", "hpp",
        "rb", "php", "swift", "scala", "cs", "fs", "clj", "ex", "exs", "erl", "hs",
        "ml", "lua", "r", "jl", "dart", "vue", "svelte", "html", "css", "scss", "sql",
        "sh", "bash", "zsh", "yaml", "yml", "toml", "json", "xml", "md", "txt",
    ];

    path.extension()
        .map(|ext| code_extensions.contains(&ext.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

async fn process_changes(
    request_tx: &mpsc::Sender<super::daemon::OrchestratorMessage>,
    paths: &HashSet<PathBuf>,
) {
    if paths.is_empty() {
        return;
    }

    println!("  Detected {} file change(s), re-indexing...", paths.len());

    // Find the common root directory
    if let Some(first_path) = paths.iter().next() {
        // Find the project root (look for common markers)
        let mut root = first_path.clone();
        while let Some(parent) = root.parent() {
            if parent.join("Cargo.toml").exists()
                || parent.join("package.json").exists()
                || parent.join(".git").exists()
                || parent.join("pyproject.toml").exists()
            {
                root = parent.to_path_buf();
                break;
            }
            root = parent.to_path_buf();
        }

        // Send index command through channel
        let (response_tx, response_rx) = oneshot::channel();
        let msg = super::daemon::OrchestratorMessage {
            input: format!("/index {}", root.display()),
            response_tx,
        };

        if request_tx.send(msg).await.is_ok() {
            match response_rx.await {
                Ok(Ok(result)) => {
                    println!("  Re-indexed: {}", result);
                }
                Ok(Err(e)) => {
                    eprintln!("  Re-index error: {}", e);
                }
                Err(_) => {
                    eprintln!("  Re-index error: response channel closed");
                }
            }
        }
    }
}

/// Simple incremental indexer for single file updates
pub struct IncrementalIndexer;

impl IncrementalIndexer {
    /// Check if a file should be indexed
    pub fn should_index(path: &Path) -> bool {
        should_index(path)
    }
}
