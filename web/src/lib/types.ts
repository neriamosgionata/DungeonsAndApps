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

export interface Message {
  id: string;
  campaign_id: string;
  sender_id: string;
  recipient_id?: string | null;
  scope: 'campaign' | 'whisper';
  body: string;
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

export interface SpellEffectTemplate {
  name: string;
  kind: 'buff' | 'debuff' | 'neutral' | 'condition';
  icon: string;
  duration_unit: 'rounds' | 'minutes' | 'hours' | 'permanent';
  duration_value: number | null;
  tick_trigger: string;
  modifiers?: Record<string, unknown>;
}

export interface Invitation {
  id: string;
  campaign_id: string;
  user_id: string;
  role: MembershipRole;
  message: string | null;
  created_at: string;
  accepted: boolean | null;
  campaign_name: string;
  inviter_name: string | null;
}
