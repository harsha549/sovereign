use anyhow::Result;
use automerge::{AutoCommit, ObjType, Prop, ReadDoc, ROOT};
use automerge::transaction::Transactable;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use uuid::Uuid;

/// CRDT-based memory store using Automerge for conflict-free sync
pub struct CrdtMemoryStore {
    doc: AutoCommit,
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CrdtMemory {
    pub id: String,
    pub content: String,
    pub memory_type: CrdtMemoryType,
    pub timestamp: DateTime<Utc>,
    pub project: Option<String>,
    pub tags: Vec<String>,
    pub importance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrdtMemoryType {
    Conversation,
    CodePattern,
    Decision,
    Preference,
    Fact,
}

impl CrdtMemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::CodePattern => "code_pattern",
            Self::Decision => "decision",
            Self::Preference => "preference",
            Self::Fact => "fact",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "conversation" => Self::Conversation,
            "code_pattern" => Self::CodePattern,
            "decision" => Self::Decision,
            "preference" => Self::Preference,
            "fact" => Self::Fact,
            _ => Self::Fact,
        }
    }
}

impl CrdtMemoryStore {
    pub fn new(data_dir: &PathBuf) -> Result<Self> {
        let path = data_dir.join("memories.automerge");

        let doc = if path.exists() {
            // Load existing document
            let bytes = std::fs::read(&path)?;
            AutoCommit::load(&bytes)?
        } else {
            // Create new document with memories list
            let mut doc = AutoCommit::new();
            doc.put_object(ROOT, "memories", ObjType::List)?;
            doc.put_object(ROOT, "metadata", ObjType::Map)?;
            doc
        };

        Ok(Self { doc, path })
    }

    /// Save the document to disk
    pub fn save(&mut self) -> Result<()> {
        let bytes = self.doc.save();
        std::fs::write(&self.path, bytes)?;
        Ok(())
    }

    /// Add a new memory
    pub fn add(&mut self, content: &str, memory_type: CrdtMemoryType) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().to_rfc3339();

        // Get the memories list
        let memories = self.doc.get(ROOT, "memories")?
            .ok_or_else(|| anyhow::anyhow!("Memories list not found"))?;

        // Insert a new memory object at the end
        let len = self.doc.length(&memories.1);
        let mem_obj = self.doc.insert_object(&memories.1, len, ObjType::Map)?;

        self.doc.put(&mem_obj, "id", id.clone())?;
        self.doc.put(&mem_obj, "content", content)?;
        self.doc.put(&mem_obj, "type", memory_type.as_str())?;
        self.doc.put(&mem_obj, "timestamp", timestamp)?;
        self.doc.put(&mem_obj, "importance", 0.5)?;
        self.doc.put_object(&mem_obj, "tags", ObjType::List)?;

