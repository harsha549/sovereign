use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::thread;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

use crate::agents::Orchestrator;
use crate::watcher::FileWatcher;

const DEFAULT_PORT: u16 = 7655;
const SOCKET_NAME: &str = "sovereign.sock";

/// Message sent to the orchestrator thread
pub struct OrchestratorMessage {
    pub input: String,
    pub response_tx: oneshot::Sender<Result<String, String>>,
}

/// Daemon server for background Sovereign operation
pub struct Daemon {
    request_tx: mpsc::Sender<OrchestratorMessage>,
    watcher: Option<FileWatcher>,
    data_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonRequest {
    pub command: String,
    pub args: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DaemonResponse {
    pub success: bool,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl Daemon {
    pub fn new(model: &str, data_dir: PathBuf) -> Result<Self> {
        // Create channel for communicating with orchestrator thread
        let (request_tx, request_rx) = mpsc::channel::<OrchestratorMessage>(100);

        // Spawn a dedicated blocking thread for the orchestrator
        let model = model.to_string();
        let data_dir_clone = data_dir.clone();

        thread::spawn(move || {
            // Create a new runtime for this thread
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create runtime");

            rt.block_on(async {
                let mut orchestrator = match Orchestrator::new(&model, data_dir_clone) {
                    Ok(o) => o,
                    Err(e) => {
                        eprintln!("Failed to create orchestrator: {}", e);
                        return;
                    }
                };

                let mut request_rx = request_rx;
                while let Some(msg) = request_rx.recv().await {
                    let result = match orchestrator.process_command(&msg.input).await {
                        Ok(r) => Ok(r),
                        Err(e) => Err(e.to_string()),
                    };
                    let _ = msg.response_tx.send(result);
                }
            });
        });

        Ok(Self {
            request_tx,
            watcher: None,
            data_dir,
        })
    }

    /// Start the daemon with Unix socket (preferred on Unix systems)
    #[cfg(unix)]
    pub async fn start_unix(&self) -> Result<()> {
        let socket_path = self.data_dir.join(SOCKET_NAME);

        // Remove existing socket if present
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }

        let listener = UnixListener::bind(&socket_path)?;
        println!("Sovereign daemon listening on {}", socket_path.display());

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let request_tx = self.request_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_unix_connection(stream, request_tx).await {
                            eprintln!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Accept error: {}", e);
                }
            }
        }
    }

    /// Start the daemon with TCP (cross-platform)
    pub async fn start_tcp(&self, port: Option<u16>) -> Result<()> {
        let port = port.unwrap_or(DEFAULT_PORT);
        let addr = format!("127.0.0.1:{}", port);

        let listener = TcpListener::bind(&addr).await?;
        println!("Sovereign daemon listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    println!("Connection from {}", peer);
                    let request_tx = self.request_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_tcp_connection(stream, request_tx).await {
                            eprintln!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Accept error: {}", e);
                }
            }
        }
    }

    /// Start file watcher for auto-reindex
    pub async fn start_watcher(&mut self, paths: Vec<PathBuf>) -> Result<()> {
        let request_tx = self.request_tx.clone();
        let mut watcher = FileWatcher::new(request_tx)?;

        for path in paths {
            watcher.watch(&path)?;
        }

        self.watcher = Some(watcher);
        Ok(())
    }

    /// Get the request channel for sending commands
    pub fn request_channel(&self) -> mpsc::Sender<OrchestratorMessage> {
        self.request_tx.clone()
    }

    /// Get daemon status
    pub fn status(&self) -> DaemonStatus {
        DaemonStatus {
            running: true,
            watching: self.watcher.is_some(),
            data_dir: self.data_dir.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub watching: bool,
    pub data_dir: PathBuf,
}

#[cfg(unix)]
async fn handle_unix_connection(
    stream: UnixStream,
    request_tx: mpsc::Sender<OrchestratorMessage>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let response = process_request(&line, &request_tx).await;
        let json = serde_json::to_string(&response)? + "\n";
        writer.write_all(json.as_bytes()).await?;
        line.clear();
    }

    Ok(())
}

