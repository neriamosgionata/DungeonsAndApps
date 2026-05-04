package com.dungeonsandapps

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.room.Room
import com.dungeonsandapps.api.Api
import com.dungeonsandapps.api.createHttpClient
import com.dungeonsandapps.data.AppDb
import com.dungeonsandapps.data.AuthStore
import com.dungeonsandapps.data.Repository
import com.dungeonsandapps.ui.CampaignsScreen
import com.dungeonsandapps.ui.DiceScreen
import com.dungeonsandapps.ui.LoginScreen
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import androidx.compose.runtime.collectAsState
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val authStore = AuthStore(applicationContext)
        val appScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        val client = createHttpClient(tokenProvider = { runBlocking { authStore.currentToken() } })
        val api = Api(client)
        val db = Room.databaseBuilder(applicationContext, AppDb::class.java, "dungeonsandapps.db").build()
        val repo = Repository(api, db, appScope)
        val json = Json { ignoreUnknownKeys = true; explicitNulls = false }

        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    val nav = rememberNavController()
                    val token by authStore.token.collectAsState(initial = null)
                    val start = if (token == null) "login" else "campaigns"

                    NavHost(nav, startDestination = start) {
                        composable("login") {
                            LoginScreen(
                                onLogin = { email, pw ->
                                    val res = api.login(email, pw)
                                    authStore.set(res.token, json.encodeToString(res.user))
                                    nav.navigate("campaigns") { popUpTo("login") { inclusive = true } }
                                },
                                onGoRegister = { nav.navigate("register") }
                            )
                        }
                        composable("register") {
                            LoginScreen(
                                onLogin = { email, pw ->
                                    val res = api.register(email, pw, email.substringBefore("@"))
                                    authStore.set(res.token, json.encodeToString(res.user))
                                    nav.navigate("campaigns") { popUpTo("login") { inclusive = true } }
                                },
                                onGoRegister = { nav.popBackStack() }
                            )
                        }
                        composable("campaigns") {
                            val items by repo.observeCampaigns().collectAsState(initial = emptyList())
                            CampaignsScreen(
                                items = items,
                                onOpen = { nav.navigate("campaign/${it.id}") },
                                onCreate = { name -> appScope.launch { api.createCampaign(name); repo.refreshCampaigns() } },
                                onLogout = { appScope.launch { authStore.clear() }; nav.navigate("login") { popUpTo(0) } }
                            )
                        }
                        composable("campaign/{cid}") { back ->
                            val cid = back.arguments?.getString("cid") ?: return@composable
                            DiceScreen(onRoll = { expr -> api.rollDice(cid, expr) })
                        }
                    }
                }
            }
        }
    }
}
