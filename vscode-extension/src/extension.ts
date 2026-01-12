import * as vscode from 'vscode';
import { OllamaClient } from './ollama';
import { ChatViewProvider } from './chatView';

let ollamaClient: OllamaClient;
let chatViewProvider: ChatViewProvider;

export function activate(context: vscode.ExtensionContext) {
    console.log('Sovereign extension activated');

    // Initialize Ollama client
    const config = vscode.workspace.getConfiguration('sovereign');
    const ollamaUrl = config.get<string>('ollamaUrl') || 'http://localhost:11434';
    const model = config.get<string>('model') || 'qwen2.5-coder:14b';

    ollamaClient = new OllamaClient(ollamaUrl, model);

    // Initialize chat view
    chatViewProvider = new ChatViewProvider(context.extensionUri, ollamaClient);
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider('sovereign.chatView', chatViewProvider)
    );

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('sovereign.chat', () => openChat()),
        vscode.commands.registerCommand('sovereign.explain', () => explainSelection()),
        vscode.commands.registerCommand('sovereign.generate', () => generateCode()),
        vscode.commands.registerCommand('sovereign.review', () => reviewCode()),
        vscode.commands.registerCommand('sovereign.refactor', () => refactorCode()),
        vscode.commands.registerCommand('sovereign.fix', () => fixBug()),
        vscode.commands.registerCommand('sovereign.tests', () => generateTests()),
        vscode.commands.registerCommand('sovereign.index', () => indexWorkspace()),
        vscode.commands.registerCommand('sovereign.search', () => searchCodebase())
    );

    // Check Ollama availability
    checkOllamaStatus();
}

async function checkOllamaStatus() {
    const available = await ollamaClient.isAvailable();
    if (!available) {
        vscode.window.showWarningMessage(
            'Sovereign: Ollama is not running. Start it with: ollama serve',
            'Open Terminal'
        ).then(selection => {
            if (selection === 'Open Terminal') {
                vscode.commands.executeCommand('workbench.action.terminal.new');
            }
        });
    }
}

function openChat() {
    vscode.commands.executeCommand('sovereign.chatView.focus');
}

async function explainSelection() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    const selection = editor.selection;
    const code = editor.document.getText(selection);

    if (!code) {
        vscode.window.showErrorMessage('No code selected');
        return;
    }

    const language = editor.document.languageId;

    vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: 'Sovereign: Explaining code...',
        cancellable: true
    }, async (progress, token) => {
        try {
            const explanation = await ollamaClient.explainCode(code, language);
            showResultPanel('Code Explanation', explanation);
        } catch (error) {
            vscode.window.showErrorMessage(`Error: ${error}`);
        }
    });
}

async function generateCode() {
    const prompt = await vscode.window.showInputBox({
        prompt: 'Describe the code you want to generate',
        placeHolder: 'e.g., A function that sorts an array of objects by a key'
    });

    if (!prompt) {
        return;
    }

    const editor = vscode.window.activeTextEditor;
    const language = editor?.document.languageId;

    vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: 'Sovereign: Generating code...',
        cancellable: true
    }, async (progress, token) => {
        try {
            const code = await ollamaClient.generateCode(prompt, language);

            if (editor) {
                const position = editor.selection.active;
                editor.edit(editBuilder => {
                    editBuilder.insert(position, code);
                });
            } else {
                showResultPanel('Generated Code', code);
            }
        } catch (error) {
            vscode.window.showErrorMessage(`Error: ${error}`);
        }
    });
}

async function reviewCode() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    const selection = editor.selection;
    const code = selection.isEmpty
        ? editor.document.getText()
        : editor.document.getText(selection);

    const language = editor.document.languageId;

    vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: 'Sovereign: Reviewing code...',
        cancellable: true
    }, async (progress, token) => {
        try {
            const review = await ollamaClient.reviewCode(code, language);
            showResultPanel('Code Review', review);
        } catch (error) {
            vscode.window.showErrorMessage(`Error: ${error}`);
        }
    });
}

