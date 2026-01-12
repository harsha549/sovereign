export class OllamaClient {
    private baseUrl: string;
    private model: string;

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

    async explainCode(code: string, language?: string): Promise<string> {
        const langHint = language ? ` (${language})` : '';
        const prompt = `Explain the following code${langHint}:\n\n\`\`\`\n${code}\n\`\`\`\n\nProvide a clear explanation of what this code does, its purpose, and any important details.`;

        const system = 'You are an expert code explainer. Provide clear, concise explanations that help developers understand code quickly. Focus on the purpose, logic flow, and important implementation details.';

        return this.generate(prompt, system);
    }

    async generateCode(request: string, language?: string): Promise<string> {
        const langHint = language ? ` in ${language}` : '';
        const prompt = `Generate code${langHint} for the following request:\n\n${request}\n\nProvide only the code without explanations unless necessary for understanding.`;

        const system = 'You are an expert programmer. Generate clean, efficient, and well-documented code. Follow best practices and modern conventions.';

        const response = await this.generate(prompt, system);
        return this.extractCode(response);
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

    async chat(message: string, context?: string): Promise<string> {
        let prompt = message;
        if (context) {
            prompt = `Context:\n${context}\n\nUser: ${message}`;
        }

        const system = 'You are a helpful AI coding assistant. You are knowledgeable about programming, software architecture, and best practices. Be concise but thorough.';

        return this.generate(prompt, system);
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
