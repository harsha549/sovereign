package com.sovereign.plugin.actions

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.progress.Task
import com.intellij.openapi.ui.Messages
import com.sovereign.plugin.services.SovereignService

class GenerateCodeAction : AnAction() {
    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(CommonDataKeys.EDITOR)

        val description = Messages.showInputDialog(
            project,
            "Describe the code you want to generate:",
            "Generate Code",
            null
        )

        if (description.isNullOrBlank()) {
            return
        }

        val service = SovereignService.getInstance(project)
        if (!service.checkOllamaStatus()) {
            return
        }

        val language = editor?.let {
            val file = FileDocumentManager.getInstance().getFile(it.document)
            file?.extension
        }

        ProgressManager.getInstance().run(object : Task.Backgroundable(project, "Generating Code...", true) {
            override fun run(indicator: ProgressIndicator) {
                indicator.isIndeterminate = true
                try {
                    val code = service.generateCode(description, language)
                    ApplicationManager.getApplication().invokeLater {
                        if (editor != null) {
                            WriteCommandAction.runWriteCommandAction(project) {
                                val offset = editor.caretModel.offset
                                editor.document.insertString(offset, code)
                            }
                        } else {
                            Messages.showInfoMessage(project, code, "Generated Code")
                        }
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
