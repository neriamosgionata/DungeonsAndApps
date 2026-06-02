import { api } from './client';
import { auth } from '$lib/stores/auth.svelte';
import type {
  User, Campaign, Spell, DiceRollResult, DiceHistory,
  Character, NPC, Faction, LoreEntry, NewsEntry, Quest,
  Map, MapPin, PartyData, LootItem, CampaignSession, Encounter,
  Combatant, Message, Notification, Invitation, Member,
  CombatantEffect, EncounterOverlay, AttackResult, DamageResult, SaveResult, ComputedStats,
  GrappleResult, ShoveResult, FlankPair, CoverResult
} from '$lib/types';

function tok() { return auth.token ?? undefined; }

export type UserRow = { id: string; email: string; display_name: string; role: string; language: string; created_at: string };

export const Users = {
  list: () => api<UserRow[]>('/users', {}, tok()),
  create: (body: { email: string; password: string; display_name: string; role?: string; language?: string }) =>
    api<UserRow>('/users', { method: 'POST', body: JSON.stringify(body) }, tok()),
  update: (id: string, patch: { display_name?: string; role?: 'user' | 'admin'; language?: 'en' | 'it' }) =>
    api<UserRow>(`/users/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/users/${id}`, { method: 'DELETE' }, tok()),
  resetPassword: (id: string, new_password: string) =>
    api<void>(`/users/${id}/reset-password`, { method: 'POST', body: JSON.stringify({ new_password }) }, tok()),
};

export interface BackupData {
  version: number;
  exported_at: string;
  tables: Record<string, unknown[]>;
}

export const Admin = {
  stats: () => api<{ users: number; campaigns: number; characters: number; messages: number; encounters: number; spells: number }>('/admin/stats', {}, tok()),
  campaigns: () => api<Array<{ id: string; name: string; owner_name: string; member_count: number; created_at: string }>>('/admin/campaigns', {}, tok()),
  deleteCampaign: (id: string) => api<void>(`/admin/campaigns/${id}`, { method: 'DELETE' }, tok()),
  backup: () => api<BackupData>('/admin/backup', {}, tok()),
  restore: (backup: BackupData) => api<void>('/admin/restore', { method: 'POST', body: JSON.stringify({ backup }) }, tok()),
};

export const Invitations = {
  mine: () => api<Invitation[]>('/invitations', {}, tok()),
  create: (cid: string, email: string, role: 'player' | 'master' = 'player', message?: string) =>
    api(`/campaigns/${cid}/invitations`, { method: 'POST',
      body: JSON.stringify({ email, role, message }) }, tok()),
  forCampaign: (cid: string) => api<Invitation[]>(`/campaigns/${cid}/invitations`, {}, tok()),
  accept: (id: string) =>
    api<void>(`/invitations/${id}/accept`, { method: 'POST' }, tok()),
  decline: (id: string) =>
    api<void>(`/invitations/${id}/decline`, { method: 'POST' }, tok()),
  revoke: (id: string) =>
    api<void>(`/invitations/${id}`, { method: 'DELETE' }, tok()),
};

