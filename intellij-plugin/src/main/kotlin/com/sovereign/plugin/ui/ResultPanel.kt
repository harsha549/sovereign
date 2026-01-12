package com.sovereign.plugin.ui

import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.DialogWrapper
import com.intellij.ui.components.JBScrollPane
import com.intellij.ui.components.JBTextArea
import com.intellij.util.ui.JBUI
import java.awt.BorderLayout
import java.awt.Dimension
import java.awt.Toolkit
import java.awt.datatransfer.StringSelection
import javax.swing.JButton
import javax.swing.JComponent
import javax.swing.JPanel

class ResultPanel private constructor(
    project: Project,
    private val title: String,
    private val content: String
) : DialogWrapper(project, true) {

    companion object {
        fun show(project: Project, title: String, content: String) {
            ResultPanel(project, title, content).show()
        }
    }

    init {
        init()
        setTitle(title)
    }

    override fun createCenterPanel(): JComponent {
        val panel = JPanel(BorderLayout())
        panel.preferredSize = Dimension(700, 500)
        panel.border = JBUI.Borders.empty(8)

        val textArea = JBTextArea(content).apply {
            isEditable = false
            lineWrap = true
            wrapStyleWord = true
            font = font.deriveFont(13f)
            margin = JBUI.insets(8)
        }

        val scrollPane = JBScrollPane(textArea)
        panel.add(scrollPane, BorderLayout.CENTER)

        val buttonPanel = JPanel().apply {
            border = JBUI.Borders.emptyTop(8)

            val copyButton = JButton("Copy to Clipboard").apply {
                addActionListener {
                    val selection = StringSelection(content)
                    Toolkit.getDefaultToolkit().systemClipboard.setContents(selection, null)
                }
            }
            add(copyButton)
        }
        panel.add(buttonPanel, BorderLayout.SOUTH)

        return panel
    }

    override fun createActions() = arrayOf(okAction)
}