async fn handle_tcp_connection(
    stream: TcpStream,
    request_tx: mpsc::Sender<OrchestratorMessage>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let response = process_request(&line, &request_tx).await;
        let json = serde_json::to_string(&response)? + "\n";
        writer.write_all(json.as_bytes()).await?;
        line.clear();
    }

    Ok(())
}

async fn process_request(
    request_str: &str,
    request_tx: &mpsc::Sender<OrchestratorMessage>,
) -> DaemonResponse {
    let request: DaemonRequest = match serde_json::from_str(request_str) {
        Ok(r) => r,
        Err(e) => {
            return DaemonResponse {
                success: false,
                result: None,
                error: Some(format!("Invalid request: {}", e)),
            }
        }
    };

    let input = if let Some(args) = &request.args {
        format!("{} {}", request.command, args)
    } else {
        request.command.clone()
    };

    // Send request through channel and wait for response
    let (response_tx, response_rx) = oneshot::channel();
    let msg = OrchestratorMessage {
        input,
        response_tx,
    };

    if request_tx.send(msg).await.is_err() {
        return DaemonResponse {
            success: false,
            result: None,
            error: Some("Orchestrator thread terminated".to_string()),
        };
    }

    match response_rx.await {
        Ok(Ok(result)) => DaemonResponse {
            success: true,
            result: Some(result),
            error: None,
        },
        Ok(Err(e)) => DaemonResponse {
            success: false,
            result: None,
            error: Some(e),
        },
        Err(_) => DaemonResponse {
            success: false,
            result: None,
            error: Some("Response channel closed".to_string()),
        },
    }
}

/// Client for connecting to the daemon
pub struct DaemonClient {
    #[cfg(unix)]
    socket_path: Option<PathBuf>,
    tcp_addr: Option<String>,
}

impl DaemonClient {
    #[cfg(unix)]
    pub fn unix(data_dir: &PathBuf) -> Self {
        Self {
            socket_path: Some(data_dir.join(SOCKET_NAME)),
            tcp_addr: None,
        }
    }

    pub fn tcp(port: Option<u16>) -> Self {
        let port = port.unwrap_or(DEFAULT_PORT);
        Self {
            #[cfg(unix)]
            socket_path: None,
            tcp_addr: Some(format!("127.0.0.1:{}", port)),
        }
    }

    pub async fn send(&self, request: DaemonRequest) -> Result<DaemonResponse> {
        let request_json = serde_json::to_string(&request)? + "\n";

        #[cfg(unix)]
        if let Some(ref socket_path) = self.socket_path {
            let stream = UnixStream::connect(socket_path).await?;
            return self.send_to_unix_stream(stream, &request_json).await;
        }

        if let Some(ref addr) = self.tcp_addr {
            let stream = TcpStream::connect(addr).await?;
            return self.send_to_tcp_stream(stream, &request_json).await;
        }

        Err(anyhow::anyhow!("No connection method specified"))
    }

    #[cfg(unix)]
    async fn send_to_unix_stream(&self, stream: UnixStream, request: &str) -> Result<DaemonResponse> {
        let (reader, mut writer) = stream.into_split();
        writer.write_all(request.as_bytes()).await?;

        let mut reader = BufReader::new(reader);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        let response: DaemonResponse = serde_json::from_str(&response_line)?;
        Ok(response)
    }

    async fn send_to_tcp_stream(&self, stream: TcpStream, request: &str) -> Result<DaemonResponse> {
        let (reader, mut writer) = stream.into_split();
        writer.write_all(request.as_bytes()).await?;

        let mut reader = BufReader::new(reader);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        let response: DaemonResponse = serde_json::from_str(&response_line)?;
        Ok(response)
    }

    pub async fn is_running(&self) -> bool {
        let request = DaemonRequest {
            command: "/stats".to_string(),
            args: None,
        };
        self.send(request).await.is_ok()
    }
}
