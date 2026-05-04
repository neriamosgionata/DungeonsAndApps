package com.dungeonsandapps.model

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class User(
    val id: String,
    val email: String,
    val display_name: String,
    val role: String,
    val language: String,
    val avatar_url: String? = null
)

@Serializable
data class AuthRes(val token: String, val user: User)

@Serializable
data class Campaign(
    val id: String,
    val name: String,
    val description: String? = null,
    val master_id: String
)

@Serializable
data class Character(
    val id: String,
    val campaign_id: String,
    val owner_id: String,
    val name: String,
    val race: String? = null,
    val class_primary: String? = null,
    val level_total: Int,
    val sheet: JsonElement
)

@Serializable
data class DiceRollTerm(
    val expr: String,
    val kind: String,
    val rolls: List<Int>,
    val kept: List<Int>,
    val value: Int
)

@Serializable
data class DiceRollResult(
    val id: String? = null,
    val expression: String,
    val total: Int,
    val terms: List<DiceRollTerm>,
    val label: String? = null
)
