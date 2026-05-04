package com.dungeonsandapps.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.dungeonsandapps.model.Campaign
import com.dungeonsandapps.model.DiceRollResult
import kotlinx.coroutines.CancellationException

@Composable
fun LoginScreen(onLogin: suspend (String, String) -> Unit, onGoRegister: () -> Unit) {
    var email by remember { mutableStateOf("") }
    var pw by remember { mutableStateOf("") }
    var err by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope() // Fix: lifecycle-aware scope

    Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Log in", style = MaterialTheme.typography.headlineMedium)
        OutlinedTextField(
            email, 
            { email = it }, 
            label = { Text("Email") }, 
            singleLine = true,
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Email,
                imeAction = ImeAction.Next
            )
        )
        OutlinedTextField(
            pw, 
            { pw = it }, 
            label = { Text("Password") }, 
            singleLine = true,
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Password,
                imeAction = ImeAction.Done
            )
        )
        err?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        Button(
            onClick = {
                if (busy) return@Button
                busy = true
                scope.launch {
                    try {
                        onLogin(email, pw)
                        err = null
                    } catch (e: CancellationException) {
                        throw e // Don't swallow cancellation
                    } catch (e: Exception) {
                        err = e.message
                    } finally {
                        busy = false
                    }
                }
            }, 
            enabled = !busy
        ) { Text(if (busy) "…" else "Log in") }
        TextButton(onClick = onGoRegister) { Text("Register") }
    }
}

@Composable
fun CampaignsScreen(items: List<Campaign>, onOpen: (Campaign) -> Unit, onCreate: (String) -> Unit, onLogout: () -> Unit) {
    var newName by remember { mutableStateOf("") }
    Column(Modifier.fillMaxSize().padding(16.dp)) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Text("Campaigns", style = MaterialTheme.typography.headlineMedium)
            TextButton(onClick = onLogout) { Text("Logout") }
        }
        Spacer(Modifier.height(8.dp))
        Row {
            OutlinedTextField(newName, { newName = it }, label = { Text("New campaign") }, modifier = Modifier.weight(1f))
            Spacer(Modifier.width(8.dp))
            Button(onClick = { if (newName.isNotBlank()) { onCreate(newName); newName = "" } }) { Text("Add") }
        }
        Spacer(Modifier.height(16.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(items, key = { it.id }) { c ->
                ElevatedCard(Modifier.fillMaxWidth().clickable { onOpen(c) }) {
                    Column(Modifier.padding(16.dp)) {
                        Text(c.name, style = MaterialTheme.typography.titleMedium)
                        c.description?.let { Text(it, style = MaterialTheme.typography.bodySmall) }
                    }
                }
            }
        }
    }
}

@Composable
fun DiceScreen(onRoll: suspend (String) -> DiceRollResult) {
    var expr by remember { mutableStateOf("1d20") }
    var last by remember { mutableStateOf<DiceRollResult?>(null) }
    var err by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope() // Fix: lifecycle-aware scope

    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Dice", style = MaterialTheme.typography.headlineMedium)
        OutlinedTextField(
            expr, 
            { expr = it }, 
            label = { Text("Expression") }, 
            singleLine = true,
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Text,
                imeAction = ImeAction.Done
            )
        )
        Button(
            onClick = {
                if (busy) return@Button
                busy = true
                scope.launch {
                    try {
                        val result = onRoll(expr)
                        last = result
                        err = null
                    } catch (e: CancellationException) {
                        throw e // Don't swallow cancellation
                    } catch (e: Exception) {
                        err = e.message
                    } finally {
                        busy = false
                    }
                }
            },
            enabled = !busy
        ) { Text(if (busy) "…" else "Roll") }
        err?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        last?.let {
            ElevatedCard(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(16.dp)) {
                    Text(it.total.toString(), style = MaterialTheme.typography.displayMedium)
                    Text(it.expression, style = MaterialTheme.typography.bodySmall)
                }
            }
        }
    }
}
