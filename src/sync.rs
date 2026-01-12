use anyhow::Result;
use std::path::PathBuf;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Simple P2P Sync Service for local network sync
/// Uses direct TCP connections for simplicity and reliability
pub struct P2PSync {
    data_dir: PathBuf,
    port: u16,
}

impl P2PSync {
    /// Create a new P2P sync service
    pub fn new(data_dir: PathBuf, port: u16) -> Self {
        Self { data_dir, port }
    }

    /// Get the local sync file path
    fn sync_file(&self) -> PathBuf {
        self.data_dir.join("memories.automerge")
    }

    /// Start listening for sync requests
    pub async fn start_server(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        println!("  Sync server listening on {}", addr);

        loop {
            let (socket, peer_addr) = listener.accept().await?;
            println!("  Sync connection from {}", peer_addr);

            let sync_file = self.sync_file();
            tokio::spawn(async move {
                if let Err(e) = handle_sync_connection(socket, sync_file).await {
                    eprintln!("  Sync error: {}", e);
                }
            });
        }
    }

    /// Send local data to a peer
    pub async fn push_to_peer(&self, peer_addr: &str) -> Result<SyncResult> {
        let mut stream = TcpStream::connect(peer_addr).await?;

        // Read local CRDT document
        let local_data = if self.sync_file().exists() {
            std::fs::read(self.sync_file())?
        } else {
            return Err(anyhow::anyhow!("No local sync data found"));
        };

        // Send PUSH command
        stream.write_all(b"PUSH").await?;
        stream.write_all(&(local_data.len() as u64).to_be_bytes()).await?;
        stream.write_all(&local_data).await?;

        // Read response
        let mut response = [0u8; 4];
        stream.read_exact(&mut response).await?;

        if &response == b"OK  " {
            Ok(SyncResult {
                bytes_sent: local_data.len(),
                bytes_received: 0,
                status: "Pushed successfully".to_string(),
            })
        } else {
            Err(anyhow::anyhow!("Push failed"))
        }
    }

    /// Pull data from a peer
    pub async fn pull_from_peer(&self, peer_addr: &str) -> Result<(Vec<u8>, SyncResult)> {
        let mut stream = TcpStream::connect(peer_addr).await?;

        // Send PULL command
        stream.write_all(b"PULL").await?;

        // Read response length
        let mut len_bytes = [0u8; 8];
        stream.read_exact(&mut len_bytes).await?;
        let len = u64::from_be_bytes(len_bytes) as usize;

        // Read data
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;

        Ok((data, SyncResult {
            bytes_sent: 4,
            bytes_received: len,
            status: "Pulled successfully".to_string(),
        }))
    }

    /// Sync with a peer (bidirectional merge)
    pub async fn sync_with_peer(&self, peer_addr: &str) -> Result<(Vec<u8>, SyncResult)> {
        let mut stream = TcpStream::connect(peer_addr).await?;

        // Read local CRDT document
        let local_data = if self.sync_file().exists() {
            std::fs::read(self.sync_file())?
        } else {
            vec![]
        };

        // Send SYNC command with our data
        stream.write_all(b"SYNC").await?;
        stream.write_all(&(local_data.len() as u64).to_be_bytes()).await?;
        stream.write_all(&local_data).await?;

        // Read their data back
        let mut len_bytes = [0u8; 8];
        stream.read_exact(&mut len_bytes).await?;
        let len = u64::from_be_bytes(len_bytes) as usize;

        let mut remote_data = vec![0u8; len];
        stream.read_exact(&mut remote_data).await?;

        Ok((remote_data, SyncResult {
            bytes_sent: local_data.len(),
            bytes_received: len,
            status: "Synced successfully".to_string(),
        }))
    }

    /// Get connection info for sharing
    pub fn connection_info(&self) -> ConnectionInfo {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "localhost".to_string());

        ConnectionInfo {
            hostname,
            port: self.port,
            has_data: self.sync_file().exists(),
        }
    }
}

async fn handle_sync_connection(mut socket: TcpStream, sync_file: PathBuf) -> Result<()> {
    let mut cmd = [0u8; 4];
    socket.read_exact(&mut cmd).await?;

    match &cmd {
        b"PUSH" => {
            // Receive data from peer
            let mut len_bytes = [0u8; 8];
            socket.read_exact(&mut len_bytes).await?;
            let len = u64::from_be_bytes(len_bytes) as usize;

            let mut data = vec![0u8; len];
            socket.read_exact(&mut data).await?;

            // Save to temp file and merge
            let temp_file = sync_file.with_extension("incoming");
            std::fs::write(&temp_file, &data)?;

            // TODO: Merge with local using CRDT
            // For now, just acknowledge
            socket.write_all(b"OK  ").await?;
        }
        b"PULL" => {
            // Send our data to peer
            let data = if sync_file.exists() {
                std::fs::read(&sync_file)?
            } else {
                vec![]
            };

            socket.write_all(&(data.len() as u64).to_be_bytes()).await?;
            socket.write_all(&data).await?;
        }
        b"SYNC" => {
            // Bidirectional sync
            // Receive their data
            let mut len_bytes = [0u8; 8];
            socket.read_exact(&mut len_bytes).await?;
            let len = u64::from_be_bytes(len_bytes) as usize;

            let mut remote_data = vec![0u8; len];
            socket.read_exact(&mut remote_data).await?;

            // Send our data
            let local_data = if sync_file.exists() {
                std::fs::read(&sync_file)?
            } else {
                vec![]
            };

            socket.write_all(&(local_data.len() as u64).to_be_bytes()).await?;
            socket.write_all(&local_data).await?;

            // TODO: Actually merge the CRDTs
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown command"));
        }
    }

    Ok(())
}

/// Result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub bytes_sent: usize,
    pub bytes_received: usize,
    pub status: String,
}

impl std::fmt::Display for SyncResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Sent: {} bytes, Received: {} bytes - {}",
            self.bytes_sent, self.bytes_received, self.status
        )
    }
}

/// Connection info for sharing with peers
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub hostname: String,
    pub port: u16,
    pub has_data: bool,
}

impl std::fmt::Display for ConnectionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Host: {}:{}", self.hostname, self.port)?;
        writeln!(f, "Has sync data: {}", if self.has_data { "yes" } else { "no" })
    }
}
