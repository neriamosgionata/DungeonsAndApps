-- Composite index for list_combatants ordering and initiative notification filter
CREATE INDEX IF NOT EXISTS idx_combatants_encounter_rolled_turn
    ON combatants(encounter_id, initiative_rolled, turn_order);
