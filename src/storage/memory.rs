use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub project: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub importance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryType {
    Conversation,
    CodePattern,
    Decision,
    Preference,
    Fact,
}

impl MemoryType {
    pub fn as_str(&self) -> &str {
        match self {
            MemoryType::Conversation => "conversation",
            MemoryType::CodePattern => "code_pattern",
            MemoryType::Decision => "decision",
            MemoryType::Preference => "preference",
            MemoryType::Fact => "fact",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "conversation" => MemoryType::Conversation,
            "code_pattern" => MemoryType::CodePattern,
            "decision" => MemoryType::Decision,
            "preference" => MemoryType::Preference,
            "fact" => MemoryType::Fact,
            _ => MemoryType::Fact,
        }
    }
}

pub struct MemoryStore {
    conn: Connection,
}

impl MemoryStore {
    pub fn new(data_dir: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join("memory.db");
        let conn = Connection::open(&db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                memory_type TEXT NOT NULL,
                project TEXT,
                tags TEXT NOT NULL,
                created_at TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 0.5
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memory_type ON memories(memory_type)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_project ON memories(project)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn store(&self, memory: &Memory) -> Result<()> {
        let tags_json = serde_json::to_string(&memory.tags)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO memories (id, content, memory_type, project, tags, created_at, importance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                memory.id,
                memory.content,
                memory.memory_type.as_str(),
                memory.project,
                tags_json,
                memory.created_at.to_rfc3339(),
                memory.importance,
            ],
        )?;

        Ok(())
    }

    pub fn remember(
        &self,
        content: &str,
        memory_type: MemoryType,
        project: Option<&str>,
        tags: Vec<String>,
        importance: f32,
    ) -> Result<Memory> {
        let memory = Memory {
            id: Uuid::new_v4().to_string(),
            content: content.to_string(),
            memory_type,
            project: project.map(|s| s.to_string()),
            tags,
            created_at: Utc::now(),
            importance,
        };

        self.store(&memory)?;
        Ok(memory)
    }

    #[allow(dead_code)]
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, memory_type, project, tags, created_at, importance
             FROM memories
             WHERE content LIKE ?1
             ORDER BY importance DESC, created_at DESC
             LIMIT ?2",
        )?;

        let pattern = format!("%{}%", query);
        let memories = stmt
            .query_map(params![pattern, limit as i64], |row| {
                let tags_json: String = row.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let created_str: String = row.get(5)?;
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Memory {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    memory_type: MemoryType::from_str(&row.get::<_, String>(2)?),
                    project: row.get(3)?,
                    tags,
                    created_at,
                    importance: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(memories)
    }

    #[allow(dead_code)]
    pub fn get_by_project(&self, project: &str, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, memory_type, project, tags, created_at, importance
             FROM memories
             WHERE project = ?1
             ORDER BY importance DESC, created_at DESC
             LIMIT ?2",
        )?;

        let memories = stmt
            .query_map(params![project, limit as i64], |row| {
                let tags_json: String = row.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let created_str: String = row.get(5)?;
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Memory {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    memory_type: MemoryType::from_str(&row.get::<_, String>(2)?),
                    project: row.get(3)?,
                    tags,
                    created_at,
                    importance: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(memories)
    }

    pub fn get_recent(&self, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, memory_type, project, tags, created_at, importance
             FROM memories
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let memories = stmt
            .query_map(params![limit as i64], |row| {
                let tags_json: String = row.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let created_str: String = row.get(5)?;
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Memory {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    memory_type: MemoryType::from_str(&row.get::<_, String>(2)?),
                    project: row.get(3)?,
                    tags,
                    created_at,
                    importance: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(memories)
    }

    pub fn get_by_type(&self, memory_type: MemoryType, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, memory_type, project, tags, created_at, importance
             FROM memories
             WHERE memory_type = ?1
             ORDER BY importance DESC, created_at DESC
             LIMIT ?2",
        )?;

        let memories = stmt
            .query_map(params![memory_type.as_str(), limit as i64], |row| {
                let tags_json: String = row.get(4)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let created_str: String = row.get(5)?;
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Memory {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    memory_type: MemoryType::from_str(&row.get::<_, String>(2)?),
                    project: row.get(3)?,
                    tags,
                    created_at,
                    importance: row.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(memories)
    }

    #[allow(dead_code)]
    pub fn count(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memories",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}
