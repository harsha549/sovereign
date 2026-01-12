package com.sovereign.plugin.services

import com.google.gson.Gson
import com.google.gson.JsonObject
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit

@Service
class OllamaService {
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .writeTimeout(30, TimeUnit.SECONDS)
        .build()

    private val gson = Gson()
    private val jsonMediaType = "application/json".toMediaType()

    var baseUrl: String = "http://localhost:11434"
    var model: String = "qwen2.5-coder:14b"

    companion object {
        fun getInstance(): OllamaService = service()
    }

    fun isAvailable(): Boolean {
        return try {
            val request = Request.Builder()
                .url("$baseUrl/api/tags")
                .get()
                .build()
            client.newCall(request).execute().use { response ->
                response.isSuccessful
            }
        } catch (e: Exception) {
            false
        }
    }

    fun generate(prompt: String, system: String? = null): String {
        val requestBody = buildJsonObject {
            put("model", model)
            put("prompt", prompt)
            put("stream", false)
            system?.let { put("system", it) }
        }

        val request = Request.Builder()
            .url("$baseUrl/api/generate")
            .post(requestBody.toString().toRequestBody(jsonMediaType))
            .build()

        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                throw IOException("Ollama error: ${response.code} ${response.message}")
            }
            val json = gson.fromJson(response.body?.string(), JsonObject::class.java)
            return json.get("response")?.asString ?: ""
        }
    }

    fun generateStreaming(prompt: String, system: String? = null, onToken: (String) -> Unit): String {
        val requestBody = buildJsonObject {
            put("model", model)
            put("prompt", prompt)
            put("stream", true)
            system?.let { put("system", it) }
        }

        val request = Request.Builder()
            .url("$baseUrl/api/generate")
            .post(requestBody.toString().toRequestBody(jsonMediaType))
            .build()

        val fullResponse = StringBuilder()

        client.newCall(request).execute().use { response ->
            if (!response.isSuccessful) {
                throw IOException("Ollama error: ${response.code} ${response.message}")
            }

            response.body?.source()?.let { source ->
                while (!source.exhausted()) {
                    val line = source.readUtf8Line() ?: break
                    if (line.isNotBlank()) {
                        try {
                            val json = gson.fromJson(line, JsonObject::class.java)
                            val token = json.get("response")?.asString ?: ""
                            fullResponse.append(token)
                            onToken(token)
                        } catch (e: Exception) {
                            // Skip malformed JSON
                        }
                    }
                }
            }
        }

        return fullResponse.toString()
    }

    fun explainCode(code: String, language: String? = null): String {
        val langHint = language?.let { " ($it)" } ?: ""
        val prompt = """
            Explain the following code$langHint:

            ```
            $code
            ```

            Provide a clear explanation of what this code does, its purpose, and any important details.
        """.trimIndent()

        val system = "You are an expert code explainer. Provide clear, concise explanations that help developers understand code quickly."
        return generate(prompt, system)
    }

    fun reviewCode(code: String, language: String? = null): String {
        val langHint = language?.let { " ($it)" } ?: ""
        val prompt = """
            Review the following code$langHint:

            ```
            $code
            ```

            Provide a thorough code review covering:
            1. Potential bugs or issues
            2. Performance considerations
            3. Security concerns
            4. Code quality and readability
            5. Suggestions for improvement
        """.trimIndent()

        val system = "You are a senior software engineer conducting a code review. Be thorough but constructive."
        return generate(prompt, system)
    }

    fun generateCode(description: String, language: String? = null): String {
        val langHint = language?.let { " in $it" } ?: ""
        val prompt = """
            Generate code$langHint for the following request:

            $description

            Provide only the code without explanations unless necessary.
        """.trimIndent()

        val system = "You are an expert programmer. Generate clean, efficient, and well-documented code."
        val response = generate(prompt, system)
        return extractCode(response)
    }

    fun refactorCode(code: String, instructions: String, language: String? = null): String {
        val langHint = language?.let { " ($it)" } ?: ""
        val prompt = """
            Refactor the following code$langHint according to these instructions: $instructions

            ```
            $code
            ```

            Provide the refactored code.
        """.trimIndent()

        val system = "You are an expert at code refactoring. Improve code quality while maintaining functionality."
        val response = generate(prompt, system)
        return extractCode(response)
    }

    fun fixBug(code: String, bugDescription: String, language: String? = null): String {
        val langHint = language?.let { " ($it)" } ?: ""
        val prompt = """
            Fix the following bug in this code$langHint:

            Bug description: $bugDescription

            ```
            $code
            ```

            Provide the fixed code.
        """.trimIndent()

        val system = "You are an expert debugger. Identify and fix bugs while ensuring the fix does not introduce new issues."
        val response = generate(prompt, system)
        return extractCode(response)
    }

    fun generateTests(code: String, language: String? = null): String {
        val langHint = language?.let { " ($it)" } ?: ""
        val prompt = """
            Generate comprehensive unit tests for the following code$langHint:

            ```
            $code
            ```

            Include tests for normal operation, edge cases, and error conditions.
        """.trimIndent()

        val system = "You are a test engineer. Write comprehensive, well-structured tests."
        val response = generate(prompt, system)
        return extractCode(response)
    }

    fun chat(message: String, context: String? = null): String {
        val prompt = if (context != null) {
            "Context:\n$context\n\nUser: $message"
        } else {
            message
        }

        val system = "You are a helpful AI coding assistant. Be concise but thorough."
        return generate(prompt, system)
    }

    private fun extractCode(response: String): String {
        val codeBlockRegex = Regex("```[\\w]*\\n([\\s\\S]*?)```")
        val match = codeBlockRegex.find(response)
        return match?.groupValues?.get(1)?.trim() ?: response.trim()
    }

    private fun buildJsonObject(builder: JsonObjectBuilder.() -> Unit): JsonObject {
        return JsonObjectBuilder().apply(builder).build()
    }

    private class JsonObjectBuilder {
        private val json = JsonObject()

        fun put(key: String, value: String) {
            json.addProperty(key, value)
        }

        fun put(key: String, value: Boolean) {
            json.addProperty(key, value)
        }

        fun build(): JsonObject = json
    }
}
