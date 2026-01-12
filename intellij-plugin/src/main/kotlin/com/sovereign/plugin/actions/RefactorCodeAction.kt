package com.sovereign.plugin.actions

import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.progress.Task
import com.intellij.openapi.ui.Messages
import com.sovereign.plugin.services.SovereignService

class RefactorCodeAction : BaseCodeAction() {
    override fun getActionTitle() = "Refactoring Code..."

    override fun processCode(service: SovereignService, code: String, language: String?): String {
        // This won't be called directly - we override actionPerformed
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

        val instructions = Messages.showInputDialog(
            project,
            "How should the code be refactored?",
            "Refactor Code",
            null
        )

        if (instructions.isNullOrBlank()) {
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
                    val result = service.refactorCode(selectedText, instructions, language)
                    ApplicationManager.getApplication().invokeLater {
                        showResult(project, "Refactored Code", result)
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