export const Auth = {
  bootstrapStatus: () => api<{ needs_bootstrap: boolean }>('/auth/bootstrap'),
  register: (email: string, password: string, display_name: string, language = 'en') =>
    api<{ token: string; user: User }>('/auth/register', {
      method: 'POST',
      body: JSON.stringify({ email, password, display_name, language }),
    }, tok()),
  login: (email: string, password: string) =>
    api<{ token: string; user: User }>('/auth/login', {
      method: 'POST',
      body: JSON.stringify({ email, password }),
    }),
  logout: () => api<void>('/auth/logout', { method: 'POST' }, tok()),
  me: () => api<User>('/auth/me', {}, tok()),
  updateMe: (patch: { display_name?: string; language?: 'en' | 'it' }) =>
    api<User>('/users/me', { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  changePassword: (current_password: string, new_password: string) =>
    api<void>('/users/me/change-password', { method: 'POST', body: JSON.stringify({ current_password, new_password }) }, tok()),
};

export const Campaigns = {
  list: () => api<Campaign[]>('/campaigns', {}, tok()),
  create: (name: string, description?: string, icon_url?: string | null) =>
    api<Campaign>('/campaigns', {
      method: 'POST',
      body: JSON.stringify({ name, description, icon_url }),
    }, tok()),
  get: (id: string) => api<Campaign>(`/campaigns/${id}`, {}, tok()),
  update: (id: string, patch: Partial<Campaign>) =>
    api<Campaign>(`/campaigns/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/campaigns/${id}`, { method: 'DELETE' }, tok()),
  members: (id: string) => api<Member[]>(`/campaigns/${id}/members`, {}, tok()),
  addMember: (id: string, email: string, role: 'player' | 'master') =>
    api(`/campaigns/${id}/members`, { method: 'POST', body: JSON.stringify({ email, role }) }, tok()),
  updateMember: (id: string, userId: string, patch: { character_limit?: number; role?: 'player' | 'master' }) =>
    api(`/campaigns/${id}/members/${userId}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  awardXp: (id: string, body: { character_ids: string[]; xp_each: number; reason?: string }) =>
    api<import('$lib/types').AwardXpResult>(`/campaigns/${id}/award-xp`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  removeMember: (id: string, userId: string) =>
    api<void>(`/campaigns/${id}/members/${userId}`, { method: 'DELETE' }, tok()),
  presence: (id: string) => api<string[]>(`/campaigns/${id}/presence`, {}, tok()),
};

export const Characters = {
  list: (campaign: string) => api<Character[]>(`/campaigns/${campaign}/characters`, {}, tok()),
  create: (campaign: string, body: Partial<Character>) =>
    api<Character>(`/campaigns/${campaign}/characters`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  get: (id: string) => api<Character>(`/characters/${id}`, {}, tok()),
  update: (id: string, patch: Partial<Character>) =>
    api<Character>(`/characters/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/characters/${id}`, { method: 'DELETE' }, tok()),
  shortRest: (id: string, hitDiceSpent: number) => api<import('$lib/types').ShortRestResult>(`/characters/${id}/short-rest`, { method: 'POST', body: JSON.stringify({ hit_dice_spent: hitDiceSpent }) }, tok()),
  longRest: (id: string) => api<import('$lib/types').LongRestResult>(`/characters/${id}/long-rest`, { method: 'POST' }, tok()),
  spells: {
    list: (id: string) => api<Array<{ spell_id: string; name: string; slug: string; level: number; prepared: boolean; notes: string | null }>>(`/characters/${id}/spells`, {}, tok()),
    add: (id: string, body: { spell_id: string; prepared?: boolean; notes?: string }) =>
      api<void>(`/characters/${id}/spells`, { method: 'POST', body: JSON.stringify(body) }, tok()),
    update: (id: string, spellId: string, patch: { prepared?: boolean; notes?: string | null }) =>
      api<void>(`/characters/${id}/spells/${spellId}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
    remove: (id: string, spellId: string) => api<void>(`/characters/${id}/spells/${spellId}`, { method: 'DELETE' }, tok()),
  },
};

export const Sessions = {
  list: (cid: string) => api<CampaignSession[]>(`/campaigns/${cid}/sessions`, {}, tok()),
  create: (cid: string, body: Partial<CampaignSession>) =>
    api<CampaignSession>(`/campaigns/${cid}/sessions`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  update: (id: string, patch: Partial<CampaignSession>) =>
    api<CampaignSession>(`/sessions/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/sessions/${id}`, { method: 'DELETE' }, tok()),
};

export const Maps = {
  list: (cid: string) => api<Map[]>(`/campaigns/${cid}/maps`, {}, tok()),
  create: (cid: string, body: Partial<Map>) =>
    api<Map>(`/campaigns/${cid}/maps`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  update: (id: string, patch: Partial<Map>) =>
    api<Map>(`/maps/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/maps/${id}`, { method: 'DELETE' }, tok()),
  pins: {
    list: (mapId: string) => api<MapPin[]>(`/maps/${mapId}/pins`, {}, tok()),
    create: (mapId: string, body: Partial<MapPin>) =>
      api<MapPin>(`/maps/${mapId}/pins`, { method: 'POST', body: JSON.stringify(body) }, tok()),
    update: (id: string, patch: Partial<MapPin>) =>
      api<MapPin>(`/pins/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
    delete: (id: string) => api<void>(`/pins/${id}`, { method: 'DELETE' }, tok()),
  },
};

function crud<T>(col: string, itemPath: string) {
  return {
    list: (cid: string) => api<T[]>(`/campaigns/${cid}/${col}`, {}, tok()),
    create: (cid: string, body: Record<string, unknown>) =>
      api<T>(`/campaigns/${cid}/${col}`, { method: 'POST', body: JSON.stringify(body) }, tok()),
    get: (id: string) => api<T>(`/${itemPath}/${id}`, {}, tok()),
    update: (id: string, patch: Record<string, unknown>) =>
      api<T>(`/${itemPath}/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
    delete: (id: string) => api<void>(`/${itemPath}/${id}`, { method: 'DELETE' }, tok()),
  };
}

export const Factions = crud<Faction>('factions', 'factions');
export const NPCs     = crud<NPC>('npcs', 'npcs');
export const Lore     = crud<LoreEntry>('lore', 'lore');
export const News     = crud<NewsEntry>('news', 'news');
export const Quests   = {
  ...crud<Quest>('quests', 'quests'),
  linkNpc: (id: string, npcId: string, role?: string) =>
    api<void>(`/quests/${id}/npcs`, { method: 'POST', body: JSON.stringify({ npc_id: npcId, role }) }, tok()),
  unlinkNpc: (id: string, npcId: string) =>
    api<void>(`/quests/${id}/npcs/${npcId}`, { method: 'DELETE' }, tok()),
};

export const Parties = {
  get: (cid: string) => api<PartyData>(`/campaigns/${cid}/party`, {}, tok()),
  update: (cid: string, patch: Partial<PartyData>) =>
    api<PartyData>(`/campaigns/${cid}/party`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
};

export const Loot = {
  list: (cid: string) => api<LootItem[]>(`/campaigns/${cid}/loot`, {}, tok()),
  create: (cid: string, body: Partial<LootItem>) =>
    api<LootItem>(`/campaigns/${cid}/loot`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  update: (id: string, patch: Partial<LootItem>) =>
    api<LootItem>(`/loot/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/loot/${id}`, { method: 'DELETE' }, tok()),
};

export const Messages = {
  chat: (cid: string, limit = 100, offset = 0) =>
    api<Message[]>(`/campaigns/${cid}/messages?limit=${limit}&offset=${offset}`, {}, tok()),
  whispers: (cid: string, withUser?: string, limit = 100, offset = 0) =>
    api<Message[]>(`/campaigns/${cid}/messages?whispers=true${withUser ? `&with_user=${withUser}` : ''}&limit=${limit}&offset=${offset}`, {}, tok()),
  send: (cid: string, body: string, scope: 'campaign' | 'whisper', recipient_id?: string) =>
    api<Message>(`/campaigns/${cid}/messages`, { method: 'POST',
      body: JSON.stringify({ body, scope, recipient_id }) }, tok()),
  edit: (id: string, body: string) =>
    api<Message>(`/messages/${id}`, { method: 'PATCH', body: JSON.stringify({ body }) }, tok()),
  delete: (id: string) => api<void>(`/messages/${id}`, { method: 'DELETE' }, tok()),
};

export const Dice = {
  roll: (cid: string, expression: string, label?: string, isPrivate = false, character_id?: string) =>
    api<DiceRollResult>(`/campaigns/${cid}/dice`, { method: 'POST',
      body: JSON.stringify({ expression, label, private: isPrivate, character_id }) }, tok()),
  history: (cid: string, limit = 50, offset = 0) =>
    api<DiceHistory[]>(`/campaigns/${cid}/dice?limit=${limit}&offset=${offset}`, {}, tok()),
  clear: (cid: string) =>
    api<void>(`/campaigns/${cid}/dice`, { method: 'DELETE' }, tok()),
};

export const Spells = {
  list: (q?: { q?: string; level?: number; class?: string }) => {
    const p = new URLSearchParams();
    if (q?.q) p.set('q', q.q);
    if (q?.level !== undefined) p.set('level', String(q.level));
    if (q?.class) p.set('class', q.class);
    const qs = p.toString();
    return api<Spell[]>(`/spells${qs ? `?${qs}` : ''}`, {}, tok());
  },
  get: (slug: string) => api<Spell>(`/spells/${slug}`, {}, tok()),
};

export const Effects = {
  forEncounter: (eid: string) => api<CombatantEffect[]>(`/encounters/${eid}/effects`, {}, tok()),
  forCombatant: (cid: string) => api<CombatantEffect[]>(`/combatants/${cid}/effects`, {}, tok()),
  apply: (cid: string, body: Partial<CombatantEffect>) =>
    api<CombatantEffect>(`/combatants/${cid}/effects`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  applySpell: (cid: string, spellSlug: string, casterCombatantId?: string) =>
    api<CombatantEffect[]>(`/combatants/${cid}/effects/apply-spell`, { method: 'POST', body: JSON.stringify({ spell_slug: spellSlug, caster_combatant_id: casterCombatantId }) }, tok()),
  remove: (cid: string, eid: string) =>
    api<void>(`/combatants/${cid}/effects/${eid}`, { method: 'DELETE' }, tok()),
  update: (cid: string, eid: string, patch: { name?: string; active?: boolean; remaining?: number }) =>
    api<CombatantEffect>(`/combatants/${cid}/effects/${eid}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
};

export const Combatants = {
  useAction: (cid: string, action: 'action' | 'bonus_action' | 'reaction' | 'legendary_action' | 'legendary_resistance') =>
    api<Combatant>(`/combatants/${cid}/use-action`, { method: 'POST', body: JSON.stringify({ action }) }, tok()),
  attack: (cid: string, body: { target_id: string; attack_expression?: string; damage_expression?: string; damage_type: string; ability?: string; proficient?: boolean; advantage?: boolean; disadvantage?: boolean; cover?: string; is_spell_attack?: boolean; is_magical?: boolean; label?: string; weapon_id?: string; extra_damage_expression?: string; extra_damage_type?: string; power_attack?: boolean; skip_ammo?: boolean; reckless?: boolean; bless_dice?: number; bardic_inspiration_dice?: number }) =>
    api<AttackResult>(`/combatants/${cid}/attack`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  damage: (cid: string, body: { amount: number; damage_type: string; source_combatant_id?: string; label?: string; is_magical?: boolean }) =>
    api<DamageResult>(`/combatants/${cid}/damage`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  save: (cid: string, body: { ability: string; dc: number; advantage?: boolean; disadvantage?: boolean; label?: string }) =>
    api<SaveResult>(`/combatants/${cid}/save`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  computedStats: (cid: string) => api<ComputedStats>(`/combatants/${cid}/computed-stats`, {}, tok()),
  react: (cid: string, reaction_type: string, label?: string) =>
    api<Combatant>(`/combatants/${cid}/react`, { method: 'POST', body: JSON.stringify({ reaction_type, label }) }, tok()),
  castSpell: (cid: string, body: { spell_slug: string; target_ids: string[]; upcast_level?: number; damage_expression?: string; save_dc?: number; spell_attack_bonus?: number; half_on_save?: boolean; cast_as_ritual?: boolean; use_spell_attack?: boolean }) =>
    api<{ spell_name: string; spell_level: number; caster_id: string; slot_level_consumed: number; targets: Array<{ target_id: string; target_name: string; hit?: boolean | null; critical: boolean; save_passed?: boolean | null; save_total?: number | null; damage_applied: number; hp_after: number; temp_hp_after: number; effects_applied: string[]; concentration_broken: boolean }>; overlay_created?: string | null; concentration_required: boolean }>(`/combatants/${cid}/cast-spell`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  dodge: (cid: string) => api<Combatant>(`/combatants/${cid}/dodge`, { method: 'POST', body: JSON.stringify({}) }, tok()),
  disengage: (cid: string, useBonusAction?: boolean) => api<Combatant>(`/combatants/${cid}/disengage`, { method: 'POST', body: JSON.stringify({ use_bonus_action: useBonusAction ?? false }) }, tok()),
  help: (cid: string, target_id: string) => api<Combatant>(`/combatants/${cid}/help`, { method: 'POST', body: JSON.stringify({ target_id }) }, tok()),
  opportunityAttack: (cid: string, target_id: string) => api<import('$lib/types').AttackResult>(`/combatants/${cid}/opportunity-attack`, { method: 'POST', body: JSON.stringify({ target_id }) }, tok()),
  difficulty: (eid: string) => api<{ total_xp: number; adjusted_xp: number; difficulty: string; thresholds: { easy: number; medium: number; hard: number; deadly: number }; party_levels: number[]; monster_xp: [string, number, number][] }>(`/encounters/${eid}/difficulty`, {}, tok()),
  lairAction: (eid: string) => api<import('$lib/types').Encounter>(`/encounters/${eid}/lair-action`, { method: 'POST' }, tok()),
  legendaryAction: (cid: string) => api<{ legendary_actions_used: number; legendary_actions_max: number }>(`/combatants/${cid}/legendary-action`, { method: 'POST' }, tok()),
  ready: (cid: string, trigger: string, action: string, targetId?: string, triggerEvent?: string, watchTargetId?: string) => api<Combatant>(`/combatants/${cid}/ready`, { method: 'POST', body: JSON.stringify({ trigger, action, target_id: targetId, trigger_event: triggerEvent || undefined, watch_target_id: watchTargetId || undefined }) }, tok()),
  delay: (cid: string, insertAfter: number) => api<Combatant>(`/combatants/${cid}/delay`, { method: 'POST', body: JSON.stringify({ insert_after_turn_index: insertAfter }) }, tok()),
  grapple: (cid: string, targetId: string) => api<GrappleResult>(`/combatants/${cid}/grapple`, { method: 'POST', body: JSON.stringify({ target_id: targetId }) }, tok()),
  grappleEscape: (cid: string, grapplerId: string) => api<import('$lib/types').GrappleEscapeResult>(`/combatants/${cid}/grapple-escape`, { method: 'POST', body: JSON.stringify({ grappler_id: grapplerId }) }, tok()),
  shove: (cid: string, targetId: string, knockProne: boolean) => api<ShoveResult>(`/combatants/${cid}/shove`, { method: 'POST', body: JSON.stringify({ target_id: targetId, knock_prone: knockProne }) }, tok()),
  standUp: (cid: string) => api<Combatant>(`/combatants/${cid}/stand-up`, { method: 'POST' }, tok()),
  heal: (cid: string, body: { amount: number; source_combatant_id?: string; label?: string }) => api<import('$lib/types').HealResult>(`/combatants/${cid}/heal`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  deathSave: (cid: string, body?: { advantage?: boolean; disadvantage?: boolean; label?: string }) => api<import('$lib/types').DeathSaveResult>(`/combatants/${cid}/death-save`, { method: 'POST', body: JSON.stringify(body ?? {}) }, tok()),
  skillCheck: (cid: string, body: { skill: string; dc?: number; advantage?: boolean; disadvantage?: boolean; label?: string }) => api<import('$lib/types').SkillCheckResult>(`/combatants/${cid}/skill-check`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  multiattack: (cid: string, body: { targets: Array<{ target_id: string; attack_expression?: string; damage_expression?: string; damage_type: string; ability?: string; weapon_id?: string; label?: string }> }) => api<import('$lib/types').MultiAttackResult>(`/combatants/${cid}/multiattack`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  parseMultiattack: (cid: string) => api<{ attacks: Array<{ name: string; attack_expression?: string; damage_expression?: string; damage_type: string; label?: string }> }>(`/combatants/${cid}/parse-multiattack`, {}, tok()),
  triggerReady: (cid: string) => api<Combatant>(`/combatants/${cid}/trigger-ready`, { method: 'POST' }, tok()),
  classFeature: (cid: string, feature: string, targetId?: string) => api<import('$lib/types').ClassFeatureResult>(`/combatants/${cid}/class-feature`, { method: 'POST', body: JSON.stringify({ feature, target_id: targetId }) }, tok()),
  twoWeaponFight: (cid: string, targetId: string, offhandWeaponId: string) => api<Combatant>(`/combatants/${cid}/two-weapon-fight`, { method: 'POST', body: JSON.stringify({ target_id: targetId, offhand_weapon_id: offhandWeaponId }) }, tok()),
  dash: (cid: string, useBonusAction?: boolean) => api<Combatant>(`/combatants/${cid}/dash`, { method: 'POST', body: JSON.stringify({ use_bonus_action: useBonusAction ?? false }) }, tok()),
  hide: (cid: string, useBonusAction?: boolean) => api<Combatant>(`/combatants/${cid}/hide`, { method: 'POST', body: JSON.stringify({ use_bonus_action: useBonusAction ?? false }) }, tok()),
  search: (cid: string, label?: string) => api<Combatant>(`/combatants/${cid}/search`, { method: 'POST', body: JSON.stringify({ label }) }, tok()),
  useObject: (cid: string, label?: string, targetId?: string) => api<Combatant>(`/combatants/${cid}/use-object`, { method: 'POST', body: JSON.stringify({ label, target_id: targetId }) }, tok()),
  addCondition: (cid: string, condition: string, remove?: boolean, durationRounds?: number) => api<Combatant>(`/combatants/${cid}/conditions`, { method: 'POST', body: JSON.stringify({ condition, remove, duration_rounds: durationRounds }) }, tok()),
  overlayDamage: (eid: string, body: { overlay_id: string; damage_expression: string; damage_type: string; save_ability?: string; save_dc?: number; half_on_save?: boolean; is_magical?: boolean; label?: string }) => api<import('$lib/types').OverlayDamageResult>(`/encounters/${eid}/overlay-damage`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  surpriseRound: (eid: string, surprisedIds: string[]) => api<Encounter>(`/encounters/${eid}/surprise`, { method: 'POST', body: JSON.stringify({ surprised_combatant_ids: surprisedIds }) }, tok()),
  surpriseAuto: (eid: string, ambusherIds: string[]) => api<{ surprised_ids: string[]; stealth_rolls: Array<{ combatant_id: string; name: string; stealth_total: number; natural: number }>; perceptions: Array<{ combatant_id: string; name: string; passive_perception: number; surprised: boolean }> }>(`/encounters/${eid}/surprise-auto`, { method: 'POST', body: JSON.stringify({ ambusher_ids: ambusherIds }) }, tok()),
  flanking: (eid: string) => api<{ flanking_pairs: FlankPair[] }>(`/encounters/${eid}/flanking`, {}, tok()),
  cover: (eid: string, attackerId: string, targetId: string) => api<CoverResult>(`/encounters/${eid}/cover?attacker_id=${attackerId}&target_id=${targetId}`, {}, tok()),
  events: (eid: string, limit = 100, offset = 0) =>
    api<Array<{ id: string; encounter_id: string; round: number; actor_combatant: string | null; target_combatant: string | null; action: string; delta_hp: number | null; note: string | null; created_at: string }>>(`/encounters/${eid}/events?limit=${limit}&offset=${offset}`, {}, tok()),
  deleteEvent: (eventId: string) => api<void>(`/combat-events/${eventId}`, { method: 'DELETE' }, tok()),
};

export const Overlays = {
  list: (eid: string) => api<EncounterOverlay[]>(`/encounters/${eid}/overlays`, {}, tok()),
  create: (eid: string, body: Partial<EncounterOverlay>) =>
    api<EncounterOverlay>(`/encounters/${eid}/overlays`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  delete: (eid: string, oid: string) =>
    api<void>(`/encounters/${eid}/overlays/${oid}`, { method: 'DELETE' }, tok()),
};

export const Encounters = {
  list: (cid: string) => api<Encounter[]>(`/campaigns/${cid}/encounters`, {}, tok()),
  create: (cid: string, body: Partial<Encounter>) =>
    api<Encounter>(`/campaigns/${cid}/encounters`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  get: (id: string) => api<Encounter>(`/encounters/${id}`, {}, tok()),
  update: (id: string, patch: Partial<Encounter>) =>
    api<Encounter>(`/encounters/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/encounters/${id}`, { method: 'DELETE' }, tok()),
  start: (id: string) => api<Encounter>(`/encounters/${id}/start`, { method: 'POST' }, tok()),
  end: (id: string) => api<Encounter>(`/encounters/${id}/end`, { method: 'POST' }, tok()),
  nextTurn: (id: string) => api<Encounter>(`/encounters/${id}/next-turn`, { method: 'POST' }, tok()),
  prevTurn: (id: string) => api<Encounter>(`/encounters/${id}/prev-turn`, { method: 'POST' }, tok()),
  gotoTurn: (id: string, turn_index: number) =>
    api<Encounter>(`/encounters/${id}/goto-turn`, { method: 'POST', body: JSON.stringify({ turn_index }) }, tok()),
  setInitiative: (id: string, character_id: string, initiative: number) =>
    api<Encounter>(`/encounters/${id}/set-initiative`, { method: 'POST',
      body: JSON.stringify({ character_id, initiative }) }, tok()),
  combatants: {
    list: (eid: string) => api<Combatant[]>(`/encounters/${eid}/combatants`, {}, tok()),
    add: (eid: string, body: Partial<Combatant>) =>
      api<Combatant>(`/encounters/${eid}/combatants`, { method: 'POST', body: JSON.stringify(body) }, tok()),
    update: (id: string, patch: Partial<Combatant>) =>
      api<Combatant>(`/combatants/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
    delete: (id: string) => api<void>(`/combatants/${id}`, { method: 'DELETE' }, tok()),
    move: (id: string, x: number, y: number) =>
      api<Combatant>(`/combatants/${id}/move`, { method: 'POST', body: JSON.stringify({ x, y }) }, tok()),
  },
};

export const Notifications = {
  list: (limit = 50, offset = 0, unreadOnly = false) =>
    api<Notification[]>(`/notifications?limit=${limit}&offset=${offset}${unreadOnly ? '&unread_only=true' : ''}`, {}, tok()),
  markRead: (id: string) => api<void>(`/notifications/${id}/read`, { method: 'POST' }, tok()),
  markAllRead: () => api<void>('/notifications/read-all', { method: 'POST' }, tok()),
  delete: (id: string) => api<void>(`/notifications/${id}`, { method: 'DELETE' }, tok()),
  deleteAll: () => api<void>('/notifications', { method: 'DELETE' }, tok()),
};
