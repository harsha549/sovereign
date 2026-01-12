package com.sovereign.plugin.actions

import com.sovereign.plugin.services.SovereignService

class ReviewCodeAction : BaseCodeAction() {
    override fun getActionTitle() = "Reviewing Code..."

    override fun processCode(service: SovereignService, code: String, language: String?): String {
        return service.reviewCode(code, language)
    }
}
