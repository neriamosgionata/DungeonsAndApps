package com.cinghialapp

import com.cinghialapp.api.createHttpClient
import com.cinghialapp.model.DiceRollResult
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.request.get
import io.ktor.client.statement.bodyAsText
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import io.ktor.utils.io.ByteReadChannel
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class ApiClientTest {
    @Test
    fun clientParsesJson() = runBlocking {
        val engine = MockEngine { _ ->
            respond(
                content = ByteReadChannel("""{"ok":true}"""),
                status = HttpStatusCode.OK,
                headers = headersOf(HttpHeaders.ContentType, "application/json")
            )
        }
        val client = createHttpClient(engine = engine)
        val body: String = client.get("http://test/health").bodyAsText()
        assertEquals("""{"ok":true}""", body)
    }

    @Test
    fun dtoShapeRoundtrip() {
        val json = kotlinx.serialization.json.Json
        val sample = DiceRollResult(
            expression = "1d20+5",
            total = 18,
            terms = listOf()
        )
        val s = json.encodeToString(DiceRollResult.serializer(), sample)
        val back = json.decodeFromString(DiceRollResult.serializer(), s)
        assertEquals(sample.total, back.total)
        assertEquals(sample.expression, back.expression)
    }
}
