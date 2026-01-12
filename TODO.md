# Sovereign - Next Development Session TODO

## Priority 1: Testing & Validation

- [ ] **Test DeepSeek Integration**
  - Set up DEEPSEEK_API_KEY
  - Test streaming responses
  - Compare quality vs Ollama
  - Test error handling (invalid key, rate limits)

- [ ] **Test WebSocket Server**
  - Connect via wscat: `wscat -c ws://localhost:7656`
  - Test concurrent connections
  - Verify streaming chunks work
  - Test reconnection handling

- [ ] **Test Git Integration**
  - `sovereign commit` on real changes
  - `sovereign pr-summary` on feature branch
  - Test with large diffs
  - Test with merge commits

- [ ] **Test Web UI**
  - Open in Chrome, Firefox, Safari
  - Test streaming display
  - Test code block copy
  - Test dark/light themes (if applicable)

## Priority 2: Publishing & Distribution

- [ ] **VS Code Marketplace**
  - Create publisher account
  - Add icon and screenshots
  - Write marketplace description
  - Package: `vsce package`
  - Publish: `vsce publish`

- [ ] **JetBrains Marketplace**
  - Create vendor account
  - Add plugin icon
  - Write plugin description
  - Build: `./gradlew buildPlugin`
  - Submit for review

- [ ] **Homebrew Formula**
  - Create formula for macOS installation
  - `brew install sovereign`

- [ ] **Cargo Publish**
  - Publish to crates.io
  - `cargo publish`

## Priority 3: New Features

- [ ] **Neovim Plugin** (Lua)
  - Create `nvim-sovereign/` directory
  - Telescope integration for search
  - Floating window for chat
  - Commands: `:SovereignChat`, `:SovereignExplain`
  - LSP-like code actions

- [ ] **Code Completion (Inline Suggestions)**
  - Research: LSP vs custom protocol
  - Ghost text rendering in editors
  - Debounced trigger on typing
  - Context-aware suggestions
  - Tab to accept

- [ ] **Web UI Enhancements**
  - File browser panel
  - Search results display
  - Settings panel
  - Model selector dropdown
  - Chat history persistence

## Priority 4: Infrastructure

- [ ] **CI/CD Pipeline**
  - GitHub Actions for Rust build
  - Automated testing
  - Release builds for Linux/macOS/Windows
  - Extension packaging

- [ ] **Docker Support**
  - Dockerfile for Sovereign
  - Docker Compose with Ollama
  - One-command setup

- [ ] **Documentation Site**
  - mdBook or Docusaurus
  - API documentation
  - Tutorial videos
  - Example use cases

## Priority 5: Advanced Features

- [ ] **Multi-Repo Support**
  - Index multiple codebases
  - Cross-repo search
  - Unified memory

- [ ] **Team Collaboration**
  - Shared memory sync
  - Team prompts/templates
  - Usage analytics

- [ ] **Context Providers**
  - Jira/Linear integration
  - GitHub Issues context
  - Slack thread context
  - Documentation (Notion, Confluence)

- [ ] **Custom Models**
  - Fine-tuning support
  - LoRA adapters
  - Custom system prompts per project

## Quick Wins (< 1 hour each)

- [ ] Add `--version` detailed output (git hash, build date)
- [ ] Add `/models` command to list available Ollama models
- [ ] Add `/switch <model>` command to change model mid-session
- [ ] Add syntax highlighting to CLI output
- [ ] Add `/export` command to export chat as markdown
- [ ] Add `--quiet` flag for scripting
- [ ] Add config file support (`~/.sovereign/config.toml`)

## Bug Fixes / Tech Debt

- [ ] Fix unused function warnings in `rag.rs`
- [ ] Add proper error messages for Ollama connection failures
- [ ] Handle large files gracefully (skip files > 1MB)
- [ ] Add timeout for LLM requests
- [ ] Improve commit message quality with better prompts

## Metrics to Track

- [ ] Response latency (p50, p95, p99)
- [ ] Token usage per session
- [ ] Most used commands
- [ ] Error rates
- [ ] User retention (if telemetry enabled)

---

## Suggested Session Order

### Session 1: Testing & Quick Wins
1. Run all manual tests
2. Fix any bugs found
3. Implement quick wins
4. Update documentation

### Session 2: Publishing
1. VS Code Marketplace
2. JetBrains Marketplace
3. Homebrew formula
4. crates.io publish

### Session 3: Neovim Plugin
1. Create Lua plugin structure
2. Implement core commands
3. Add Telescope integration
4. Test and document

### Session 4: Code Completion
1. Research LSP integration
2. Prototype inline suggestions
3. Implement for VS Code first
4. Port to other editors

---

*Last updated: January 2026*
