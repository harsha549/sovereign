package com.sovereign.plugin.services

import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.project.Project

@Service(Service.Level.PROJECT)
class SovereignService(private val project: Project) {
    private val ollamaService = OllamaService.getInstance()

    companion object {
        fun getInstance(project: Project): SovereignService = project.service()
    }

    fun checkOllamaStatus(): Boolean {
        val available = ollamaService.isAvailable()
        if (!available) {
            showNotification(
                "Ollama Not Running",
                "Start Ollama with: ollama serve",
                NotificationType.WARNING
            )
        }
        return available
    }

    fun explainCode(code: String, language: String?): String {
        return ollamaService.explainCode(code, language)
    }

    fun reviewCode(code: String, language: String?): String {
        return ollamaService.reviewCode(code, language)
    }

    fun generateCode(description: String, language: String?): String {
        return ollamaService.generateCode(description, language)
    }

    fun refactorCode(code: String, instructions: String, language: String?): String {
        return ollamaService.refactorCode(code, instructions, language)
    }

    fun fixBug(code: String, bugDescription: String, language: String?): String {
        return ollamaService.fixBug(code, bugDescription, language)
    }

    fun generateTests(code: String, language: String?): String {
        return ollamaService.generateTests(code, language)
    }

    fun chat(message: String, context: String? = null): String {
        return ollamaService.chat(message, context)
    }

    fun chatStreaming(message: String, context: String? = null, onToken: (String) -> Unit): String {
        val prompt = if (context != null) {
            "Context:\n$context\n\nUser: $message"
        } else {
            message
        }
        val system = "You are a helpful AI coding assistant. Be concise but thorough."
        return ollamaService.generateStreaming(prompt, system, onToken)
    }

    private fun showNotification(title: String, content: String, type: NotificationType) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup("Sovereign Notifications")
            .createNotification(title, content, type)
            .notify(project)
    }
}
