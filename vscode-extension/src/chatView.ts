import * as vscode from 'vscode';
import { OllamaClient } from './ollama';

export class ChatViewProvider implements vscode.WebviewViewProvider {
    private view?: vscode.WebviewView;
    private messages: Array<{ role: 'user' | 'assistant'; content: string }> = [];

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
            if (message.type === 'sendMessage') {
                await this.handleUserMessage(message.content);
            } else if (message.type === 'clear') {
                this.messages = [];
                this.updateMessages();
            }
        });
    }

    private async handleUserMessage(content: string) {
        // Add user message
        this.messages.push({ role: 'user', content });
        this.updateMessages();

        // Show typing indicator
        this.view?.webview.postMessage({ type: 'typing', isTyping: true });

        try {
            // Get context from current file
            const editor = vscode.window.activeTextEditor;
            let context = '';
            if (editor) {
                const selection = editor.selection;
                if (!selection.isEmpty) {
                    context = `Selected code (${editor.document.languageId}):\n\`\`\`\n${editor.document.getText(selection)}\n\`\`\``;
                }
            }

            // Generate response
            const response = await this.ollamaClient.chat(content, context || undefined);

            // Add assistant message
            this.messages.push({ role: 'assistant', content: response });
            this.updateMessages();
        } catch (error) {
            this.messages.push({ role: 'assistant', content: `Error: ${error}` });
            this.updateMessages();
        }

        // Hide typing indicator
        this.view?.webview.postMessage({ type: 'typing', isTyping: false });
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
                    .header h3 {
                        font-size: 14px;
                        font-weight: 600;
                    }
                    .clear-btn {
                        background: none;
                        border: none;
                        color: var(--vscode-textLink-foreground);
                        cursor: pointer;
                        font-size: 12px;
                    }
                    .clear-btn:hover {
                        color: var(--vscode-textLink-activeForeground);
                    }
                    .messages {
                        flex: 1;
                        overflow-y: auto;
                        padding: 8px 0;
                    }
                    .message {
                        margin-bottom: 12px;
                        padding: 8px 12px;
                        border-radius: 8px;
                        max-width: 100%;
                        word-wrap: break-word;
                    }
                    .message.user {
                        background-color: var(--vscode-button-background);
                        color: var(--vscode-button-foreground);
                        margin-left: 20px;
                    }
                    .message.assistant {
                        background-color: var(--vscode-editor-background);
                        border: 1px solid var(--vscode-panel-border);
                    }
                    .message pre {
                        background-color: var(--vscode-textBlockQuote-background);
                        padding: 8px;
                        border-radius: 4px;
                        overflow-x: auto;
                        margin: 8px 0;
                    }
                    .message code {
                        font-family: var(--vscode-editor-font-family);
                        font-size: 12px;
                    }
                    .typing {
                        display: none;
                        padding: 8px 12px;
                        color: var(--vscode-descriptionForeground);
                        font-style: italic;
                    }
                    .typing.visible {
                        display: block;
                    }
                    .input-container {
                        display: flex;
                        gap: 8px;
                        padding-top: 8px;
                        border-top: 1px solid var(--vscode-panel-border);
                    }
                    .input-container input {
                        flex: 1;
                        padding: 8px;
                        border: 1px solid var(--vscode-input-border);
                        background-color: var(--vscode-input-background);
                        color: var(--vscode-input-foreground);
                        border-radius: 4px;
                        outline: none;
                    }
                    .input-container input:focus {
                        border-color: var(--vscode-focusBorder);
                    }
                    .input-container button {
                        padding: 8px 16px;
                        background-color: var(--vscode-button-background);
                        color: var(--vscode-button-foreground);
                        border: none;
                        border-radius: 4px;
                        cursor: pointer;
                    }
                    .input-container button:hover {
                        background-color: var(--vscode-button-hoverBackground);
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
                    }
                </style>
            </head>
            <body>
                <div class="header">
                    <h3>Sovereign Chat</h3>
                    <button class="clear-btn" onclick="clearChat()">Clear</button>
                </div>
                <div class="messages" id="messages">
                    <div class="empty-state" id="emptyState">
                        <h4>Local-First AI Assistant</h4>
                        <p>Ask questions about your code or get help with programming tasks. Your conversations stay on your machine.</p>
                    </div>
                </div>
                <div class="typing" id="typing">Thinking...</div>
                <div class="input-container">
                    <input type="text" id="messageInput" placeholder="Ask about your code..." onkeypress="handleKeyPress(event)">
                    <button onclick="sendMessage()">Send</button>
                </div>

                <script>
                    const vscode = acquireVsCodeApi();
                    const messagesContainer = document.getElementById('messages');
                    const emptyState = document.getElementById('emptyState');
                    const typingIndicator = document.getElementById('typing');
                    const messageInput = document.getElementById('messageInput');

                    function sendMessage() {
                        const content = messageInput.value.trim();
                        if (!content) return;

                        messageInput.value = '';
                        vscode.postMessage({ type: 'sendMessage', content });
                    }

                    function clearChat() {
                        vscode.postMessage({ type: 'clear' });
                    }

                    function handleKeyPress(event) {
                        if (event.key === 'Enter') {
                            sendMessage();
                        }
                    }

                    function formatMessage(content) {
                        // Basic markdown formatting
                        return content
                            .replace(/\`\`\`(\\w*)\\n([\\s\\S]*?)\`\`\`/g, '<pre><code>$2</code></pre>')
                            .replace(/\`([^\`]+)\`/g, '<code>$1</code>')
                            .replace(/\\n/g, '<br>');
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
                            '<div class="message ' + msg.role + '">' + formatMessage(msg.content) + '</div>'
                        ).join('');

                        messagesContainer.scrollTop = messagesContainer.scrollHeight;
                    }

                    window.addEventListener('message', event => {
                        const message = event.data;
                        if (message.type === 'updateMessages') {
                            renderMessages(message.messages);
                        } else if (message.type === 'typing') {
                            typingIndicator.classList.toggle('visible', message.isTyping);
                        }
                    });
                </script>
            </body>
            </html>
        `;
    }
}
