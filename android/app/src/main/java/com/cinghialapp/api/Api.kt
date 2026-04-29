package com.cinghialapp.api

import com.cinghialapp.model.*
import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.*
import io.ktor.http.ContentType
import io.ktor.http.contentType

class Api(private val client: HttpClient, private val baseUrl: String = ApiConfig.BASE_URL) {
    suspend fun register(email: String, password: String, displayName: String, language: String = "en"): AuthRes =
        client.post("$baseUrl/auth/register") {
            contentType(ContentType.Application.Json)
            setBody(mapOf("email" to email, "password" to password, "display_name" to displayName, "language" to language))
        }.body()

    suspend fun login(email: String, password: String): AuthRes =
        client.post("$baseUrl/auth/login") {
            contentType(ContentType.Application.Json)
            setBody(mapOf("email" to email, "password" to password))
        }.body()

    suspend fun me(): User = client.get("$baseUrl/auth/me").body()

    suspend fun campaigns(): List<Campaign> = client.get("$baseUrl/campaigns").body()

    suspend fun createCampaign(name: String, description: String? = null): Campaign =
        client.post("$baseUrl/campaigns") {
            contentType(ContentType.Application.Json)
            setBody(mapOf("name" to name, "description" to description))
        }.body()

    suspend fun characters(campaignId: String): List<Character> =
        client.get("$baseUrl/campaigns/$campaignId/characters").body()

    suspend fun rollDice(campaignId: String, expression: String, label: String? = null): DiceRollResult =
        client.post("$baseUrl/campaigns/$campaignId/dice") {
            contentType(ContentType.Application.Json)
            setBody(mapOf("expression" to expression, "label" to label))
        }.body()
}
