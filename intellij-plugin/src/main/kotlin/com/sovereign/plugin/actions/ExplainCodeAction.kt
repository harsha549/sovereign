package com.sovereign.plugin.actions

import com.sovereign.plugin.services.SovereignService

class ExplainCodeAction : BaseCodeAction() {
    override fun getActionTitle() = "Explaining Code..."

    override fun processCode(service: SovereignService, code: String, language: String?): String {
        return service.explainCode(code, language)
    }
}
