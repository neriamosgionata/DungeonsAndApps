package com.dungeonsandapps.data

import android.content.Context
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map

private val Context.dataStore by preferencesDataStore(name = "dungeonsandapps")

private val TOKEN = stringPreferencesKey("token")
private val USER  = stringPreferencesKey("user")

class AuthStore(private val ctx: Context) {
    val token: Flow<String?> = ctx.dataStore.data.map { it[TOKEN] }
    val userJson: Flow<String?> = ctx.dataStore.data.map { it[USER] }

    suspend fun currentToken(): String? = token.first()

    suspend fun set(token: String, userJson: String) {
        ctx.dataStore.edit { it[TOKEN] = token; it[USER] = userJson }
    }

    suspend fun clear() {
        ctx.dataStore.edit { it.remove(TOKEN); it.remove(USER) }
    }
}
