-- Missing indexes for performance

-- Combatants FK lookups
CREATE INDEX IF NOT EXISTS idx_combatants_character_id ON combatants(character_id);
CREATE INDEX IF NOT EXISTS idx_combatants_npc_id ON combatants(npc_id);

-- Messages soft-delete filter
CREATE INDEX IF NOT EXISTS idx_messages_deleted_at ON messages(deleted_at) WHERE deleted_at IS NULL;

-- Combat events by actor/target
CREATE INDEX IF NOT EXISTS idx_combat_events_actor ON combat_events(actor_combatant);
CREATE INDEX IF NOT EXISTS idx_combat_events_target ON combat_events(target_combatant);

-- Composite indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_characters_campaign_owner ON characters(campaign_id, owner_id);
CREATE INDEX IF NOT EXISTS idx_encounters_campaign_status ON encounters(campaign_id, status);
CREATE INDEX IF NOT EXISTS idx_messages_campaign_scope_created ON messages(campaign_id, scope, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dice_rolls_campaign_user_rolled ON dice_rolls(campaign_id, user_id, rolled_at DESC);

-- Spell name search (trigram GIN for ilike '%query%')
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX IF NOT EXISTS idx_spells_name_trgm ON spells USING gin(name gin_trgm_ops);

-- CHECK >= 0 constraints for data integrity
ALTER TABLE combatants
    ADD CONSTRAINT chk_combatants_hp_current_nonneg CHECK (hp_current >= 0),
    ADD CONSTRAINT chk_combatants_hp_max_nonneg CHECK (hp_max >= 0),
    ADD CONSTRAINT chk_combatants_temp_hp_nonneg CHECK (temp_hp >= 0),
    ADD CONSTRAINT chk_combatants_ac_nonneg CHECK (ac >= 0),
    ADD CONSTRAINT chk_combatants_movement_nonneg CHECK (movement_used_ft >= 0),
    ADD CONSTRAINT chk_combatants_legendary_actions_max_nonneg CHECK (legendary_actions_max >= 0),
    ADD CONSTRAINT chk_combatants_legendary_actions_used_nonneg CHECK (legendary_actions_used >= 0),
    ADD CONSTRAINT chk_combatants_legendary_resistances_max_nonneg CHECK (legendary_resistances_max >= 0),
    ADD CONSTRAINT chk_combatants_legendary_resistances_used_nonneg CHECK (legendary_resistances_used >= 0);

ALTER TABLE parties
    ADD CONSTRAINT chk_parties_cp_nonneg CHECK (cp >= 0),
    ADD CONSTRAINT chk_parties_sp_nonneg CHECK (sp >= 0),
    ADD CONSTRAINT chk_parties_ep_nonneg CHECK (ep >= 0),
    ADD CONSTRAINT chk_parties_gp_nonneg CHECK (gp >= 0),
    ADD CONSTRAINT chk_parties_pp_nonneg CHECK (pp >= 0);

ALTER TABLE encounters
    ADD CONSTRAINT chk_encounters_round_nonneg CHECK (round >= 0),
    ADD CONSTRAINT chk_encounters_turn_index_nonneg CHECK (turn_index >= 0);
