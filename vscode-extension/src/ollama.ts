export interface StreamCallback {
    onChunk: (text: string) => void;
    onComplete: (fullResponse: string) => void;
    onError: (error: Error) => void;
}

export interface ChatMessage {
    role: 'system' | 'user' | 'assistant';
    content: string;
}

export class OllamaClient {
    private baseUrl: string;
    private model: string;
    private abortController: AbortController | null = null;

    constructor(baseUrl: string, model: string) {
        this.baseUrl = baseUrl;
        this.model = model;
    }

    async isAvailable(): Promise<boolean> {
        try {
            const response = await fetch(`${this.baseUrl}/api/tags`);
            return response.ok;
        } catch {
            return false;
        }
    }

    async listModels(): Promise<string[]> {
        try {
            const response = await fetch(`${this.baseUrl}/api/tags`);
            if (!response.ok) {
                return [];
            }
            const data = await response.json() as { models: { name: string }[] };
            return data.models.map(m => m.name);
        } catch {
            return [];
        }
    }

    setModel(model: string) {
        this.model = model;
    }

    getModel(): string {
        return this.model;
    }

    cancelRequest() {
        if (this.abortController) {
            this.abortController.abort();
            this.abortController = null;
        }
    }

    async generate(prompt: string, system?: string): Promise<string> {
        const response = await fetch(`${this.baseUrl}/api/generate`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                model: this.model,
                prompt,
                system,
                stream: false
            })
        });

        if (!response.ok) {
            throw new Error(`Ollama error: ${response.statusText}`);
        }

        const data = await response.json() as { response: string };
        return data.response;
    }

    async generateStream(prompt: string, callback: StreamCallback, system?: string): Promise<void> {
        this.abortController = new AbortController();

        try {
            const response = await fetch(`${this.baseUrl}/api/generate`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    model: this.model,
                    prompt,
                    system,
                    stream: true
                }),
                signal: this.abortController.signal
            });

            if (!response.ok) {
                throw new Error(`Ollama error: ${response.statusText}`);
            }

            const reader = response.body?.getReader();
            if (!reader) {
                throw new Error('No response body');
            }

            const decoder = new TextDecoder();
            let fullResponse = '';
            let buffer = '';

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });

                // Process complete lines in buffer
                const lines = buffer.split('\n');
                buffer = lines.pop() || '';

                for (const line of lines) {
                    if (line.trim()) {
                        try {
                            const data = JSON.parse(line) as { response: string; done: boolean };
                            if (data.response) {
                                fullResponse += data.response;
                                callback.onChunk(data.response);
                            }
                        } catch {
                            // Skip malformed JSON
                        }
                    }
                }
            }

            callback.onComplete(fullResponse);
        } catch (error) {
            if (error instanceof Error && error.name === 'AbortError') {
                callback.onComplete(''); // Request was cancelled
            } else {
                callback.onError(error instanceof Error ? error : new Error(String(error)));
            }
        } finally {
            this.abortController = null;
        }
    }

    async chatStream(messages: ChatMessage[], callback: StreamCallback): Promise<void> {
        this.abortController = new AbortController();

        try {
            const response = await fetch(`${this.baseUrl}/api/chat`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    model: this.model,
                    messages,
                    stream: true
                }),
                signal: this.abortController.signal
            });

            if (!response.ok) {
                throw new Error(`Ollama error: ${response.statusText}`);
            }

            const reader = response.body?.getReader();
            if (!reader) {
                throw new Error('No response body');
            }

            const decoder = new TextDecoder();
            let fullResponse = '';
            let buffer = '';

            while (true) {
                const { done, value } = await reader.read();
                if (done) break;

                buffer += decoder.decode(value, { stream: true });

                const lines = buffer.split('\n');
                buffer = lines.pop() || '';

                for (const line of lines) {
                    if (line.trim()) {
                        try {
                            const data = JSON.parse(line) as { message?: { content: string }; done: boolean };
                            if (data.message?.content) {
                                fullResponse += data.message.content;
                                callback.onChunk(data.message.content);
                            }
                        } catch {
                            // Skip malformed JSON
                        }
                    }
                }
            }

            callback.onComplete(fullResponse);
        } catch (error) {
            if (error instanceof Error && error.name === 'AbortError') {
                callback.onComplete('');
            } else {
                callback.onError(error instanceof Error ? error : new Error(String(error)));
            }
        } finally {
            this.abortController = null;
        }
    }

    async explainCode(code: string, language?: string): Promise<string> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Explain the following code${langHint}:\n\n\`\`\`\n${code}\n\`\`\`\n\nProvide a clear explanation of what this code does, its purpose, and any important details.`;

        const system = 'You are an expert code explainer. Provide clear, concise explanations that help developers understand code quickly. Focus on the purpose, logic flow, and important implementation details.';

        return this.generate(prompt, system);
    }

    async explainCodeStream(code: string, callback: StreamCallback, language?: string): Promise<void> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Explain the following code${langHint}:\n\n\`\`\`\n${code}\n\`\`\`\n\nProvide a clear explanation of what this code does, its purpose, and any important details.`;

        const system = 'You are an expert code explainer. Provide clear, concise explanations that help developers understand code quickly. Focus on the purpose, logic flow, and important implementation details.';

        return this.generateStream(prompt, callback, system);
    }

    async generateCode(request: string, language?: string): Promise<string> {
        const langHint = language ? ` in ${language}` : '';
        const prompt = `Generate code${langHint} for the following request:\n\n${request}\n\nProvide only the code without explanations unless necessary for understanding.`;

        const system = 'You are an expert programmer. Generate clean, efficient, and well-documented code. Follow best practices and modern conventions.';

        const response = await this.generate(prompt, system);
        return this.extractCode(response);
    }

    async generateCodeStream(request: string, callback: StreamCallback, language?: string): Promise<void> {
        const langHint = language ? ` in ${language}` : '';
        const prompt = `Generate code${langHint} for the following request:\n\n${request}\n\nProvide only the code without explanations unless necessary for understanding.`;

        const system = 'You are an expert programmer. Generate clean, efficient, and well-documented code. Follow best practices and modern conventions.';

        return this.generateStream(prompt, callback, system);
    }

    async reviewCode(code: string, language?: string): Promise<string> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Review the following code${langHint}:\n\n\`\`\`\n${code}\n\`\`\`\n\nProvide a thorough code review covering:
1. Potential bugs or issues
2. Performance considerations
3. Security concerns
4. Code quality and readability
5. Suggestions for improvement`;

        const system = 'You are a senior software engineer conducting a code review. Be thorough but constructive. Focus on actionable feedback.';

        return this.generate(prompt, system);
    }

    async reviewCodeStream(code: string, callback: StreamCallback, language?: string): Promise<void> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Review the following code${langHint}:\n\n\`\`\`\n${code}\n\`\`\`\n\nProvide a thorough code review covering:
1. Potential bugs or issues
2. Performance considerations
3. Security concerns
4. Code quality and readability
5. Suggestions for improvement`;

        const system = 'You are a senior software engineer conducting a code review. Be thorough but constructive. Focus on actionable feedback.';

        return this.generateStream(prompt, callback, system);
    }

    async refactorCode(code: string, instructions: string, language?: string): Promise<string> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Refactor the following code${langHint} according to these instructions: ${instructions}\n\n\`\`\`\n${code}\n\`\`\`\n\nProvide the refactored code.`;

        const system = 'You are an expert at code refactoring. Improve code quality while maintaining functionality. Follow best practices and keep the code clean and readable.';

        const response = await this.generate(prompt, system);
        return this.extractCode(response);
    }

    async fixBug(code: string, bugDescription: string, language?: string): Promise<string> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Fix the following bug in this code${langHint}:\n\nBug description: ${bugDescription}\n\n\`\`\`\n${code}\n\`\`\`\n\nProvide the fixed code.`;

        const system = 'You are an expert debugger. Identify and fix bugs while ensuring the fix does not introduce new issues. Explain the fix briefly.';

        const response = await this.generate(prompt, system);
        return this.extractCode(response);
    }

    async generateTests(code: string, language?: string): Promise<string> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Generate comprehensive unit tests for the following code${langHint}:\n\n\`\`\`\n${code}\n\`\`\`\n\nInclude tests for:
1. Normal operation
2. Edge cases
3. Error conditions

Use appropriate testing conventions for the language.`;

        const system = 'You are a test engineer. Write comprehensive, well-structured tests that cover all important cases. Use best practices for the testing framework appropriate for the language.';

        const response = await this.generate(prompt, system);
        return this.extractCode(response);
    }

    async generateTestsStream(code: string, callback: StreamCallback, language?: string): Promise<void> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Generate comprehensive unit tests for the following code${langHint}:\n\n\`\`\`\n${code}\n\`\`\`\n\nInclude tests for:
1. Normal operation
2. Edge cases
3. Error conditions

Use appropriate testing conventions for the language.`;

        const system = 'You are a test engineer. Write comprehensive, well-structured tests that cover all important cases. Use best practices for the testing framework appropriate for the language.';

        return this.generateStream(prompt, callback, system);
    }

    async chat(message: string, context?: string): Promise<string> {
        let prompt = message;
        if (context) {
            prompt = `Context:\n${context}\n\nUser: ${message}`;
        }

        const system = 'You are a helpful AI coding assistant. You are knowledgeable about programming, software architecture, and best practices. Be concise but thorough.';

        return this.generate(prompt, system);
    }

    async chatWithHistory(messages: ChatMessage[], newMessage: string): Promise<string> {
        const allMessages: ChatMessage[] = [
            {
                role: 'system',
                content: 'You are a helpful AI coding assistant. You are knowledgeable about programming, software architecture, and best practices. Be concise but thorough.'
            },
            ...messages,
            { role: 'user', content: newMessage }
        ];

        const response = await fetch(`${this.baseUrl}/api/chat`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                model: this.model,
                messages: allMessages,
                stream: false
            })
        });

        if (!response.ok) {
            throw new Error(`Ollama error: ${response.statusText}`);
        }

        const data = await response.json() as { message: { content: string } };
        return data.message.content;
    }

    async chatWithHistoryStream(messages: ChatMessage[], newMessage: string, callback: StreamCallback): Promise<void> {
        const allMessages: ChatMessage[] = [
            {
                role: 'system',
                content: 'You are a helpful AI coding assistant. You are knowledgeable about programming, software architecture, and best practices. Be concise but thorough.'
            },
            ...messages,
            { role: 'user', content: newMessage }
        ];

        return this.chatStream(allMessages, callback);
    }

    private extractCode(response: string): string {
        // Try to extract code from markdown code blocks
        const codeBlockMatch = response.match(/```[\w]*\n([\s\S]*?)```/);
        if (codeBlockMatch) {
            return codeBlockMatch[1].trim();
        }
        return response.trim();
    }
}
