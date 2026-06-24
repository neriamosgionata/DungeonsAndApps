export type Language = 'en' | 'it';
export type UserRole = 'user' | 'admin';
export type MembershipRole = 'player' | 'master';
export type Visibility = 'master' | 'players';

export interface User {
  id: string;
  email: string;
  display_name: string;
  role: UserRole;
  language: Language;
  avatar_url?: string | null;
  token_version?: number;
  created_at: string;
}

export interface Campaign {
  id: string;
  name: string;
  description?: string | null;
  master_id: string;
  icon_url?: string | null;
  leveling?: 'xp' | 'milestone';
  created_at: string;
}

export interface Member {
  user_id: string;
  display_name: string;
  email: string;
  role: MembershipRole;
  character_limit: number;
}

export interface Spell {
  slug: string;
  name: string;
  level: number;
  school: string;
  classes: string[];
  ritual: boolean;
  concentration: boolean;
  description: string;
  casting_time?: string | null;
  range_text?: string | null;
  components?: string | null;
  duration?: string | null;
  higher_levels?: string | null;
  source: string;
  effects?: SpellEffectTemplate[];
  aoe?: SpellAoe;
}

export interface DiceRollTerm {
  expr: string;
  kind: 'dice' | 'modifier';
  rolls: number[];
  kept: number[];
  value: number;
}

export interface DiceRollResult {
  id: string;
  expression: string;
  total: number;
  terms: DiceRollTerm[];
  label?: string | null;
}

export interface DiceHistory {
  id: string;
  user_id: string;
  character_id?: string | null;
  expression: string;
  label?: string | null;
  results: Record<string, unknown>;
  total: number;
  private: boolean;
  rolled_at: string;
}

export interface MessageRollResult {
  expression: string;
  total: number;
  terms: DiceRollTerm[];
}

export interface MessageReaction {
  emoji: string;
  count: number;
  user_ids: string[];
}

export interface Message {
  id: string;
  campaign_id: string;
  sender_id: string;
  recipient_id?: string | null;
  scope: 'campaign' | 'whisper';
  body: string;
  roll_result?: MessageRollResult | null;
  created_at: string;
  edited_at?: string | null;
  deleted_at?: string | null;
}

export interface Notification {
  id: string;
  user_id: string;
  campaign_id?: string | null;
  kind: string;
  title: string;
  body?: string | null;
  ref_kind?: string | null;
  ref_id?: string | null;
  read_at?: string | null;
  created_at: string;
}

export interface Character {
  id: string;
  campaign_id: string;
  owner_id: string;
  name: string;
  race?: string | null;
  level_total: number;
  sheet: Record<string, unknown>;
  portrait_url?: string | null;
  clear_portrait?: boolean;
  created_at: string;
  updated_at: string;
}

export interface NPC {
  id: string;
  campaign_id: string;
  name: string;
  role?: string | null;
  faction_id?: string | null;
  description?: string | null;
  stats: Record<string, unknown>;
  image_key?: string | null;
  visibility: Visibility;
  created_at: string;
  updated_at: string;
}

export interface Faction {
  id: string;
  campaign_id: string;
  name: string;
  banner_color?: string | null;
  description?: string | null;
  attitude?: string | null;
  visibility: Visibility;
  created_at: string;
  updated_at: string;
}

export interface LoreEntry {
  id: string;
  campaign_id: string;
  title: string;
  category?: string | null;
  body: string;
  visibility: Visibility;
  created_at: string;
  updated_at: string;
}

export interface NewsEntry {
  id: string;
  campaign_id: string;
  title: string;
  body: string;
  published_at: string;
  visibility: Visibility;
  created_at: string;
  updated_at: string;
}

export interface Quest {
  id: string;
  campaign_id: string;
  title: string;
  description?: string | null;
  status: 'active' | 'completed' | 'failed' | 'abandoned';
  reward?: string | null;
  visibility: Visibility;
  npcs?: Array<{ npc_id: string; name: string; role?: string | null }>;
  created_at: string;
  updated_at: string;
}

