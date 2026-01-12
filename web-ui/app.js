/**
 * Sovereign Web UI - Main Application
 * Connects to Sovereign daemon via WebSocket for real-time chat
 */

class SovereignApp {
    constructor() {
        this.ws = null;
        this.tcpFallback = false;
        this.isConnected = false;
        this.isStreaming = false;
        this.currentStreamMessage = null;
        this.messageBuffer = '';

        // DOM Elements
        this.elements = {
            statusDot: document.getElementById('statusDot'),
            connectionStatus: document.getElementById('connectionStatus'),
            connectBtn: document.getElementById('connectBtn'),
            wsUrl: document.getElementById('wsUrl'),
            tcpPort: document.getElementById('tcpPort'),
            statsContainer: document.getElementById('statsContainer'),
            memoryContainer: document.getElementById('memoryContainer'),
            refreshMemory: document.getElementById('refreshMemory'),
            chatContainer: document.getElementById('chatContainer'),
            welcomeMessage: document.getElementById('welcomeMessage'),
            messageInput: document.getElementById('messageInput'),
            sendBtn: document.getElementById('sendBtn'),
        };

        this.init();
    }

    init() {
        // Configure marked.js
        marked.setOptions({
            highlight: (code, lang) => {
                if (lang && hljs.getLanguage(lang)) {
                    return hljs.highlight(code, { language: lang }).value;
                }
                return hljs.highlightAuto(code).value;
            },
            breaks: true,
            gfm: true
        });

        // Bind event listeners
        this.elements.connectBtn.addEventListener('click', () => this.toggleConnection());
        this.elements.sendBtn.addEventListener('click', () => this.sendMessage());
        this.elements.refreshMemory.addEventListener('click', () => this.fetchMemory());

        // Input handling
        this.elements.messageInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                this.sendMessage();
            }
        });

        // Auto-resize textarea
        this.elements.messageInput.addEventListener('input', () => {
            this.elements.messageInput.style.height = 'auto';
            this.elements.messageInput.style.height = Math.min(this.elements.messageInput.scrollHeight, 200) + 'px';
        });

        // Quick command buttons
        document.querySelectorAll('.command-btn').forEach(btn => {
            btn.addEventListener('click', () => {
                const command = btn.dataset.command;
                if (command === '/clear') {
                    this.clearChat();
                } else if (this.isConnected) {
                    this.elements.messageInput.value = command;
                    this.sendMessage();
                }
            });
        });

        // Auto-connect on load
        setTimeout(() => this.connect(), 500);
    }

    toggleConnection() {
        if (this.isConnected) {
            this.disconnect();
        } else {
            this.connect();
        }
    }

    connect() {
        const wsUrl = this.elements.wsUrl.value;
        this.updateStatus('connecting', 'Connecting...');

        try {
            this.ws = new WebSocket(wsUrl);

            this.ws.onopen = () => {
                this.isConnected = true;
                this.updateStatus('connected', 'Connected');
                this.enableInput(true);
                this.elements.connectBtn.textContent = 'Disconnect';
                this.elements.refreshMemory.disabled = false;

                // Fetch initial data
                this.fetchStats();
                this.fetchMemory();
            };

            this.ws.onclose = () => {
                this.handleDisconnect();
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                // Try TCP fallback via HTTP polling (for demo purposes)
                this.handleDisconnect('Connection failed. Make sure the daemon is running with WebSocket support.');
            };

            this.ws.onmessage = (event) => {
                this.handleMessage(event.data);
            };
        } catch (error) {
            console.error('Connection error:', error);
            this.handleDisconnect('Failed to connect: ' + error.message);
        }
    }

    disconnect() {
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
        this.handleDisconnect();
    }

    handleDisconnect(errorMessage = null) {
        this.isConnected = false;
        this.updateStatus('disconnected', 'Disconnected');
        this.enableInput(false);
        this.elements.connectBtn.textContent = 'Connect';
        this.elements.refreshMemory.disabled = true;

        if (errorMessage) {
            this.addSystemMessage(errorMessage);
        }

        // Reset stats and memory displays
        this.elements.statsContainer.innerHTML = '<p class="empty-state">Not connected</p>';
        this.elements.memoryContainer.innerHTML = '<p class="empty-state">Not connected</p>';
    }

    updateStatus(status, text) {
        this.elements.statusDot.className = 'status-dot ' + status;
        this.elements.connectionStatus.textContent = text;
    }

    enableInput(enabled) {
        this.elements.messageInput.disabled = !enabled;
        this.elements.sendBtn.disabled = !enabled;
    }

    sendMessage() {
        const message = this.elements.messageInput.value.trim();
        if (!message || !this.isConnected || this.isStreaming) return;

        // Add user message to chat
        this.addMessage('user', message);

        // Clear input
        this.elements.messageInput.value = '';
        this.elements.messageInput.style.height = 'auto';

        // Hide welcome message
        if (this.elements.welcomeMessage) {
            this.elements.welcomeMessage.style.display = 'none';
        }

        // Send to server
        const request = {
            command: message,
            args: null,
            stream: true
        };

        this.ws.send(JSON.stringify(request));
        this.isStreaming = true;

        // Create streaming message placeholder
        this.currentStreamMessage = this.addMessage('assistant', '', true);
        this.messageBuffer = '';
    }

    handleMessage(data) {
        try {
            const response = JSON.parse(data);

            // Handle different response types
            if (response.type === 'stream') {
                // Streaming chunk
                this.messageBuffer += response.content || '';
                this.updateStreamMessage(this.messageBuffer);
            } else if (response.type === 'end' || response.success !== undefined) {
                // End of stream or complete response
                this.isStreaming = false;

                if (response.result) {
                    this.messageBuffer = response.result;
                }

                if (this.currentStreamMessage) {
                    this.updateStreamMessage(this.messageBuffer, true);
                    this.currentStreamMessage = null;
                    this.messageBuffer = '';
                }

                if (response.error) {
                    this.addSystemMessage('Error: ' + response.error, true);
                }

                // Refresh stats after command
                this.fetchStats();
            } else if (response.stats) {
                // Stats response
                this.displayStats(response.stats);
            } else if (response.memories) {
                // Memory response
                this.displayMemory(response.memories);
            }
        } catch (error) {
            // Non-JSON response, treat as streaming text
            this.messageBuffer += data;
            this.updateStreamMessage(this.messageBuffer);
        }
    }

    addMessage(role, content, isStreaming = false) {
        const messageDiv = document.createElement('div');
        messageDiv.className = `message ${role}`;

        const avatarText = role === 'user' ? 'U' : 'S';
        const roleText = role === 'user' ? 'You' : 'Sovereign';

        messageDiv.innerHTML = `
            <div class="message-header">
                <div class="message-avatar">${avatarText}</div>
                <span class="message-role">${roleText}</span>
            </div>
            <div class="message-content">
                ${content ? this.renderMarkdown(content) : ''}
                ${isStreaming ? '<div class="streaming-indicator"><div class="streaming-dots"><span></span><span></span><span></span></div><span>Thinking...</span></div>' : ''}
            </div>
        `;

        this.elements.chatContainer.appendChild(messageDiv);
        this.scrollToBottom();

        return messageDiv;
    }

    updateStreamMessage(content, isComplete = false) {
        if (!this.currentStreamMessage) return;

        const contentDiv = this.currentStreamMessage.querySelector('.message-content');
        contentDiv.innerHTML = this.renderMarkdown(content);

        if (!isComplete) {
            contentDiv.innerHTML += '<div class="streaming-indicator"><div class="streaming-dots"><span></span><span></span><span></span></div></div>';
        }

        // Add copy buttons to code blocks
        this.addCopyButtons(contentDiv);

        this.scrollToBottom();
    }

    addSystemMessage(content, isError = false) {
        const messageDiv = document.createElement('div');
        messageDiv.className = `message system ${isError ? 'error-message' : ''}`;

        messageDiv.innerHTML = `
            <div class="message-content">
                ${content}
            </div>
        `;

        this.elements.chatContainer.appendChild(messageDiv);
        this.scrollToBottom();
    }

    renderMarkdown(text) {
        if (!text) return '';

        // Parse markdown
        let html = marked.parse(text);

        // Wrap code blocks with header for copy button
        html = html.replace(/<pre><code class="language-(\w+)">/g, (match, lang) => {
            return `<pre><div class="code-header"><span class="code-language">${lang}</span><button class="copy-btn" onclick="sovereignApp.copyCode(this)">Copy</button></div><code class="language-${lang}">`;
        });

        // Handle code blocks without language
        html = html.replace(/<pre><code>(?!<div class="code-header">)/g,
            '<pre><div class="code-header"><span class="code-language">text</span><button class="copy-btn" onclick="sovereignApp.copyCode(this)">Copy</button></div><code>');

        return html;
    }

    addCopyButtons(container) {
        container.querySelectorAll('pre').forEach(pre => {
            if (!pre.querySelector('.code-header')) {
                const code = pre.querySelector('code');
                const langClass = code?.className.match(/language-(\w+)/);
                const lang = langClass ? langClass[1] : 'text';

                const header = document.createElement('div');
                header.className = 'code-header';
                header.innerHTML = `<span class="code-language">${lang}</span><button class="copy-btn" onclick="sovereignApp.copyCode(this)">Copy</button>`;
                pre.insertBefore(header, pre.firstChild);
            }
        });
    }

    copyCode(button) {
        const pre = button.closest('pre');
        const code = pre.querySelector('code');
        const text = code.textContent;

        navigator.clipboard.writeText(text).then(() => {
            button.textContent = 'Copied!';
            button.classList.add('copied');
            setTimeout(() => {
                button.textContent = 'Copy';
                button.classList.remove('copied');
            }, 2000);
        });
    }

    clearChat() {
        this.elements.chatContainer.innerHTML = '';
        if (this.elements.welcomeMessage) {
            this.elements.welcomeMessage.style.display = 'block';
            this.elements.chatContainer.appendChild(this.elements.welcomeMessage);
        }
    }

    fetchStats() {
        if (!this.isConnected) return;

        const request = {
            command: '/stats',
            args: null
        };
        this.ws.send(JSON.stringify(request));
    }

    fetchMemory() {
        if (!this.isConnected) return;

        const request = {
            command: '/memory',
            args: null
        };
        this.ws.send(JSON.stringify(request));
    }

    displayStats(stats) {
        if (!stats) {
            this.elements.statsContainer.innerHTML = '<p class="empty-state">No codebase indexed</p>';
            return;
        }

        let html = `
            <div class="stat-item">
                <span class="stat-label">Files</span>
                <span class="stat-value">${stats.total_files || 0}</span>
            </div>
            <div class="stat-item">
                <span class="stat-label">Lines</span>
                <span class="stat-value">${stats.total_lines || 0}</span>
            </div>
        `;

        if (stats.languages && Object.keys(stats.languages).length > 0) {
            html += '<div class="language-list">';
            for (const [lang, count] of Object.entries(stats.languages)) {
                html += `
                    <div class="language-item">
                        <span class="language-name">${lang}</span>
                        <span class="language-count">${count}</span>
                    </div>
                `;
            }
            html += '</div>';
        }

        this.elements.statsContainer.innerHTML = html;
    }

    displayMemory(memories) {
        if (!memories || memories.length === 0) {
            this.elements.memoryContainer.innerHTML = '<p class="empty-state">No memories stored</p>';
            return;
        }

        let html = '';
        for (const memory of memories.slice(0, 5)) {
            const content = memory.content.length > 60
                ? memory.content.substring(0, 60) + '...'
                : memory.content;
            html += `
                <div class="memory-item">
                    <div class="memory-type">${memory.memory_type || 'note'}</div>
                    <div class="memory-content">${this.escapeHtml(content)}</div>
                </div>
            `;
        }

        this.elements.memoryContainer.innerHTML = html;
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    scrollToBottom() {
        this.elements.chatContainer.scrollTop = this.elements.chatContainer.scrollHeight;
    }
}

// Initialize the app
let sovereignApp;
document.addEventListener('DOMContentLoaded', () => {
    sovereignApp = new SovereignApp();
});
