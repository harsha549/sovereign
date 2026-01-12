import * as vscode from 'vscode';
import { OllamaClient, ChatMessage, StreamCallback } from './ollama';

export class ChatViewProvider implements vscode.WebviewViewProvider {
    private view?: vscode.WebviewView;
    private messages: ChatMessage[] = [];
    private isGenerating = false;

    constructor(
        private readonly extensionUri: vscode.Uri,
        private readonly ollamaClient: OllamaClient
    ) {}

    resolveWebviewView(
        webviewView: vscode.WebviewView,
        context: vscode.WebviewViewResolveContext,
        token: vscode.CancellationToken
    ): void {
        this.view = webviewView;

        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this.extensionUri]
        };

        webviewView.webview.html = this.getHtmlContent();

        webviewView.webview.onDidReceiveMessage(async (message) => {
            switch (message.type) {
                case 'sendMessage':
                    await this.handleUserMessage(message.content);
                    break;
                case 'clear':
                    this.messages = [];
                    this.updateMessages();
                    break;
                case 'stop':
                    this.ollamaClient.cancelRequest();
                    this.isGenerating = false;
                    this.view?.webview.postMessage({ type: 'generating', isGenerating: false });
                    break;
                case 'insertCode':
                    this.insertCodeInEditor(message.code);
                    break;
                case 'copyCode':
                    vscode.env.clipboard.writeText(message.code);
                    vscode.window.showInformationMessage('Code copied to clipboard');
                    break;
            }
        });
    }

    private async handleUserMessage(content: string) {
        if (this.isGenerating) {
            return;
        }

        // Add user message
        this.messages.push({ role: 'user', content });
        this.updateMessages();

        // Show generating state
        this.isGenerating = true;
        this.view?.webview.postMessage({ type: 'generating', isGenerating: true });

        try {
            // Get context from current file
            const editor = vscode.window.activeTextEditor;
            let contextInfo = '';
            if (editor) {
                const fileName = editor.document.fileName.split('/').pop() || 'Unknown';
                const language = editor.document.languageId;
                const selection = editor.selection;

                if (!selection.isEmpty) {
                    contextInfo = `Currently working in ${fileName} (${language}). Selected code:\n\`\`\`${language}\n${editor.document.getText(selection)}\n\`\`\``;
                } else {
                    contextInfo = `Currently working in ${fileName} (${language}).`;
                }
            }

            // Prepare messages with context
            const contextualMessages: ChatMessage[] = [...this.messages];
            if (contextInfo) {
                // Insert context before the last user message
                contextualMessages[contextualMessages.length - 1] = {
                    role: 'user',
                    content: `${contextInfo}\n\nUser question: ${content}`
                };
            }

            // Create a placeholder for the assistant message
            const assistantIndex = this.messages.length;
            this.messages.push({ role: 'assistant', content: '' });

            // Stream the response
            const callback: StreamCallback = {
                onChunk: (text: string) => {
                    this.messages[assistantIndex].content += text;
                    this.view?.webview.postMessage({
                        type: 'streamChunk',
                        index: assistantIndex,
                        content: this.messages[assistantIndex].content
                    });
                },
                onComplete: (fullResponse: string) => {
                    this.messages[assistantIndex].content = fullResponse;
                    this.isGenerating = false;
                    this.view?.webview.postMessage({ type: 'generating', isGenerating: false });
                    this.updateMessages();
                },
                onError: (error: Error) => {
                    this.messages[assistantIndex].content = `Error: ${error.message}`;
                    this.isGenerating = false;
                    this.view?.webview.postMessage({ type: 'generating', isGenerating: false });
                    this.updateMessages();
                }
            };

            await this.ollamaClient.chatWithHistoryStream(
                contextualMessages.slice(0, -1),
                content,
                callback
            );
        } catch (error) {
            this.messages.push({ role: 'assistant', content: `Error: ${error}` });
            this.isGenerating = false;
            this.view?.webview.postMessage({ type: 'generating', isGenerating: false });
            this.updateMessages();
        }
    }

    private insertCodeInEditor(code: string) {
        const editor = vscode.window.activeTextEditor;
        if (editor) {
            editor.edit(editBuilder => {
                if (editor.selection.isEmpty) {
                    editBuilder.insert(editor.selection.active, code);
                } else {
                    editBuilder.replace(editor.selection, code);
                }
            });
        } else {
            vscode.window.showErrorMessage('No active editor to insert code');
        }
    }

    private updateMessages() {
        this.view?.webview.postMessage({
            type: 'updateMessages',
            messages: this.messages
        });
    }

    private getHtmlContent(): string {
        return `
            <!DOCTYPE html>
            <html>
            <head>
                <style>
                    * {
                        box-sizing: border-box;
                        margin: 0;
                        padding: 0;
                    }
                    body {
                        font-family: var(--vscode-font-family);
                        font-size: var(--vscode-font-size);
                        color: var(--vscode-foreground);
                        background-color: var(--vscode-sideBar-background);
                        display: flex;
                        flex-direction: column;
                        height: 100vh;
                        padding: 8px;
                    }
                    .header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        padding: 8px 0;
                        border-bottom: 1px solid var(--vscode-panel-border);
                        margin-bottom: 8px;
                    }
                    .header-title {
                        display: flex;
                        align-items: center;
                        gap: 8px;
                    }
                    .header h3 {
                        font-size: 14px;
                        font-weight: 600;
                    }
                    .model-badge {
                        font-size: 10px;
                        padding: 2px 6px;
                        background-color: var(--vscode-badge-background);
                        color: var(--vscode-badge-foreground);
                        border-radius: 10px;
                    }
                    .header-actions {
                        display: flex;
                        gap: 8px;
                    }
                    .icon-btn {
                        background: none;
                        border: none;
                        color: var(--vscode-foreground);
                        cursor: pointer;
                        padding: 4px;
                        border-radius: 4px;
                        opacity: 0.7;
                    }
                    .icon-btn:hover {
                        opacity: 1;
                        background-color: var(--vscode-toolbar-hoverBackground);
                    }
                    .messages {
                        flex: 1;
                        overflow-y: auto;
                        padding: 8px 0;
                    }
                    .message {
                        margin-bottom: 16px;
                        animation: fadeIn 0.2s ease-out;
                    }
                    @keyframes fadeIn {
                        from { opacity: 0; transform: translateY(4px); }
                        to { opacity: 1; transform: translateY(0); }
                    }
                    .message-header {
                        display: flex;
                        align-items: center;
                        gap: 8px;
                        margin-bottom: 4px;
                        font-size: 12px;
                        font-weight: 600;
                    }
                    .message-header.user {
                        color: var(--vscode-textLink-foreground);
                    }
                    .message-header.assistant {
                        color: var(--vscode-symbolIcon-functionForeground);
                    }
                    .message-content {
                        padding: 8px 12px;
                        border-radius: 8px;
                        max-width: 100%;
                        word-wrap: break-word;
                        line-height: 1.5;
                    }
                    .message-content.user {
                        background-color: var(--vscode-inputOption-activeBackground);
                        border: 1px solid var(--vscode-inputOption-activeBorder);
                    }
                    .message-content.assistant {
                        background-color: var(--vscode-editor-background);
                        border: 1px solid var(--vscode-panel-border);
                    }
                    .code-block {
                        position: relative;
                        margin: 8px 0;
                    }
                    .code-block-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        background-color: var(--vscode-editorGroupHeader-tabsBackground);
                        padding: 4px 8px;
                        border-radius: 4px 4px 0 0;
                        font-size: 11px;
                    }
                    .code-block-lang {
                        color: var(--vscode-descriptionForeground);
                    }
                    .code-block-actions {
                        display: flex;
                        gap: 4px;
                    }
                    .code-action-btn {
                        background: none;
                        border: none;
                        color: var(--vscode-foreground);
                        cursor: pointer;
                        padding: 2px 6px;
                        border-radius: 3px;
                        font-size: 11px;
                        opacity: 0.7;
                    }
                    .code-action-btn:hover {
                        opacity: 1;
                        background-color: var(--vscode-toolbar-hoverBackground);
                    }
                    .code-block pre {
                        background-color: var(--vscode-textBlockQuote-background);
                        padding: 12px;
                        border-radius: 0 0 4px 4px;
                        overflow-x: auto;
                        margin: 0;
                    }
                    .code-block code {
                        font-family: var(--vscode-editor-font-family);
                        font-size: 12px;
                        line-height: 1.4;
                    }
                    .inline-code {
                        font-family: var(--vscode-editor-font-family);
                        font-size: 12px;
                        background-color: var(--vscode-textBlockQuote-background);
                        padding: 2px 4px;
                        border-radius: 3px;
                    }
                    .generating-indicator {
                        display: none;
                        padding: 8px 12px;
                        color: var(--vscode-descriptionForeground);
                        font-style: italic;
                        align-items: center;
                        gap: 8px;
                    }
                    .generating-indicator.visible {
                        display: flex;
                    }
                    .spinner {
                        width: 14px;
                        height: 14px;
                        border: 2px solid var(--vscode-descriptionForeground);
                        border-top-color: transparent;
                        border-radius: 50%;
                        animation: spin 1s linear infinite;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .stop-btn {
                        background-color: var(--vscode-errorForeground);
                        color: white;
                        border: none;
                        padding: 4px 8px;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 11px;
                    }
                    .stop-btn:hover {
                        opacity: 0.9;
                    }
                    .input-container {
                        display: flex;
                        gap: 8px;
                        padding-top: 8px;
                        border-top: 1px solid var(--vscode-panel-border);
                    }
                    .input-wrapper {
                        flex: 1;
                        position: relative;
                    }
                    .input-wrapper textarea {
                        width: 100%;
                        padding: 8px;
                        border: 1px solid var(--vscode-input-border);
                        background-color: var(--vscode-input-background);
                        color: var(--vscode-input-foreground);
                        border-radius: 4px;
                        outline: none;
                        resize: none;
                        min-height: 40px;
                        max-height: 120px;
                        font-family: inherit;
                        font-size: inherit;
                    }
                    .input-wrapper textarea:focus {
                        border-color: var(--vscode-focusBorder);
                    }
                    .send-btn {
                        padding: 8px 16px;
                        background-color: var(--vscode-button-background);
                        color: var(--vscode-button-foreground);
                        border: none;
                        border-radius: 4px;
                        cursor: pointer;
                        align-self: flex-end;
                    }
                    .send-btn:hover {
                        background-color: var(--vscode-button-hoverBackground);
                    }
                    .send-btn:disabled {
                        opacity: 0.5;
                        cursor: not-allowed;
                    }
                    .empty-state {
                        text-align: center;
                        padding: 40px 20px;
                        color: var(--vscode-descriptionForeground);
                    }
                    .empty-state h4 {
                        margin-bottom: 8px;
                        color: var(--vscode-foreground);
                    }
                    .empty-state p {
                        font-size: 12px;
                        line-height: 1.5;
                        margin-bottom: 16px;
                    }
                    .suggestion-chips {
                        display: flex;
                        flex-wrap: wrap;
                        gap: 8px;
                        justify-content: center;
                    }
                    .suggestion-chip {
                        background-color: var(--vscode-button-secondaryBackground);
                        color: var(--vscode-button-secondaryForeground);
                        border: none;
                        padding: 6px 12px;
                        border-radius: 16px;
                        font-size: 11px;
                        cursor: pointer;
                    }
                    .suggestion-chip:hover {
                        background-color: var(--vscode-button-secondaryHoverBackground);
                    }
                </style>
            </head>
            <body>
                <div class="header">
                    <div class="header-title">
                        <h3>Sovereign Chat</h3>
                        <span class="model-badge">Local AI</span>
                    </div>
                    <div class="header-actions">
                        <button class="icon-btn" onclick="clearChat()" title="Clear chat">🗑️</button>
                    </div>
                </div>
                <div class="messages" id="messages">
                    <div class="empty-state" id="emptyState">
                        <h4>Local-First AI Assistant</h4>
                        <p>Ask questions about your code, get explanations, or request help with programming tasks. All processing happens locally on your machine.</p>
                        <div class="suggestion-chips">
                            <button class="suggestion-chip" onclick="sendSuggestion('Explain this code')">Explain code</button>
                            <button class="suggestion-chip" onclick="sendSuggestion('Review this code')">Review code</button>
                            <button class="suggestion-chip" onclick="sendSuggestion('Suggest improvements')">Improve</button>
                            <button class="suggestion-chip" onclick="sendSuggestion('Write tests for this')">Write tests</button>
                        </div>
                    </div>
                </div>
                <div class="generating-indicator" id="generatingIndicator">
                    <div class="spinner"></div>
                    <span>Generating response...</span>
                    <button class="stop-btn" onclick="stopGeneration()">Stop</button>
                </div>
                <div class="input-container">
                    <div class="input-wrapper">
                        <textarea
                            id="messageInput"
                            placeholder="Ask about your code... (Shift+Enter for new line)"
                            onkeydown="handleKeyDown(event)"
                            oninput="autoResize(this)"
                        ></textarea>
                    </div>
                    <button class="send-btn" onclick="sendMessage()" id="sendBtn">Send</button>
                </div>

                <script>
                    const vscode = acquireVsCodeApi();
                    const messagesContainer = document.getElementById('messages');
                    const emptyState = document.getElementById('emptyState');
                    const generatingIndicator = document.getElementById('generatingIndicator');
                    const messageInput = document.getElementById('messageInput');
                    const sendBtn = document.getElementById('sendBtn');
                    let isGenerating = false;

                    function sendMessage() {
                        const content = messageInput.value.trim();
                        if (!content || isGenerating) return;

                        messageInput.value = '';
                        messageInput.style.height = '40px';
                        vscode.postMessage({ type: 'sendMessage', content });
                    }

                    function sendSuggestion(suggestion) {
                        vscode.postMessage({ type: 'sendMessage', content: suggestion });
                    }

                    function clearChat() {
                        vscode.postMessage({ type: 'clear' });
                    }

                    function stopGeneration() {
                        vscode.postMessage({ type: 'stop' });
                    }

                    function handleKeyDown(event) {
                        if (event.key === 'Enter' && !event.shiftKey) {
                            event.preventDefault();
                            sendMessage();
                        }
                    }

                    function autoResize(textarea) {
                        textarea.style.height = '40px';
                        textarea.style.height = Math.min(textarea.scrollHeight, 120) + 'px';
                    }

                    function escapeHtml(text) {
                        const div = document.createElement('div');
                        div.textContent = text;
                        return div.innerHTML;
                    }

                    function formatMessage(content) {
                        // Format code blocks with actions
                        let formatted = content.replace(
                            /\`\`\`(\\w*)\\n([\\s\\S]*?)\`\`\`/g,
                            function(match, lang, code) {
                                const escapedCode = escapeHtml(code.trim());
                                const langLabel = lang || 'code';
                                return '<div class="code-block">' +
                                    '<div class="code-block-header">' +
                                        '<span class="code-block-lang">' + langLabel + '</span>' +
                                        '<div class="code-block-actions">' +
                                            '<button class="code-action-btn" onclick="copyCode(this)">Copy</button>' +
                                            '<button class="code-action-btn" onclick="insertCode(this)">Insert</button>' +
                                        '</div>' +
                                    '</div>' +
                                    '<pre><code data-code="' + btoa(encodeURIComponent(code.trim())) + '">' + escapedCode + '</code></pre>' +
                                '</div>';
                            }
                        );

                        // Format inline code
                        formatted = formatted.replace(/\`([^\`]+)\`/g, '<code class="inline-code">$1</code>');

                        // Format bold
                        formatted = formatted.replace(/\\*\\*([^*]+)\\*\\*/g, '<strong>$1</strong>');

                        // Format italic
                        formatted = formatted.replace(/\\*([^*]+)\\*/g, '<em>$1</em>');

                        // Format newlines
                        formatted = formatted.replace(/\\n/g, '<br>');

                        return formatted;
                    }

                    function copyCode(btn) {
                        const codeEl = btn.closest('.code-block').querySelector('code');
                        const code = decodeURIComponent(atob(codeEl.dataset.code));
                        vscode.postMessage({ type: 'copyCode', code });
                    }

                    function insertCode(btn) {
                        const codeEl = btn.closest('.code-block').querySelector('code');
                        const code = decodeURIComponent(atob(codeEl.dataset.code));
                        vscode.postMessage({ type: 'insertCode', code });
                    }

                    function renderMessages(messages) {
                        if (messages.length === 0) {
                            emptyState.style.display = 'block';
                            messagesContainer.innerHTML = '';
                            messagesContainer.appendChild(emptyState);
                            return;
                        }

                        emptyState.style.display = 'none';
                        messagesContainer.innerHTML = messages.map(msg =>
                            '<div class="message">' +
                                '<div class="message-header ' + msg.role + '">' +
                                    (msg.role === 'user' ? '👤 You' : '🤖 Sovereign') +
                                '</div>' +
                                '<div class="message-content ' + msg.role + '">' +
                                    formatMessage(msg.content) +
                                '</div>' +
                            '</div>'
                        ).join('');

                        messagesContainer.scrollTop = messagesContainer.scrollHeight;
                    }

                    function updateLastMessage(content) {
                        const messages = messagesContainer.querySelectorAll('.message');
                        if (messages.length > 0) {
                            const lastMessage = messages[messages.length - 1];
                            const contentEl = lastMessage.querySelector('.message-content');
                            if (contentEl) {
                                contentEl.innerHTML = formatMessage(content);
                                messagesContainer.scrollTop = messagesContainer.scrollHeight;
                            }
                        }
                    }

                    window.addEventListener('message', event => {
                        const message = event.data;
                        switch (message.type) {
                            case 'updateMessages':
                                renderMessages(message.messages);
                                break;
                            case 'streamChunk':
                                updateLastMessage(message.content);
                                break;
                            case 'generating':
                                isGenerating = message.isGenerating;
                                generatingIndicator.classList.toggle('visible', message.isGenerating);
                                sendBtn.disabled = message.isGenerating;
                                break;
                        }
                    });
                </script>
            </body>
            </html>
        `;
    }
}
