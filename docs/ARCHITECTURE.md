# Sovereign Architecture

## Overview

Sovereign is a local-first AI code assistant built on the principle that your code should never leave your machine. It uses local LLM inference via Ollama and stores all data locally using SQLite and CRDT-based storage.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           CLI Interface                              │
│                         (src/main.rs)                                │
├─────────────────────────────────────────────────────────────────────┤
│                        Agent Orchestrator                            │
│                    (src/agents/orchestrator.rs)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│  │ Code Agent  │  │Search Agent │  │ Chat Agent  │                  │
│  │ (code.rs)   │  │ (search.rs) │  │ (chat.rs)   │                  │
│  └─────────────┘  └─────────────┘  └─────────────┘                  │
├─────────────────────────────────────────────────────────────────────┤
│                         Core Services                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│  │   Ollama    │  │  Codebase   │  │   Memory    │                  │
│  │   Client    │  │   Index     │  │   Store     │                  │
│  │ (llm.rs)    │  │(codebase.rs)│  │(memory.rs)  │                  │
│  │             │  │             │  │             │                  │
│  │ + Embedding │  │ + FTS5      │  │ + CRDT      │                  │
│  │   Client    │  │ + Embeddings│  │(crdt_mem.rs)│                  │
│  └─────────────┘  └─────────────┘  └─────────────┘                  │
├─────────────────────────────────────────────────────────────────────┤
│                        P2P Sync Service                              │
│                         (src/sync.rs)                                │
│                    TCP-based peer-to-peer sync                       │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Details

### CLI Interface (`src/main.rs`)

The main entry point providing:
- Interactive chat mode with readline support
- Subcommands: `chat`, `index`, `search`, `ask`, `generate`, `explain`, `stats`, `memory`
- Command-line argument parsing via `clap`

### Agent Orchestrator (`src/agents/orchestrator.rs`)

Coordinates between different agents and handles command routing:
- Routes `/` commands to appropriate handlers
- Manages codebase indexing
- Coordinates between agents for complex operations
- Manages CRDT memory and P2P sync

### Code Agent (`src/agents/code.rs`)

Handles code-related operations:
- `generate_code`: Create new code from descriptions
- `explain_code`: Explain what code does
- `review_code`: Review code for improvements
- `write_tests`: Generate unit tests
- `fix_bug`: Debug and fix issues
- `refactor_code`: Improve code structure

### Search Agent (`src/agents/search.rs`)

Provides intelligent code search:
- `semantic_search`: Vector embedding-based search
- `find_symbol`: Symbol definition lookup
- `answer_question`: RAG-style Q&A about codebase
- `summarize_file`: Generate file summaries
- `index_embeddings`: Build vector embeddings

### Chat Agent (`src/agents/chat.rs`)

Handles conversational interactions:
- Maintains conversation history
- Integrates memory context
- Detects and stores user preferences
- Streaming responses

### Ollama Client (`src/llm.rs`)

Interface to local Ollama server:
- `generate`: Single completion
- `generate_streaming`: Streaming completion
- `chat`: Multi-turn conversation
- Health checking

### Embedding Client (`src/embeddings.rs`)

Vector embedding generation:
- Uses `nomic-embed-text` model via Ollama
- Cosine similarity calculation
- Batch embedding support

### Codebase Index (`src/storage/codebase.rs`)

SQLite-based code indexing:
- Full-text search (FTS5)
- Language detection
- Symbol extraction
- Vector embedding storage
- File content caching

### Memory Store (`src/storage/memory.rs`)

SQLite-based persistent memory:
- Typed memories (Conversation, CodePattern, Decision, Preference, Fact)
- Importance scoring
- Project association
- Tag support

### CRDT Memory (`src/storage/crdt_memory.rs`)

Automerge-based conflict-free memory:
- Conflict-free replication
- Multi-device sync support
- Export/import capabilities
- Incremental sync

### P2P Sync (`src/sync.rs`)

TCP-based peer synchronization:
- Push/pull operations
- Bidirectional sync
- No central server required

## Data Flow

