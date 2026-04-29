package com.cinghialapp.api

import io.ktor.client.HttpClient
import io.ktor.client.engine.HttpClientEngine
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.DefaultRequest
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.client.plugins.logging.Logging
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.request.headers
import io.ktor.http.ContentType
import io.ktor.http.contentType
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json

object ApiConfig {
    const val BASE_URL = "http://10.0.2.2:8080/api/v1"
    const val WS_URL   = "ws://10.0.2.2:8080/ws"
}

fun createHttpClient(
    tokenProvider: () -> String? = { null },
    engine: HttpClientEngine? = null
): HttpClient {
    val factory = engine ?: OkHttp.create()
    return HttpClient(factory) {
        expectSuccess = true
        install(ContentNegotiation) {
            json(Json { ignoreUnknownKeys = true; explicitNulls = false })
        }
        install(WebSockets)
        install(Logging)
        install(DefaultRequest) {
            contentType(ContentType.Application.Json)
            headers {
                tokenProvider()?.let { append("Authorization", "Bearer $it") }
            }
        }
    }
}