        self.save()?;
        Ok(id)
    }

    /// Add a memory with project context
    pub fn add_with_project(
        &mut self,
        content: &str,
        memory_type: CrdtMemoryType,
        project: &str,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let timestamp = Utc::now().to_rfc3339();

        let memories = self.doc.get(ROOT, "memories")?
            .ok_or_else(|| anyhow::anyhow!("Memories list not found"))?;

        let len = self.doc.length(&memories.1);
        let mem_obj = self.doc.insert_object(&memories.1, len, ObjType::Map)?;

        self.doc.put(&mem_obj, "id", id.clone())?;
        self.doc.put(&mem_obj, "content", content)?;
        self.doc.put(&mem_obj, "type", memory_type.as_str())?;
        self.doc.put(&mem_obj, "timestamp", timestamp)?;
        self.doc.put(&mem_obj, "project", project)?;
        self.doc.put(&mem_obj, "importance", 0.5)?;
        self.doc.put_object(&mem_obj, "tags", ObjType::List)?;

        self.save()?;
        Ok(id)
    }

    /// Get recent memories
    pub fn get_recent(&self, limit: usize) -> Result<Vec<CrdtMemory>> {
        let mut memories = self.get_all()?;
        memories.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        memories.truncate(limit);
        Ok(memories)
    }

    /// Get all memories
    pub fn get_all(&self) -> Result<Vec<CrdtMemory>> {
        let memories_list = self.doc.get(ROOT, "memories")?
            .ok_or_else(|| anyhow::anyhow!("Memories list not found"))?;

        let len = self.doc.length(&memories_list.1);
        let mut result = Vec::with_capacity(len);

        for i in 0..len {
            if let Some((_, mem_obj)) = self.doc.get(&memories_list.1, Prop::Seq(i))? {
                let memory = self.read_memory(&mem_obj)?;
                result.push(memory);
            }
        }

        Ok(result)
    }

    /// Get memories by type
    pub fn get_by_type(&self, memory_type: CrdtMemoryType, limit: usize) -> Result<Vec<CrdtMemory>> {
        let all = self.get_all()?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|m| m.memory_type == memory_type)
            .take(limit)
            .collect();
        Ok(filtered)
    }

    /// Get memories by project
    pub fn get_by_project(&self, project: &str, limit: usize) -> Result<Vec<CrdtMemory>> {
        let all = self.get_all()?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|m| m.project.as_deref() == Some(project))
            .take(limit)
            .collect();
        Ok(filtered)
    }

    /// Update memory importance
    pub fn update_importance(&mut self, id: &str, importance: f32) -> Result<()> {
        let memories_list = self.doc.get(ROOT, "memories")?
            .ok_or_else(|| anyhow::anyhow!("Memories list not found"))?;

        let len = self.doc.length(&memories_list.1);

        for i in 0..len {
            if let Some((_, mem_obj)) = self.doc.get(&memories_list.1, Prop::Seq(i))? {
                if let Some((automerge::Value::Scalar(s), _)) = self.doc.get(&mem_obj, "id")? {
                    if s.to_str() == Some(id) {
                        self.doc.put(&mem_obj, "importance", importance as f64)?;
                        self.save()?;
                        return Ok(());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Memory not found: {}", id))
    }

    /// Add tag to memory
    pub fn add_tag(&mut self, id: &str, tag: &str) -> Result<()> {
        let memories_list = self.doc.get(ROOT, "memories")?
            .ok_or_else(|| anyhow::anyhow!("Memories list not found"))?;

        let len = self.doc.length(&memories_list.1);

        for i in 0..len {
            if let Some((_, mem_obj)) = self.doc.get(&memories_list.1, Prop::Seq(i))? {
                if let Some((automerge::Value::Scalar(s), _)) = self.doc.get(&mem_obj, "id")? {
                    if s.to_str() == Some(id) {
                        if let Some((_, tags_obj)) = self.doc.get(&mem_obj, "tags")? {
                            let tags_len = self.doc.length(&tags_obj);
                            self.doc.insert(&tags_obj, tags_len, tag)?;
                            self.save()?;
                            return Ok(());
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Memory not found: {}", id))
    }

    /// Delete a memory
    pub fn delete(&mut self, id: &str) -> Result<()> {
        let memories_list = self.doc.get(ROOT, "memories")?
            .ok_or_else(|| anyhow::anyhow!("Memories list not found"))?;

        let len = self.doc.length(&memories_list.1);

        for i in 0..len {
            if let Some((_, mem_obj)) = self.doc.get(&memories_list.1, Prop::Seq(i))? {
                if let Some((automerge::Value::Scalar(s), _)) = self.doc.get(&mem_obj, "id")? {
                    if s.to_str() == Some(id) {
                        self.doc.delete(&memories_list.1, Prop::Seq(i))?;
                        self.save()?;
                        return Ok(());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Memory not found: {}", id))
    }

    /// Merge with another document (for sync)
    pub fn merge(&mut self, other_bytes: &[u8]) -> Result<()> {
        let mut other = AutoCommit::load(other_bytes)?;
        self.doc.merge(&mut other)?;
        self.save()?;
        Ok(())
    }

    /// Export document for sync
    pub fn export(&mut self) -> Vec<u8> {
        self.doc.save()
    }

    /// Get sync state (for incremental sync)
    pub fn get_heads(&mut self) -> Vec<automerge::ChangeHash> {
        self.doc.get_heads()
    }

    /// Generate changes since given heads
    pub fn generate_sync_message(&mut self, their_heads: &[automerge::ChangeHash]) -> Option<Vec<u8>> {
        let changes = self.doc.get_changes(their_heads);
        if changes.is_empty() {
            None
        } else {
            Some(self.doc.save_after(their_heads))
        }
    }

    /// Apply incremental sync changes
    pub fn apply_sync_changes(&mut self, changes: &[u8]) -> Result<()> {
        self.doc.load_incremental(changes)?;
        self.save()?;
        Ok(())
    }

    fn read_memory(&self, obj: &automerge::ObjId) -> Result<CrdtMemory> {
        let id = self.get_string(obj, "id")?.unwrap_or_default();
        let content = self.get_string(obj, "content")?.unwrap_or_default();
        let type_str = self.get_string(obj, "type")?.unwrap_or_default();
        let timestamp_str = self.get_string(obj, "timestamp")?.unwrap_or_default();
        let project = self.get_string(obj, "project")?;
        let importance = self.get_f64(obj, "importance")?.unwrap_or(0.5) as f32;

        let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let tags = self.get_string_list(obj, "tags")?;

        Ok(CrdtMemory {
            id,
            content,
            memory_type: CrdtMemoryType::from_str(&type_str),
            timestamp,
            project,
            tags,
            importance,
        })
    }

    fn get_string(&self, obj: &automerge::ObjId, key: &str) -> Result<Option<String>> {
        if let Some((automerge::Value::Scalar(s), _)) = self.doc.get(obj, key)? {
            Ok(s.to_str().map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    }

    fn get_f64(&self, obj: &automerge::ObjId, key: &str) -> Result<Option<f64>> {
        if let Some((automerge::Value::Scalar(s), _)) = self.doc.get(obj, key)? {
            Ok(s.to_f64())
        } else {
            Ok(None)
        }
    }

    fn get_string_list(&self, obj: &automerge::ObjId, key: &str) -> Result<Vec<String>> {
        let mut result = Vec::new();

        if let Some((_, list_obj)) = self.doc.get(obj, key)? {
            let len = self.doc.length(&list_obj);
            for i in 0..len {
                if let Some((automerge::Value::Scalar(s), _)) = self.doc.get(&list_obj, Prop::Seq(i))? {
                    if let Some(str_val) = s.to_str() {
                        result.push(str_val.to_string());
                    }
                }
            }
        }

        Ok(result)
    }

    /// Count total memories
    pub fn count(&self) -> Result<usize> {
        let memories_list = self.doc.get(ROOT, "memories")?
            .ok_or_else(|| anyhow::anyhow!("Memories list not found"))?;
        Ok(self.doc.length(&memories_list.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_crdt_memory_basic() {
        let dir = tempdir().unwrap();
        let mut store = CrdtMemoryStore::new(&dir.path().to_path_buf()).unwrap();

        // Add memory
        let id = store.add("Test memory", CrdtMemoryType::Fact).unwrap();
        assert!(!id.is_empty());

        // Get recent
        let memories = store.get_recent(10).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].content, "Test memory");
    }

    #[test]
    fn test_crdt_merge() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();

        // Create store and add initial memory
        let mut store1 = CrdtMemoryStore::new(&dir1.path().to_path_buf()).unwrap();
        store1.add("Initial shared memory", CrdtMemoryType::Fact).unwrap();

        // Export and create second store from same state (simulating device sync)
        let initial_bytes = store1.export();
        std::fs::write(dir2.path().join("memories.automerge"), &initial_bytes).unwrap();
        let mut store2 = CrdtMemoryStore::new(&dir2.path().to_path_buf()).unwrap();

        // Now add different memories to each (concurrent edits)
        store1.add("Memory from device 1", CrdtMemoryType::Fact).unwrap();
        store2.add("Memory from device 2", CrdtMemoryType::Preference).unwrap();

        // Merge
        let bytes1 = store1.export();
        let bytes2 = store2.export();

        store1.merge(&bytes2).unwrap();
        store2.merge(&bytes1).unwrap();

        // Both should have all 3 memories
        let memories1 = store1.get_all().unwrap();
        let memories2 = store2.get_all().unwrap();

        assert_eq!(memories1.len(), 3);
        assert_eq!(memories2.len(), 3);
    }
}