export interface Map {
  id: string;
  campaign_id: string;
  name: string;
  description?: string | null;
  image_key?: string | null;
  width?: number | null;
  height?: number | null;
  visibility: Visibility;
  created_at: string;
  updated_at: string;
}

export interface MapPin {
  id: string;
  map_id: string;
  label: string;
  kind: string;
  faction_id?: string | null;
  is_party: boolean;
  x: number;
  y: number;
  color?: string | null;
  note?: string | null;
  icon_url?: string | null;
  visibility: Visibility;
  created_at: string;
  updated_at: string;
}

export interface PartyData {
  id: string;
  campaign_id: string;
  cp: number;
  sp: number;
  ep: number;
  gp: number;
  pp: number;
  shared_notes?: string | null;
  updated_at: string;
}

export interface LootItem {
  id: string;
  party_id: string;
  name: string;
  description?: string | null;
  quantity: number;
  value_gp?: string | null;
  claimed_by?: string | null;
  created_at: string;
}

export interface CampaignSession {
  id: string;
  campaign_id: string;
  title: string;
  session_number?: number | null;
  played_at?: string | null;
  status: 'planned' | 'played' | 'cancelled';
  recap?: string | null;
  visibility: Visibility;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface Encounter {
  id: string;
  campaign_id: string;
  name: string;
  status: 'planned' | 'active' | 'ended';
  round: number;
  turn_index: number;
  notes?: string | null;
  map_image?: string | null;
  clear_map_image?: boolean;
  map_grid_size: number;
  show_grid: boolean;
  grid_type: string;
  lair_action_used: boolean;
  updated_at: string;
}

export interface Combatant {
  id: string;
  encounter_id: string;
  ref_type: 'character' | 'npc';
  character_id?: string | null;
  npc_id?: string | null;
  display_name: string;
  initiative: number;
  dex_tiebreaker: number;
  hp_current: number;
  hp_max: number;
  temp_hp: number;
  ac: number;
  conditions: string[];
  notes?: string | null;
  is_visible: boolean;
  turn_order: number;
  initiative_rolled: boolean;
  token_x?: number | null;
  token_y?: number | null;
  token_color?: string | null;
  token_on_map: boolean;
  token_image?: string | null;
  clear_token_image?: boolean;
  portrait_url?: string | null;
  token_moved_round?: number | null;
  action_used: boolean;
  bonus_action_used: boolean;
  reaction_used: boolean;
  movement_used_ft: number;
  legendary_actions_max: number;
  legendary_actions_used: number;
  legendary_resistances_max: number;
  legendary_resistances_used: number;
  readied_action?: { trigger: string; action: string; target_id?: string | null } | null;
  delayed_turn?: boolean | null;
  cover_bonus?: number | null;
  action_spell_level?: number;
  bonus_action_spell_level?: number;
  last_hit_attack_total?: number | null;
  last_hit_damage?: number | null;
  // last_hit_attacker column was dropped in migration 2026-06-17
  // (see migration 20260617000001_combatant_faction_and_drop_last_hit_attacker).
  // The Shield reaction now reads pending_hits[] JSONB queue.
  spell_being_cast?: string | null;
}

export interface CombatantEffect {
  id: string;
  combatant_id: string;
  name: string;
  kind: 'buff' | 'debuff' | 'neutral' | 'condition';
  icon: string;
  duration_unit: 'rounds' | 'minutes' | 'hours' | 'permanent';
  duration_value: number | null;
  remaining: number | null;
  tick_trigger: string;
  concentration: boolean;
  caster_combatant_id?: string | null;
  source_type?: string | null;
  source_name?: string | null;
  source_spell_slug?: string | null;
  modifiers: Record<string, unknown>;
  active: boolean;
  applied_at_round: number;
  applied_at_turn_index: number;
  created_at: string;
}

export interface EffectMovement {
  type: 'push' | 'pull' | 'teleport' | 'forced_move' | 'dash_bonus';
  distance_ft: number;
  direction?: 'away_from_caster' | 'toward_caster' | 'chosen' | 'random';
}

export interface SpellAoe {
  shape: 'circle' | 'cone' | 'line' | 'cube';
  radius_ft?: number;
  length_ft?: number;
  width_ft?: number;
  color: string;
}

export interface SpellEffectTemplate {
  name: string;
  kind: 'buff' | 'debuff' | 'neutral' | 'condition';
  icon: string;
  duration_unit: 'rounds' | 'minutes' | 'hours' | 'permanent';
  duration_value: number | null;
  tick_trigger: string;
  modifiers?: Record<string, unknown>;
}

export interface EncounterOverlay {
  id: string;
  encounter_id: string;
  kind: 'aoe' | 'zone';
  shape: 'circle' | 'cone' | 'line' | 'cube' | 'polygon';
  origin_x: number;
  origin_y: number;
  end_x?: number | null;
  end_y?: number | null;
  radius_ft?: number | null;
  length_ft?: number | null;
  width_ft?: number | null;
  angle_deg?: number | null;
  points?: Array<{ x: number; y: number }> | null;
  color: string;
  label?: string | null;
  zone_type?: string | null;
  active: boolean;
  expires_at_round?: number | null;
  expires_at_turn?: number | null;
  source_spell_slug?: string | null;
  created_by_combatant_id?: string | null;
  created_at: string;
  hazard_damage_expression?: string | null;
  hazard_damage_type?: string | null;
  hazard_save_ability?: string | null;
  hazard_save_dc?: number | null;
  hazard_half_on_save?: boolean;
}

export interface AttackResult {
  hit: boolean;
  critical: boolean;
  natural_roll: number;
  attack_total: number;
  target_ac: number;
  attack_roll: { expression: string; terms: Array<{ expr: string; kind: string; rolls: number[]; kept: number[]; value: number }>; total: number };
  damage_roll?: { expression: string; terms: Array<{ expr: string; kind: string; rolls: number[]; kept: number[]; value: number }>; total: number } | null;
  damage_base: number;
  damage_applied: number;
  extra_damage_applied: number;
  extra_damage_type?: string | null;
  target_hp_before: number;
  target_hp_after: number;
  target_temp_hp_after: number;
  concentration_broken: boolean;
  concentration_roll?: { expression: string; terms: Array<{ expr: string; kind: string; rolls: number[]; kept: number[]; value: number }>; total: number } | null;
  combat_event_id?: string | null;
  cover_bonus: number;
  attack_advantage: boolean;
  attack_disadvantage: boolean;
  damage_resisted: boolean;
  damage_vulnerable: boolean;
  damage_immune: boolean;
  reach_weapon: boolean;
  needs_ammo: boolean;
  instant_death?: boolean;
}

export interface DamageResult {
  damage_raw: number;
  damage_applied: number;
  hp_before: number;
  hp_after: number;
  temp_hp_after: number;
  concentration_broken: boolean;
  concentration_roll?: { expression: string; terms: Array<{ expr: string; kind: string; rolls: number[]; kept: number[]; value: number }>; total: number } | null;
  combat_event_id?: string | null;
  damage_resisted: boolean;
  damage_vulnerable: boolean;
  damage_immune: boolean;
  instant_death?: boolean;
  // L-F10: 'heal' for heal results (healing is just negative damage in
  // the existing data model, but the kind lets the UI distinguish and
  // skip the resistance/vulnerability/concentration checks that don't
  // apply to healing).
  kind?: 'damage' | 'heal';
}

export interface SaveResult {
  passed: boolean;
  natural_roll: number;
  save_total: number;
  dc: number;
  save_roll: { expression: string; terms: Array<{ expr: string; kind: string; rolls: number[]; kept: number[]; value: number }>; total: number };
  save_advantage: boolean;
  save_disadvantage: boolean;
}

export interface GrappleResult {
  success: boolean;
  attacker_roll: number;
  attacker_total: number;
  defender_roll: number;
  defender_total: number;
  grapple_applied: boolean;
}

export interface ShoveResult {
  success: boolean;
  attacker_total: number;
  defender_total: number;
  knocked_prone: boolean;
  pushed_away: boolean;
}

export interface GrappleEscapeResult {
  success: boolean;
  escapee_roll: number;
  escapee_total: number;
  grappler_roll: number;
  grappler_total: number;
  escaped: boolean;
}

export interface MultiAttackResult {
  results: AttackResult[];
  targets_hit: number;
  total_damage: number;
}

export interface OverlayDamageResult {
  overlay_id: string;
  targets_affected: Array<{
    target_id: string;
    target_name: string;
    in_area: boolean;
    save_passed: boolean | null;
    damage_applied: number;
    hp_after: number;
  }>;
}

export interface AwardXpResult {
  characters_awarded: Array<{
    character_id: string;
    character_name: string;
    xp_before: number;
    xp_after: number;
    xp_gained: number;
    leveled_up: boolean;
    new_level: number;
  }>;
}

export interface ClassFeatureResult {
  feature: string;
  success: boolean;
  message: string;
  hp_after?: number | null;
  effect_applied: boolean;
}

export interface FlankPair {
  attacker_a_id: string;
  attacker_a_name: string;
  attacker_b_id: string;
  attacker_b_name: string;
  target_id: string;
  target_name: string;
}

export interface CoverResult {
  attacker_id: string;
  target_id: string;
  cover_type: string;
  cover_bonus: number;
  blockers: string[];
}

export interface ComputedStats {
  ac: number;
  speed: number;
  initiative_bonus: number;
  attack_bonus: number;
  spell_attack_bonus: number;
  spell_save_dc: number;
  save_mods: Array<[string, number]>;
  skill_mods: Array<[string, number]>;
  passive_scores: Array<[string, number]>;
  exhaustion: number;
  resistances: string[];
  vulnerabilities: string[];
  immunities: string[];
  attack_advantage: boolean;
  attack_disadvantage: boolean;
  save_advantage: boolean;
  save_disadvantage: boolean;
  speed_halved: boolean;
  speed_doubled: boolean;
  incapacitated: boolean;
  invisible: boolean;
  frightened: boolean;
  paralyzed: boolean;
  restrained: boolean;
  prone: boolean;
  blinded: boolean;
  deafened: boolean;
  charmed: boolean;
  poisoned: boolean;
  stunned: boolean;
  unconscious: boolean;
  grappling: boolean;
  grappled: boolean;
  concentration: boolean;
  hover: boolean;
  flying_speed: number;
  swim_speed: number;
  climb_speed: number;
  burrow_speed: number;
  damage_bonus: number;
  weapon_damage_bonus: number;
  hp_regen_per_turn: number;
  temp_hp_per_turn: number;
  darkvision_range: number;
  truesight_range: number;
  blindsight_range: number;
  tremorsense_range: number;
}

export interface HealResult {
  amount: number;
  hp_before: number;
  hp_after: number;
  temp_hp_after: number;
  stabilized: boolean;
  revived: boolean;
}

export interface DeathSaveResult {
  natural_roll: number;
  passed: boolean;
  successes_before: number;
  failures_before: number;
  successes_after: number;
  failures_after: number;
  stabilized: boolean;
  died: boolean;
  nat20: boolean;
  nat1: boolean;
  hp_after: number;
  alive: boolean;
}

export interface SkillCheckResult {
  skill: string;
  natural_roll: number;
  total: number;
  dc: number | null;
  passed: boolean | null;
  advantage: boolean;
  disadvantage: boolean;
}

export interface ShortRestResult {
  hp_before: number;
  hp_after: number;
  hp_max: number;
  hit_dice_before: number;
  hit_dice_after: number;
  roll_total: number;
  con_mod: number;
}

export interface LongRestResult {
  hp_before: number;
  hp_after: number;
  hit_dice_before: number;
  hit_dice_after: number;
  hit_dice_max: number;
  exhaustion_before: number;
  exhaustion_after: number;
}

export interface Invitation {
  id: string;
  campaign_id: string;
  user_id: string;
  email?: string;
  role: MembershipRole;
  message: string | null;
  created_at: string;
  accepted: boolean | null;
  campaign_name: string;
  inviter_name: string | null;
}