### Indexing Flow
```
User runs: sovereign index /path/to/project
    │
    ▼
CodebaseIndex.index_directory()
    │
    ├─► Walk directory (ignore patterns)
    │
    ├─► For each file:
    │       ├─► Detect language
    │       ├─► Extract symbols (functions, classes, etc.)
    │       ├─► Calculate content hash
    │       └─► Store in SQLite FTS5
    │
    └─► Update statistics
```

### Search Flow
```
User runs: /search "authentication middleware"
    │
    ▼
SearchAgent.semantic_search()
    │
    ├─► Generate query embedding
    │
    ├─► Find similar embeddings (cosine similarity)
    │
    ├─► FTS5 text search (fallback)
    │
    ├─► Symbol search
    │
    └─► Combine, deduplicate, rank by relevance
```

### Chat Flow
```
User types: "How does the auth system work?"
    │
    ▼
ChatAgent.chat()
    │
    ├─► Add to conversation history
    │
    ├─► Include memory context
    │
    ├─► Send to Ollama (streaming)
    │
    ├─► Store conversation summary in memory
    │
    └─► Detect and store preferences
```

### Sync Flow
```
Device A: /sync-push deviceB:7654
    │
    ▼
P2PSync.push_to_peer()
    │
    ├─► Connect via TCP
    │
    ├─► Send CRDT document
    │
    └─► Device B merges with Automerge
            │
            └─► Conflict-free merge
```

## Storage Schema

### SQLite: memory.db
```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    project TEXT,
    tags TEXT NOT NULL,  -- JSON array
    created_at TEXT NOT NULL,
    importance REAL NOT NULL DEFAULT 0.5
);
```

### SQLite: codebase.db
```sql
CREATE TABLE files (
    path TEXT PRIMARY KEY,
    relative_path TEXT NOT NULL,
    language TEXT NOT NULL,
    size INTEGER NOT NULL,
    hash TEXT NOT NULL,
    content TEXT,
    summary TEXT,
    symbols TEXT NOT NULL,  -- JSON array
    indexed_at TEXT NOT NULL
);

CREATE TABLE embeddings (
    path TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    created_at TEXT NOT NULL
);

-- FTS5 for full-text search
CREATE VIRTUAL TABLE files_fts USING fts5(
    relative_path, language, symbols, content,
    content='files', content_rowid='rowid'
);
```

### Automerge: memories.automerge
```
{
    "memories": [
        {
            "id": "uuid",
            "content": "...",
            "type": "fact|preference|...",
            "timestamp": "ISO8601",
            "project": "optional",
            "importance": 0.5,
            "tags": []
        }
    ],
    "metadata": {}
}
```

## Local-First Principles

1. **No Network Required**: Works completely offline
2. **Your Data**: All data stored in `~/Library/Application Support/sovereign/` (macOS) or `~/.local/share/sovereign/` (Linux)
3. **No Spinners**: Local LLM means instant responses
4. **Privacy**: Code never leaves your machine
5. **Conflict-Free Sync**: CRDT ensures merges never conflict
6. **Longevity**: No subscription, works forever

## Extension Points

### Adding New Agents
1. Create new file in `src/agents/`
2. Implement agent struct with LLM client
3. Add to `Orchestrator`
4. Wire up commands in `handle_command`

### Adding New Storage
1. Create module in `src/storage/`
2. Implement with SQLite or file-based storage
3. Integrate with `Orchestrator`

### Adding New Commands
1. Add match arm in `Orchestrator::handle_command`
2. Update `HELP_TEXT`
3. Document in README

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_crdt_memory_basic

# Run with output
cargo test -- --nocapture
```

## Performance Considerations

- **Embedding Generation**: ~100ms per file with nomic-embed-text
- **FTS5 Search**: <10ms for most queries
- **Vector Search**: O(n) with embeddings, consider HNSW for large codebases
- **Memory**: ~1MB per 1000 indexed files
- **LLM Inference**: Depends on model size (7B: ~50 tok/s, 14B: ~30 tok/s on M1)
