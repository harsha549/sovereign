# Sovereign

**Local-First AI Code Assistant** - Your code never leaves your machine.

```
  ____                            _
 / ___|  _____   _____ _ __ ___(_) __ _ _ __
 \___ \ / _ \ \ / / _ \ '__/ _ \ |/ _` | '_ \
  ___) | (_) \ V /  __/ | |  __/ | (_| | | | |
 |____/ \___/ \_/ \___|_|  \___|_|\__, |_| |_|
                                  |___/
```

## Features

- **100% Local** - Runs entirely on your machine using Ollama
- **Privacy First** - Your code never leaves your device
- **Persistent Memory** - Remembers context across sessions
- **Codebase Indexing** - Understands your entire project
- **Semantic Search** - Find code by meaning using vector embeddings
- **CRDT Sync** - Conflict-free sync across devices with Automerge
- **P2P Sync** - Sync directly with other devices, no server needed
- **Works Offline** - No internet required
- **VS Code Extension** - Integrated AI assistance in your editor
- **Multi-Language** - Supports Rust, Python, JavaScript, TypeScript, Go, Java, and more

## Installation

### Prerequisites

1. **Ollama** - For local LLM inference
   ```bash
   brew install ollama
   brew services start ollama
   ollama pull qwen2.5-coder:14b
   ollama pull nomic-embed-text  # For semantic search
   ```

2. **Rust** - For building Sovereign
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### Build

```bash
cd sovereign
cargo build --release
```

### Install (Optional)

```bash
cargo install --path .
```

## Usage

### Interactive Chat Mode

```bash
# Start chat
sovereign

# Start chat with a codebase indexed
sovereign chat --path /path/to/your/project
```

### Index a Codebase

```bash
sovereign index /path/to/your/project
```

### Ask Questions About Your Code

```bash
sovereign ask "Where is the authentication logic?" --path /path/to/project
```

### Generate Code

```bash
sovereign generate "Write a function to parse JSON config files"
```

### Explain Code

```bash
cat myfile.rs | sovereign explain
# or
sovereign explain src/main.rs
```

### View Statistics

```bash
sovereign stats
```

### View Memories

```bash
sovereign memory
```

## Chat Commands

Once in interactive mode, use these commands:

### Code Operations
| Command | Description |
|---------|-------------|
| `/search <query>` | Search codebase (semantic search if embeddings exist) |
| `/symbol <name>` | Find symbol definitions |
| `/ask <question>` | Ask about codebase |
| `/read <file>` | Read file content |
| `/summarize <file>` | Summarize a file |
| `/embed` | Build embeddings for semantic search |
| `/stats` | Show codebase statistics |

### Code Generation
| Command | Description |
|---------|-------------|
| `/generate <desc>` | Generate code |
| `/explain <code>` | Explain code |
| `/review <code>` | Review code |
| `/test <code>` | Generate tests |
| `/fix <desc> \`\`\`code\`\`\`` | Fix a bug |
| `/refactor <desc> \`\`\`code\`\`\`` | Refactor code |

### Sync (Local-First)
| Command | Description |
|---------|-------------|
| `/sync-export` | Export CRDT memories for sync |
| `/sync-import <file>` | Import and merge CRDT memories |
| `/sync-status` | Show CRDT and P2P sync status |
| `/sync-pull <host:port>` | Pull memories from a peer |
| `/sync-push <host:port>` | Push memories to a peer |
| `/sync-live <host:port>` | Bidirectional sync with a peer |

### Other
| Command | Description |
|---------|-------------|
| `/memory` | Show recent memories |
| `/clear` | Clear conversation |
| `/quit` | Exit |

Or just type naturally to chat!

## Semantic Search

Sovereign uses vector embeddings for semantic code search:

```bash
# Index a codebase
sovereign index /path/to/project

# Build embeddings (in chat mode)
/embed

# Search semantically
/search "authentication middleware"
```

