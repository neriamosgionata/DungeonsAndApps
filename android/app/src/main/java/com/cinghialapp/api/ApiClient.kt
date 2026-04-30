package com.cinghialapp.api

import io.ktor.client.HttpClient
import io.ktor.client.engine.HttpClientEngine
import io.ktor.client.engine.okhttp.OkHttp
import io.ktor.client.plugins.DefaultRequest
import io.ktor.client.plugins.HttpTimeout
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.client.plugins.logging.LogLevel
import io.ktor.client.plugins.logging.Logging
import io.ktor.client.request.headers
import io.ktor.http.ContentType
import io.ktor.http.contentType
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json

object ApiConfig {
    // Default to emulator localhost; override in BuildConfig for production
    const val BASE_URL = "http://10.0.2.2:8080/api/v1"
    const val WS_URL   = "ws://10.0.2.2:8080/ws"
}

object TimeoutConfig {
    const val CONNECT_MS = 10000L
    const val REQUEST_MS = 30000L
    const val SOCKET_MS = 30000L
}

fun createHttpClient(
    tokenProvider: () -> String? = { null },
    engine: HttpClientEngine? = null,
    enableLogging: Boolean = false
): HttpClient {
    val factory = engine ?: OkHttp.create()
    return HttpClient(factory) {
        expectSuccess = true
        
        // Fix: Add timeout configuration
        install(HttpTimeout) {
            connectTimeoutMillis = TimeoutConfig.CONNECT_MS
            requestTimeoutMillis = TimeoutConfig.REQUEST_MS
            socketTimeoutMillis = TimeoutConfig.SOCKET_MS
        }
        
        install(ContentNegotiation) {
            json(Json { ignoreUnknownKeys = true; explicitNulls = false })
        }
        
        // Fix: Only install Logging in debug builds
        if (enableLogging) {
            install(Logging) {
                level = LogLevel.HEADERS
            }
        }
        
        install(DefaultRequest) {
            contentType(ContentType.Application.Json)
            headers {
                tokenProvider()?.let { append("Authorization", "Bearer $it") }
            }
        }
    }
}
