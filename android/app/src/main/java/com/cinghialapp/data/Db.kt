package com.cinghialapp.data

import androidx.room.*
import kotlinx.coroutines.flow.Flow

@Entity(tableName = "campaigns")
data class CampaignEntity(
    @PrimaryKey val id: String,
    val name: String,
    val description: String?,
    val masterId: String,
    val cachedAt: Long
)

@Entity(tableName = "characters")
data class CharacterEntity(
    @PrimaryKey val id: String,
    val campaignId: String,
    val ownerId: String,
    val name: String,
    val race: String?,
    val classPrimary: String?,
    val levelTotal: Int,
    val sheetJson: String,
    val cachedAt: Long
)

@Dao
interface CampaignDao {
    @Query("select * from campaigns order by cachedAt desc")
    fun observeAll(): Flow<List<CampaignEntity>>
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertAll(items: List<CampaignEntity>)
    @Query("delete from campaigns") suspend fun clear()
}

@Dao
interface CharacterDao {
    @Query("select * from characters where campaignId = :cid order by name")
    fun observeByCampaign(cid: String): Flow<List<CharacterEntity>>
    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertAll(items: List<CharacterEntity>)
    @Query("delete from characters where campaignId = :cid") suspend fun clearCampaign(cid: String)
}

@Database(entities = [CampaignEntity::class, CharacterEntity::class], version = 1)
abstract class AppDb : RoomDatabase() {
    abstract fun campaigns(): CampaignDao
    abstract fun characters(): CharacterDao
}
