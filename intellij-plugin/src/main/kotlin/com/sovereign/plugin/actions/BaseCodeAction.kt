package com.sovereign.plugin.actions

import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.Messages
import com.intellij.openapi.wm.ToolWindowManager
import com.sovereign.plugin.services.SovereignService
import com.sovereign.plugin.ui.ResultPanel

abstract class BaseCodeAction : AnAction() {

    abstract fun getActionTitle(): String
    abstract fun processCode(service: SovereignService, code: String, language: String?): String

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val editor = e.getData(CommonDataKeys.EDITOR) ?: return

        val selectedText = editor.selectionModel.selectedText
        if (selectedText.isNullOrBlank()) {
            Messages.showWarningDialog(project, "Please select code first", "No Selection")
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
                    val result = processCode(service, selectedText, language)
                    ApplicationManager.getApplication().invokeLater {
                        showResult(project, getActionTitle(), result)
                    }
                } catch (ex: Exception) {
                    ApplicationManager.getApplication().invokeLater {
                        Messages.showErrorDialog(project, "Error: ${ex.message}", "Sovereign Error")
                    }
                }
            }
        })
    }

    override fun update(e: AnActionEvent) {
        val editor = e.getData(CommonDataKeys.EDITOR)
        e.presentation.isEnabledAndVisible = editor != null && editor.selectionModel.hasSelection()
    }

    protected fun getLanguage(editor: Editor): String? {
        val file = FileDocumentManager.getInstance().getFile(editor.document)
        return file?.extension
    }

    protected fun showResult(project: Project, title: String, content: String) {
        ResultPanel.show(project, title, content)
    }

    protected fun insertCode(project: Project, editor: Editor, code: String) {
        WriteCommandAction.runWriteCommandAction(project) {
            val offset = editor.caretModel.offset
            editor.document.insertString(offset, code)
        }
    }

    protected fun replaceSelection(project: Project, editor: Editor, code: String) {
        WriteCommandAction.runWriteCommandAction(project) {
            val selectionModel = editor.selectionModel
            editor.document.replaceString(
                selectionModel.selectionStart,
                selectionModel.selectionEnd,
                code
            )
        }
    }
}
