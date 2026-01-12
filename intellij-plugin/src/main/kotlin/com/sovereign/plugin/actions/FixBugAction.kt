package com.sovereign.plugin.actions

import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.progress.Task
import com.intellij.openapi.ui.Messages
import com.sovereign.plugin.services.SovereignService

class FixBugAction : BaseCodeAction() {
    override fun getActionTitle() = "Fixing Bug..."

    override fun processCode(service: SovereignService, code: String, language: String?): String {
        return ""
    }

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(CommonDataKeys.EDITOR) ?: return

        val selectedText = editor.selectionModel.selectedText
        if (selectedText.isNullOrBlank()) {
            Messages.showWarningDialog(project, "Please select code first", "No Selection")
            return
        }

        val bugDescription = Messages.showInputDialog(
            project,
            "Describe the bug:",
            "Fix Bug",
            null
        )

        if (bugDescription.isNullOrBlank()) {
            return
        }

        val service = SovereignService.getInstance(project)
        if (!service.checkOllamaStatus()) {
            return
        }

        val language = getLanguage(editor)

        ProgressManager.getInstance().run(object : Task.Backgroundable(project, getActionTitle(), true) {
            override fun run(indicator: ProgressIndicator) {
                indicator.isIndeterminate = true
                try {
                    val result = service.fixBug(selectedText, bugDescription, language)
                    ApplicationManager.getApplication().invokeLater {
                        showResult(project, "Fixed Code", result)
                    }
                } catch (ex: Exception) {
                    ApplicationManager.getApplication().invokeLater {
                        Messages.showErrorDialog(project, "Error: ${ex.message}", "Sovereign Error")
                    }
                }
            }
        })
    }
}
