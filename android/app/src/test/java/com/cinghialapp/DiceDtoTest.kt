package com.cinghialapp

import com.cinghialapp.model.DiceRollResult
import com.cinghialapp.model.DiceRollTerm
import com.cinghialapp.model.Campaign
import kotlinx.serialization.json.Json
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class DiceDtoTest {
    private val json = Json { ignoreUnknownKeys = true; explicitNulls = false }

    @Test
    fun diceRollDeserializesFromBackendShape() {
        val payload = """
            {"id":"00000000-0000-0000-0000-000000000001",
             "expression":"1d20+5","total":18,
             "terms":[{"expr":"1d20","kind":"dice","rolls":[13],"kept":[13],"value":13},
                      {"expr":"5","kind":"modifier","rolls":[],"kept":[],"value":5}]}
        """.trimIndent()
        val r = json.decodeFromString<DiceRollResult>(payload)
        assertEquals(18, r.total)
        assertEquals("1d20+5", r.expression)
        assertEquals(2, r.terms.size)
    }

    @Test
    fun campaignDeserializesWithNullDescription() {
        val payload = """{"id":"a","name":"n","master_id":"m"}"""
        val c = json.decodeFromString<Campaign>(payload)
        assertEquals("n", c.name)
        assertEquals(null, c.description)
    }

    @Test
    fun diceRollTermRoundtrip() {
        val t = DiceRollTerm("1d8", "dice", listOf(4), listOf(4), 4)
        val s = json.encodeToString(DiceRollTerm.serializer(), t)
        val back = json.decodeFromString(DiceRollTerm.serializer(), s)
        assertEquals(t, back)
    }
}
