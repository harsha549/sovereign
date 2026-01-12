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
- **Hybrid RAG** - Intelligent retrieval combining semantic and keyword search
- **CRDT Sync** - Conflict-free sync across devices with Automerge
- **P2P Sync** - Sync directly with other devices, no server needed
- **Background Daemon** - Run as a service with auto-reindexing
- **File Watching** - Automatically reindex on file changes
- **Multi-Modal** - Analyze images, diagrams, and code screenshots
- **Works Offline** - No internet required
- **VS Code Extension** - Full-featured AI assistance with streaming
- **IntelliJ Plugin** - Native Kotlin plugin for JetBrains IDEs
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

### Background Daemon

```bash
# Start daemon with Unix socket (macOS/Linux)
sovereign daemon

# Start daemon with TCP
sovereign daemon --tcp --port 7655

# Start daemon with file watching
sovereign daemon --watch /path/to/project

# Watch directories for changes (standalone)
sovereign watch /path/to/project /another/project
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

- **Chat Panel** - AI chat in the sidebar with streaming responses
- **Context Menu** - Right-click to explain, review, refactor, or fix code
- **Code Generation** - Generate code from descriptions
- **Test Generation** - Automatically generate tests
- **Streaming Responses** - Real-time token streaming
- **Code Actions** - Copy and insert generated code
- **Request Cancellation** - Stop long-running requests

## IntelliJ Plugin

Native Kotlin plugin for JetBrains IDEs (IntelliJ IDEA, WebStorm, PyCharm, etc.).

### Installation

```bash
cd intellij-plugin
./gradlew buildPlugin
```

The plugin will be in `build/distributions/`. Install via Settings → Plugins → Install from disk.

### Features

- **Tool Window** - Sovereign panel in the IDE
- **Context Actions** - Right-click menu for code operations
- **Streaming Chat** - Real-time AI responses
- **Code Actions**:
  - Explain Code
  - Review Code
  - Refactor Code
  - Fix Bug
  - Generate Tests
  - Generate Code

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    IDE Extensions                                │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐        │
│  │ VS Code Ext   │  │ IntelliJ Plug │  │   Future...   │        │
│  └───────┬───────┘  └───────┬───────┘  └───────────────┘        │
│          └──────────────────┼───────────────────────────────────┤
│                             │                                    │
│  ┌──────────────────────────▼──────────────────────────────────┐│
│  │                  Daemon / CLI Interface                      ││
│  │           (Unix Socket / TCP / Direct CLI)                   ││
│  └──────────────────────────┬──────────────────────────────────┘│
├─────────────────────────────┼───────────────────────────────────┤
│                             │                                    │
│  ┌──────────────────────────▼──────────────────────────────────┐│
│  │                  Agent Orchestrator                          ││
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            ││
│  │  │ Code Agent  │ │Search Agent │ │ Chat Agent  │            ││
│  │  └─────────────┘ └─────────────┘ └─────────────┘            ││
│  └──────────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌────────────┐ │
│  │   Ollama    │ │  Codebase   │ │   Memory    │ │    RAG     │ │
│  │ (LLM+Vision)│ │   Index     │ │   Store     │ │  Retrieval │ │
│  │             │ │ + Embeddings│ │  + CRDT     │ │  (Hybrid)  │ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────┐  ┌─────────────────────────┐       │
│  │     P2P Sync Service    │  │     File Watcher        │       │
│  │      (TCP Direct)       │  │    (Auto-Reindex)       │       │
│  └─────────────────────────┘  └─────────────────────────┘       │
└─────────────────────────────────────────────────────────────────┘
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

### Code Models
| Model | Size | Quality | Speed |
|-------|------|---------|-------|
| qwen2.5-coder:7b | 4.7GB | Good | Fast |
| qwen2.5-coder:14b | 9GB | Excellent | Medium |
| qwen2.5-coder:32b | 20GB | Best | Slow |
| deepseek-coder-v2:16b | 9GB | Excellent | Medium |

### Vision Models (Multi-Modal)
| Model | Size | Description |
|-------|------|-------------|
| llava:7b | 4.7GB | Good general vision |
| llava:13b | 8GB | Better quality |
| bakllava | 4.7GB | Improved architecture |
| moondream | 1.8GB | Lightweight, fast |

### Embeddings
| Model | Size | Description |
|-------|------|-------------|
| nomic-embed-text | 274MB | Fast, good quality embeddings |

Change model:
```bash
sovereign --model qwen2.5-coder:7b
```

Pull vision model for multi-modal support:
```bash
ollama pull llava:7b
```

## Roadmap

### Completed
- [x] Basic CLI with chat
- [x] Codebase indexing with FTS
- [x] Persistent memory
- [x] Vector embeddings for semantic search
- [x] CRDT-based memory with Automerge
- [x] P2P sync
- [x] VS Code extension with streaming
- [x] IntelliJ plugin (Kotlin)
- [x] Background daemon mode (Unix socket + TCP)
- [x] File watching for auto-reindex
- [x] Multi-modal support (vision models)
- [x] Hybrid RAG retrieval

### In Progress
- [ ] Neovim plugin
- [ ] WebSocket support for real-time streaming
- [ ] Plugin marketplace publishing (VS Code, JetBrains)

### Planned
- [ ] Emacs plugin
- [ ] Web UI dashboard
- [ ] Project-specific memory contexts
- [ ] Git integration (commit message generation, PR summaries)
- [ ] Code completion (inline suggestions)
- [ ] Multi-repo support
- [ ] Team collaboration features

## Author

**Harsha549** - [harsha549@linux.com](mailto:harsha549@linux.com)

## License

MIT

## Credits

Built with:
- [Ollama](https://ollama.ai) - Local LLM inference
- [Rust](https://www.rust-lang.org) - Systems programming language
- [Automerge](https://automerge.org) - CRDT for conflict-free sync
- Inspired by [Ink & Switch Local-First Software](https://www.inkandswitch.com/local-first/)
