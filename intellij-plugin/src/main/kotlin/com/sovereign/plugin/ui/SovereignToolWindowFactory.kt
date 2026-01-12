package com.sovereign.plugin.ui

import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindow
import com.intellij.openapi.wm.ToolWindowFactory
import com.intellij.ui.content.ContentFactory

class SovereignToolWindowFactory : ToolWindowFactory {
    override fun createToolWindowContent(project: Project, toolWindow: ToolWindow) {
        val chatPanel = ChatPanel(project)
        val contentFactory = ContentFactory.getInstance()
        val content = contentFactory.createContent(chatPanel, "Chat", false)
        toolWindow.contentManager.addContent(content)
    }
}
