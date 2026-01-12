package com.sovereign.plugin.ui

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.project.Project
import com.intellij.ui.JBColor
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.components.JBTextArea
import com.intellij.util.ui.JBUI
import com.sovereign.plugin.services.SovereignService
import kotlinx.coroutines.*
import java.awt.BorderLayout
import java.awt.Dimension
import java.awt.event.KeyAdapter
import java.awt.event.KeyEvent
import javax.swing.*

class ChatPanel(private val project: Project) : JPanel(BorderLayout()) {
    private val chatHistory = JBTextArea().apply {
        isEditable = false
        lineWrap = true
        wrapStyleWord = true
        font = font.deriveFont(13f)
        margin = JBUI.insets(8)
    }

    private val inputField = JBTextArea(3, 40).apply {
        lineWrap = true
        wrapStyleWord = true
        font = font.deriveFont(13f)
        margin = JBUI.insets(8)
    }

    private val sendButton = JButton("Send").apply {
        preferredSize = Dimension(80, 30)
    }

    private val clearButton = JButton("Clear").apply {
        preferredSize = Dimension(80, 30)
    }

    private val scope = CoroutineScope(Dispatchers.Default + SupervisorJob())
    private var isProcessing = false

    init {
        setupUI()
        setupListeners()
        appendToChat("Sovereign", "Hello! I'm your local AI assistant. How can I help you today?\n\n")
    }

    private fun setupUI() {
        border = JBUI.Borders.empty(8)

        // Chat history
        val historyScroll = JBScrollPane(chatHistory).apply {
            verticalScrollBarPolicy = JScrollPane.VERTICAL_SCROLLBAR_AS_NEEDED
            horizontalScrollBarPolicy = JScrollPane.HORIZONTAL_SCROLLBAR_NEVER
        }
        add(historyScroll, BorderLayout.CENTER)

        // Input area
        val inputPanel = JPanel(BorderLayout()).apply {
            border = JBUI.Borders.emptyTop(8)

            val inputScroll = JBScrollPane(inputField).apply {
                preferredSize = Dimension(0, 80)
            }
            add(inputScroll, BorderLayout.CENTER)

            val buttonPanel = JPanel().apply {
                layout = BoxLayout(this, BoxLayout.Y_AXIS)
                border = JBUI.Borders.emptyLeft(8)
                add(sendButton)
                add(Box.createVerticalStrut(4))
                add(clearButton)
            }
            add(buttonPanel, BorderLayout.EAST)
        }
        add(inputPanel, BorderLayout.SOUTH)
    }

    private fun setupListeners() {
        sendButton.addActionListener { sendMessage() }
        clearButton.addActionListener { clearChat() }

        inputField.addKeyListener(object : KeyAdapter() {
            override fun keyPressed(e: KeyEvent) {
                if (e.keyCode == KeyEvent.VK_ENTER && e.isControlDown) {
                    sendMessage()
                    e.consume()
                }
            }
        })
    }

    private fun sendMessage() {
        if (isProcessing) return

        val message = inputField.text.trim()
        if (message.isEmpty()) return

        inputField.text = ""
        appendToChat("You", message + "\n\n")

        isProcessing = true
        sendButton.isEnabled = false

        scope.launch {
            try {
                val service = SovereignService.getInstance(project)
                if (!service.checkOllamaStatus()) {
                    appendToChat("Error", "Ollama is not running. Start it with: ollama serve\n\n")
                    return@launch
                }

                // Use streaming for real-time response
                appendToChat("Sovereign", "")
                service.chatStreaming(message) { token ->
                    ApplicationManager.getApplication().invokeLater {
                        chatHistory.append(token)
                        chatHistory.caretPosition = chatHistory.document.length
                    }
                }
                ApplicationManager.getApplication().invokeLater {
                    chatHistory.append("\n\n")
                }
            } catch (e: Exception) {
                appendToChat("Error", "${e.message}\n\n")
            } finally {
                ApplicationManager.getApplication().invokeLater {
                    isProcessing = false
                    sendButton.isEnabled = true
                }
            }
        }
    }

    private fun appendToChat(sender: String, message: String) {
        ApplicationManager.getApplication().invokeLater {
            if (sender.isNotEmpty()) {
                chatHistory.append("[$sender]\n$message")
            } else {
                chatHistory.append(message)
            }
            chatHistory.caretPosition = chatHistory.document.length
        }
    }

    private fun clearChat() {
        chatHistory.text = ""
        appendToChat("Sovereign", "Chat cleared. How can I help you?\n\n")
    }

    fun dispose() {
        scope.cancel()
    }
}
