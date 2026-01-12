use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ignore::WalkBuilder;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub path: String,
    pub relative_path: String,
    pub language: String,
    pub size: u64,
    pub hash: String,
    pub summary: Option<String>,
    pub symbols: Vec<String>,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebaseStats {
    pub total_files: usize,
    pub total_lines: usize,
    pub languages: Vec<(String, usize)>,
    pub last_indexed: Option<DateTime<Utc>>,
}

pub struct CodebaseIndex {
    conn: Connection,
    root_path: PathBuf,
}

impl CodebaseIndex {
    pub fn new(data_dir: &PathBuf, root_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join("codebase.db");
        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                relative_path TEXT NOT NULL,
                language TEXT NOT NULL,
                size INTEGER NOT NULL,
                hash TEXT NOT NULL,
                content TEXT,
                summary TEXT,
                symbols TEXT NOT NULL,
                indexed_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_language ON files(language)",
            [],
        )?;

        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(path, content, symbols)",
            [],
        ).ok(); // Ignore if already exists

        // Embeddings table for semantic search
        conn.execute(
            "CREATE TABLE IF NOT EXISTS embeddings (
                path TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                chunk_index INTEGER DEFAULT 0,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(Self {
            conn,
            root_path: root_path.to_path_buf(),
        })
    }

    pub fn store_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        let embedding_bytes: Vec<u8> = embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        self.conn.execute(
            "INSERT OR REPLACE INTO embeddings (path, embedding, created_at)
             VALUES (?1, ?2, ?3)",
            params![
                path,
                embedding_bytes,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn get_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, embedding FROM embeddings"
        )?;

        let results = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let embedding_bytes: Vec<u8> = row.get(1)?;

                // Convert bytes back to f32
                let embedding: Vec<f32> = embedding_bytes
                    .chunks(4)
                    .map(|chunk| {
                        let bytes: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                        f32::from_le_bytes(bytes)
                    })
                    .collect();

                Ok((path, embedding))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    pub fn has_embedding(&self, path: &str) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM embeddings WHERE path = ?1",
                params![path],
                |_| Ok(()),
            )
            .is_ok()
    }

    pub fn index_directory(&self, show_progress: bool) -> Result<usize> {
        let mut count = 0;
        let walker = WalkBuilder::new(&self.root_path)
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker.flatten() {
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let path = entry.path();
                if let Some(lang) = Self::detect_language(path) {
                    if let Ok(indexed) = self.index_file(path, &lang) {
                        count += 1;
                        if show_progress && count % 100 == 0 {
                            println!("  Indexed {} files...", count);
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    fn index_file(&self, path: &Path, language: &str) -> Result<IndexedFile> {
        let content = fs::read_to_string(path).unwrap_or_default();
        let hash = Self::compute_hash(&content);

        // Check if file already indexed with same hash
        let existing_hash: Option<String> = self.conn
            .query_row(
                "SELECT hash FROM files WHERE path = ?1",
                params![path.to_string_lossy().to_string()],
                |row| row.get(0),
            )
            .ok();

        if existing_hash.as_ref() == Some(&hash) {
            // File unchanged, skip
            return Err(anyhow::anyhow!("File unchanged"));
        }

        let relative_path = path
            .strip_prefix(&self.root_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let symbols = Self::extract_symbols(&content, language);
        let size = content.len() as u64;

        let indexed = IndexedFile {
            path: path.to_string_lossy().to_string(),
            relative_path,
            language: language.to_string(),
            size,
            hash,
            summary: None,
            symbols,
            indexed_at: Utc::now(),
        };

        let symbols_json = serde_json::to_string(&indexed.symbols)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO files (path, relative_path, language, size, hash, content, summary, symbols, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                indexed.path,
                indexed.relative_path,
                indexed.language,
                indexed.size,
                indexed.hash,
                content,
                indexed.summary,
                symbols_json,
                indexed.indexed_at.to_rfc3339(),
            ],
        )?;

        // Update FTS index
        self.conn.execute(
            "INSERT OR REPLACE INTO files_fts (path, content, symbols)
             VALUES (?1, ?2, ?3)",
            params![indexed.path, content, symbols_json],
        ).ok();

        Ok(indexed)
    }

    fn detect_language(path: &Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        let lang = match ext.to_lowercase().as_str() {
            "rs" => "rust",
            "py" => "python",
            "js" => "javascript",
            "ts" => "typescript",
            "tsx" => "typescript",
            "jsx" => "javascript",
            "go" => "go",
            "java" => "java",
            "kt" => "kotlin",
            "c" | "h" => "c",
            "cpp" | "cc" | "hpp" => "cpp",
            "cs" => "csharp",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "scala" => "scala",
            "sh" | "bash" => "shell",
            "sql" => "sql",
            "html" => "html",
            "css" => "css",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "md" => "markdown",
            _ => return None,
        };
        Some(lang.to_string())
    }

    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn extract_symbols(content: &str, language: &str) -> Vec<String> {
        let mut symbols = Vec::new();

        // Simple regex-free symbol extraction
        for line in content.lines() {
            let trimmed = line.trim();

            match language {
                "rust" => {
                    if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                        if let Some(name) = Self::extract_fn_name(trimmed, "fn ") {
                            symbols.push(format!("fn:{}", name));
                        }
                    } else if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
                        if let Some(name) = Self::extract_after(trimmed, "struct ") {
                            symbols.push(format!("struct:{}", name));
                        }
                    } else if trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ") {
                        if let Some(name) = Self::extract_after(trimmed, "enum ") {
                            symbols.push(format!("enum:{}", name));
                        }
                    } else if trimmed.starts_with("impl ") {
                        if let Some(name) = Self::extract_after(trimmed, "impl ") {
                            symbols.push(format!("impl:{}", name));
                        }
                    }
                }
                "python" => {
                    if trimmed.starts_with("def ") {
                        if let Some(name) = Self::extract_fn_name(trimmed, "def ") {
                            symbols.push(format!("def:{}", name));
                        }
                    } else if trimmed.starts_with("class ") {
                        if let Some(name) = Self::extract_after(trimmed, "class ") {
                            symbols.push(format!("class:{}", name));
                        }
                    }
                }
                "javascript" | "typescript" => {
                    if trimmed.starts_with("function ") {
                        if let Some(name) = Self::extract_fn_name(trimmed, "function ") {
                            symbols.push(format!("function:{}", name));
                        }
                    } else if trimmed.starts_with("class ") {
                        if let Some(name) = Self::extract_after(trimmed, "class ") {
                            symbols.push(format!("class:{}", name));
                        }
                    } else if trimmed.contains("const ") && trimmed.contains(" = ") {
                        if let Some(name) = Self::extract_const_name(trimmed) {
                            symbols.push(format!("const:{}", name));
                        }
                    }
                }
                "go" => {
                    if trimmed.starts_with("func ") {
                        if let Some(name) = Self::extract_fn_name(trimmed, "func ") {
                            symbols.push(format!("func:{}", name));
                        }
                    } else if trimmed.starts_with("type ") && trimmed.contains(" struct") {
                        if let Some(name) = Self::extract_after(trimmed, "type ") {
                            symbols.push(format!("struct:{}", name));
                        }
                    }
                }
                "java" | "kotlin" => {
                    if (trimmed.contains("class ") || trimmed.contains("interface "))
                        && !trimmed.starts_with("//")
                    {
                        if let Some(name) = Self::extract_java_class(trimmed) {
                            symbols.push(format!("class:{}", name));
                        }
                    }
                }
                _ => {}
            }
        }

        symbols
    }

    fn extract_fn_name(line: &str, prefix: &str) -> Option<String> {
        let after = line.split(prefix).nth(1)?;
        let name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    fn extract_after(line: &str, prefix: &str) -> Option<String> {
        let after = line.split(prefix).last()?;
        let name: String = after
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    fn extract_const_name(line: &str) -> Option<String> {
        let parts: Vec<&str> = line.split("const ").collect();
        if parts.len() < 2 {
            return None;
        }
        let after = parts[1];
        let name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    fn extract_java_class(line: &str) -> Option<String> {
        let keywords = ["class ", "interface "];
        for kw in keywords {
            if let Some(idx) = line.find(kw) {
                let after = &line[idx + kw.len()..];
                let name: String = after
                    .chars()
                    .skip_while(|c| c.is_whitespace())
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
        None
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<IndexedFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.path, f.relative_path, f.language, f.size, f.hash, f.summary, f.symbols, f.indexed_at
             FROM files f
             JOIN files_fts fts ON f.path = fts.path
             WHERE files_fts MATCH ?1
             LIMIT ?2",
        )?;

        let files = stmt
            .query_map(params![query, limit as i64], |row| {
                let symbols_json: String = row.get(6)?;
                let symbols: Vec<String> = serde_json::from_str(&symbols_json).unwrap_or_default();
                let indexed_str: String = row.get(7)?;
                let indexed_at = DateTime::parse_from_rfc3339(&indexed_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(IndexedFile {
                    path: row.get(0)?,
                    relative_path: row.get(1)?,
                    language: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    summary: row.get(5)?,
                    symbols,
                    indexed_at,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(files)
    }

    pub fn search_by_symbol(&self, symbol: &str, limit: usize) -> Result<Vec<IndexedFile>> {
        let pattern = format!("%{}%", symbol);
        let mut stmt = self.conn.prepare(
            "SELECT path, relative_path, language, size, hash, summary, symbols, indexed_at
             FROM files
             WHERE symbols LIKE ?1
             LIMIT ?2",
        )?;

        let files = stmt
            .query_map(params![pattern, limit as i64], |row| {
                let symbols_json: String = row.get(6)?;
                let symbols: Vec<String> = serde_json::from_str(&symbols_json).unwrap_or_default();
                let indexed_str: String = row.get(7)?;
                let indexed_at = DateTime::parse_from_rfc3339(&indexed_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(IndexedFile {
                    path: row.get(0)?,
                    relative_path: row.get(1)?,
                    language: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    summary: row.get(5)?,
                    symbols,
                    indexed_at,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(files)
    }

    pub fn get_file(&self, path: &str) -> Result<Option<IndexedFile>> {
        let result = self.conn.query_row(
            "SELECT path, relative_path, language, size, hash, summary, symbols, indexed_at
             FROM files WHERE path = ?1 OR relative_path = ?1",
            params![path],
            |row| {
                let symbols_json: String = row.get(6)?;
                let symbols: Vec<String> = serde_json::from_str(&symbols_json).unwrap_or_default();
                let indexed_str: String = row.get(7)?;
                let indexed_at = DateTime::parse_from_rfc3339(&indexed_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(IndexedFile {
                    path: row.get(0)?,
                    relative_path: row.get(1)?,
                    language: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    summary: row.get(5)?,
                    symbols,
                    indexed_at,
                })
            },
        );

        match result {
            Ok(file) => Ok(Some(file)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_file_content(&self, path: &str) -> Result<Option<String>> {
        let content: Option<String> = self.conn
            .query_row(
                "SELECT content FROM files WHERE path = ?1 OR relative_path = ?1",
                params![path],
                |row| row.get(0),
            )
            .ok();
        Ok(content)
    }

    pub fn get_stats(&self) -> Result<CodebaseStats> {
        let total_files: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM files",
            [],
            |row| row.get(0),
        )?;

        let total_lines: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(content) - LENGTH(REPLACE(content, char(10), '')) + 1), 0) FROM files",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT language, COUNT(*) as cnt FROM files GROUP BY language ORDER BY cnt DESC",
        )?;

        let languages: Vec<(String, usize)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let last_indexed: Option<String> = self.conn
            .query_row(
                "SELECT MAX(indexed_at) FROM files",
                [],
                |row| row.get(0),
            )
            .ok();

        let last_indexed = last_indexed.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });

        Ok(CodebaseStats {
            total_files: total_files as usize,
            total_lines: total_lines as usize,
            languages,
            last_indexed,
        })
    }

    pub fn list_files(&self, language: Option<&str>, limit: usize) -> Result<Vec<IndexedFile>> {
        let query = match language {
            Some(lang) => format!(
                "SELECT path, relative_path, language, size, hash, summary, symbols, indexed_at
                 FROM files WHERE language = '{}' ORDER BY relative_path LIMIT {}",
                lang, limit
            ),
            None => format!(
                "SELECT path, relative_path, language, size, hash, summary, symbols, indexed_at
                 FROM files ORDER BY relative_path LIMIT {}",
                limit
            ),
        };

        let mut stmt = self.conn.prepare(&query)?;

        let files = stmt
            .query_map([], |row| {
                let symbols_json: String = row.get(6)?;
                let symbols: Vec<String> = serde_json::from_str(&symbols_json).unwrap_or_default();
                let indexed_str: String = row.get(7)?;
                let indexed_at = DateTime::parse_from_rfc3339(&indexed_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(IndexedFile {
                    path: row.get(0)?,
                    relative_path: row.get(1)?,
                    language: row.get(2)?,
                    size: row.get(3)?,
                    hash: row.get(4)?,
                    summary: row.get(5)?,
                    symbols,
                    indexed_at,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(files)
    }
}