async function refactorCode() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    const selection = editor.selection;
    const code = editor.document.getText(selection);

    if (!code) {
        vscode.window.showErrorMessage('No code selected');
        return;
    }

    const instructions = await vscode.window.showInputBox({
        prompt: 'How should the code be refactored?',
        placeHolder: 'e.g., Extract common logic into a helper function'
    });

    if (!instructions) {
        return;
    }

    const language = editor.document.languageId;

    vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: 'Sovereign: Refactoring code...',
        cancellable: true
    }, async (progress, token) => {
        try {
            const refactored = await ollamaClient.refactorCode(code, instructions, language);

            // Show diff
            const document = await vscode.workspace.openTextDocument({
                content: refactored,
                language: language
            });
            vscode.window.showTextDocument(document, { viewColumn: vscode.ViewColumn.Beside });
        } catch (error) {
            vscode.window.showErrorMessage(`Error: ${error}`);
        }
    });
}

async function fixBug() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    const selection = editor.selection;
    const code = editor.document.getText(selection);

    if (!code) {
        vscode.window.showErrorMessage('No code selected');
        return;
    }

    const bugDescription = await vscode.window.showInputBox({
        prompt: 'Describe the bug',
        placeHolder: 'e.g., The function returns undefined when the array is empty'
    });

    if (!bugDescription) {
        return;
    }

    const language = editor.document.languageId;

    vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: 'Sovereign: Fixing bug...',
        cancellable: true
    }, async (progress, token) => {
        try {
            const fixed = await ollamaClient.fixBug(code, bugDescription, language);

            const document = await vscode.workspace.openTextDocument({
                content: fixed,
                language: language
            });
            vscode.window.showTextDocument(document, { viewColumn: vscode.ViewColumn.Beside });
        } catch (error) {
            vscode.window.showErrorMessage(`Error: ${error}`);
        }
    });
}

async function generateTests() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('No active editor');
        return;
    }

    const selection = editor.selection;
    const code = selection.isEmpty
        ? editor.document.getText()
        : editor.document.getText(selection);

    const language = editor.document.languageId;

    vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: 'Sovereign: Generating tests...',
        cancellable: true
    }, async (progress, token) => {
        try {
            const tests = await ollamaClient.generateTests(code, language);

            const document = await vscode.workspace.openTextDocument({
                content: tests,
                language: language
            });
            vscode.window.showTextDocument(document, { viewColumn: vscode.ViewColumn.Beside });
        } catch (error) {
            vscode.window.showErrorMessage(`Error: ${error}`);
        }
    });
}

async function indexWorkspace() {
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (!workspaceFolders || workspaceFolders.length === 0) {
        vscode.window.showErrorMessage('No workspace folder open');
        return;
    }

    vscode.window.showInformationMessage(
        'Sovereign: Indexing is handled by the CLI. Run: sovereign index ' + workspaceFolders[0].uri.fsPath
    );
}

async function searchCodebase() {
    const query = await vscode.window.showInputBox({
        prompt: 'Search the codebase',
        placeHolder: 'e.g., authentication middleware'
    });

    if (!query) {
        return;
    }

    vscode.window.showInformationMessage(
        'Sovereign: Search is handled by the CLI. Run: sovereign search "' + query + '"'
    );
}

function showResultPanel(title: string, content: string) {
    const panel = vscode.window.createWebviewPanel(
        'sovereignResult',
        title,
        vscode.ViewColumn.Beside,
        {}
    );

    panel.webview.html = `
        <!DOCTYPE html>
        <html>
        <head>
            <style>
                body {
                    font-family: var(--vscode-font-family);
                    padding: 16px;
                    color: var(--vscode-foreground);
                    background-color: var(--vscode-editor-background);
                }
                pre {
                    background-color: var(--vscode-textBlockQuote-background);
                    padding: 12px;
                    border-radius: 4px;
                    overflow-x: auto;
                }
                code {
                    font-family: var(--vscode-editor-font-family);
                }
            </style>
        </head>
        <body>
            <h2>${title}</h2>
            <div>${formatContent(content)}</div>
        </body>
        </html>
    `;
}

function formatContent(content: string): string {
    // Basic markdown-like formatting
    return content
        .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>')
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\n/g, '<br>');
}

export function deactivate() {
    console.log('Sovereign extension deactivated');
}
