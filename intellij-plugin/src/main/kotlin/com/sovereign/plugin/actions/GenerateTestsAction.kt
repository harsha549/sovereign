package com.sovereign.plugin.actions

import com.sovereign.plugin.services.SovereignService

class GenerateTestsAction : BaseCodeAction() {
    override fun getActionTitle() = "Generating Tests..."

    override fun processCode(service: SovereignService, code: String, language: String?): String {
        return service.generateTests(code, language)
    }
}
