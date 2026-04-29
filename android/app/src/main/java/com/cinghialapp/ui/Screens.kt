package com.cinghialapp.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.cinghialapp.model.Campaign
import com.cinghialapp.model.DiceRollResult
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.SupervisorJob

@Composable
fun LoginScreen(onLogin: suspend (String, String) -> Unit, onGoRegister: () -> Unit) {
    var email by remember { mutableStateOf("") }
    var pw by remember { mutableStateOf("") }
    var err by remember { mutableStateOf<String?>(null) }
    var busy by remember { mutableStateOf(false) }

    Column(Modifier.padding(24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Log in", style = MaterialTheme.typography.headlineMedium)
        OutlinedTextField(email, { email = it }, label = { Text("Email") }, singleLine = true)
        OutlinedTextField(pw, { pw = it }, label = { Text("Password") }, singleLine = true)
        err?.let { Text(it, color = MaterialTheme.colorScheme.error) }
        Button(onClick = {
            busy = true
            uiScope.launch {
                runCatching { onLogin(email, pw) }.onFailure { err = it.message }
                busy = false
            }
        }, enabled = !busy) { Text(if (busy) "…" else "Log in") }
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

    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Dice", style = MaterialTheme.typography.headlineMedium)
        OutlinedTextField(expr, { expr = it }, label = { Text("Expression") }, singleLine = true)
        Button(onClick = {
            uiScope.launch {
                runCatching { onRoll(expr) }.onSuccess { last = it; err = null }.onFailure { err = it.message }
            }
        }) { Text("Roll") }
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

private val uiScope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