## Multi-Device Sync

Sovereign supports CRDT-based sync for conflict-free merging across devices:

### Export/Import
```bash
# On device A - export
/sync-export

# Copy the file to device B, then import
/sync-import /path/to/sync_export.automerge
```

### Live P2P Sync
```bash
# On device A (listening)
# Note: Device A should have sovereign running

# On device B (connect to A)
/sync-live deviceA-hostname:7654
```

## VS Code Extension

The Sovereign VS Code extension provides AI assistance directly in your editor.

### Installation

```bash
cd vscode-extension
npm install
npm run compile
```

Then press F5 in VS Code to run the extension in development mode.

### Features

- **Chat Panel** - AI chat in the sidebar
- **Context Menu** - Right-click to explain, review, refactor, or fix code
- **Code Generation** - Generate code from descriptions
- **Test Generation** - Automatically generate tests

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Sovereign CLI                           │
├─────────────────────────────────────────────────────────────┤
│                    Agent Orchestrator                        │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│  │ Code Agent  │ │Search Agent │ │ Chat Agent  │            │
│  └─────────────┘ └─────────────┘ └─────────────┘            │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│  │  Ollama     │ │  Codebase   │ │   Memory    │            │
│  │  (LLM)      │ │   Index     │ │   Store     │            │
│  │             │ │ + Embeddings│ │  + CRDT     │            │
│  └─────────────┘ └─────────────┘ └─────────────┘            │
│                         │                │                   │
│                   ┌─────┴────────────────┴─────┐            │
│                   │      P2P Sync Service       │            │
│                   └─────────────────────────────┘            │
└─────────────────────────────────────────────────────────────┘
```

## Local-First Principles

Sovereign is built on [local-first software](https://www.inkandswitch.com/local-first/) principles:

1. **No Spinners** - Fast, instant responses from local LLM
2. **Your Data** - Everything stored locally
3. **Works Offline** - No internet dependency
4. **Privacy** - Code never leaves your machine
5. **Collaboration** - CRDT-based sync for conflict-free merging
6. **Longevity** - Works forever, no subscription needed
7. **Multi-Device** - P2P sync without central servers

## Configuration

Data is stored in:
- macOS: `~/Library/Application Support/sovereign/`
- Linux: `~/.local/share/sovereign/`

Files:
- `memory.db` - Persistent SQLite memory store
- `codebase.db` - Indexed codebase with embeddings
- `memories.automerge` - CRDT document for sync
- `history.txt` - Command history

## Models

Recommended models (via Ollama):

| Model | Size | Quality | Speed |
|-------|------|---------|-------|
| qwen2.5-coder:7b | 4.7GB | Good | Fast |
| qwen2.5-coder:14b | 9GB | Excellent | Medium |
| qwen2.5-coder:32b | 20GB | Best | Slow |
| deepseek-coder-v2:16b | 9GB | Excellent | Medium |

For embeddings:
| Model | Size | Description |
|-------|------|-------------|
| nomic-embed-text | 274MB | Fast, good quality embeddings |

Change model:
```bash
sovereign --model qwen2.5-coder:7b
```

## Roadmap

- [x] Basic CLI with chat
- [x] Codebase indexing with FTS
- [x] Persistent memory
- [x] Vector embeddings for semantic search
- [x] CRDT-based memory with Automerge
- [x] P2P sync
- [x] VS Code extension (basic)
- [ ] IntelliJ plugin
- [ ] Neovim plugin
- [ ] Background daemon mode
- [ ] File watching for auto-reindex
- [ ] Multi-modal support (diagrams, images)

## License

MIT

## Credits

Built with:
- [Ollama](https://ollama.ai) - Local LLM inference
- [Rust](https://www.rust-lang.org) - Systems programming language
- [Automerge](https://automerge.org) - CRDT for conflict-free sync
- Inspired by [Ink & Switch Local-First Software](https://www.inkandswitch.com/local-first/)
