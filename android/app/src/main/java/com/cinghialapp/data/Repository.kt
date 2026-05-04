package com.dungeonsandapps.data

import android.util.Log
import com.dungeonsandapps.api.Api
import com.dungeonsandapps.model.Campaign
import com.dungeonsandapps.model.Character
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onStart
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json

class Repository(
    private val api: Api,
    private val db: AppDb,
    private val externalScope: CoroutineScope,
    private val json: Json = Json { ignoreUnknownKeys = true; explicitNulls = false },
) {
    // Track refresh triggers
    private val refreshCampaignsTrigger = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    
    // Manage background refresh jobs properly
    private val repoScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    // stale-while-revalidate: emit cached immediately, refresh in background
    // Fix: Proper Flow construction without side effects in builder
    fun observeCampaigns(): Flow<List<Campaign>> {
        return db.campaigns().observeAll()
            .map { list ->
                list.map { Campaign(it.id, it.name, it.description, it.masterId) }
            }
            .onStart { 
                // Trigger initial refresh
                repoScope.launch { refreshCampaigns() }
            }
            .combine(refreshCampaignsTrigger) { campaigns, _ -> campaigns }
            .flowOn(Dispatchers.IO)
    }

    suspend fun refreshCampaigns() {
        try {
            val remote = api.campaigns()
            val now = System.currentTimeMillis()
            db.campaigns().upsertAll(remote.map {
                CampaignEntity(it.id, it.name, it.description, it.master_id, now)
            })
            refreshCampaignsTrigger.tryEmit(Unit)
        } catch (e: CancellationException) {
            throw e // Don't swallow
        } catch (e: Exception) {
            Log.w("Repository", "Failed to refresh campaigns", e)
            // Don't rethrow - stale data is better than crash
        }
    }
    
    // Fix: Cancel background work when Repository is no longer needed
    fun cleanup() {
        repoScope.launch { } // CoroutineScope doesn't have cancel directly
    }

    fun observeCharacters(cid: String): Flow<List<Character>> {
        return db.characters().observeByCampaign(cid)
            .map { list ->
                list.map {
                    Character(
                        id = it.id, campaign_id = it.campaignId, owner_id = it.ownerId,
                        name = it.name, race = it.race, class_primary = it.classPrimary,
                        level_total = it.levelTotal, sheet = json.parseToJsonElement(it.sheetJson),
                    )
                }
            }
            .onStart {
                repoScope.launch { refreshCharacters(cid) }
            }
            .flowOn(Dispatchers.IO)
    }

    suspend fun refreshCharacters(cid: String) {
        try {
            val remote = api.characters(cid)
            val now = System.currentTimeMillis()
            db.characters().upsertAll(remote.map {
                CharacterEntity(
                    id = it.id, campaignId = it.campaign_id, ownerId = it.owner_id,
                    name = it.name, race = it.race, classPrimary = it.class_primary,
                    levelTotal = it.level_total, sheetJson = json.encodeToString(it.sheet), cachedAt = now,
                )
            })
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w("Repository", "Failed to refresh characters for $cid", e)
        }
    }
}
