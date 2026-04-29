package com.cinghialapp.data

import com.cinghialapp.api.Api
import com.cinghialapp.model.Campaign
import com.cinghialapp.model.Character
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json

class Repository(
    private val api: Api,
    private val db: AppDb,
    private val scope: CoroutineScope,
    private val json: Json = Json { ignoreUnknownKeys = true; explicitNulls = false },
) {
    // stale-while-revalidate: emit cached immediately, refresh in background
    fun observeCampaigns(): Flow<List<Campaign>> {
        scope.launch(Dispatchers.IO) { runCatching { refreshCampaigns() } }
        return db.campaigns().observeAll().map { list ->
            list.map { Campaign(it.id, it.name, it.description, it.masterId) }
        }
    }

    suspend fun refreshCampaigns() {
        val remote = api.campaigns()
        val now = System.currentTimeMillis()
        db.campaigns().upsertAll(remote.map {
            CampaignEntity(it.id, it.name, it.description, it.master_id, now)
        })
    }

    fun observeCharacters(cid: String): Flow<List<Character>> {
        scope.launch(Dispatchers.IO) { runCatching { refreshCharacters(cid) } }
        return db.characters().observeByCampaign(cid).map { list ->
            list.map {
                Character(
                    id = it.id, campaign_id = it.campaignId, owner_id = it.ownerId,
                    name = it.name, race = it.race, class_primary = it.classPrimary,
                    level_total = it.levelTotal, sheet = json.parseToJsonElement(it.sheetJson),
                )
            }
        }
    }

    suspend fun refreshCharacters(cid: String) {
        val remote = api.characters(cid)
        val now = System.currentTimeMillis()
        db.characters().upsertAll(remote.map {
            CharacterEntity(
                id = it.id, campaignId = it.campaign_id, ownerId = it.owner_id,
                name = it.name, race = it.race, classPrimary = it.class_primary,
                levelTotal = it.level_total, sheetJson = json.encodeToString(it.sheet), cachedAt = now,
            )
        })
    }
}
