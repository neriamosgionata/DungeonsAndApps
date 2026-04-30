import { api } from './client';
import { auth } from '$lib/stores/auth.svelte';
import type {
  User, Campaign, Spell, DiceRollResult, DiceHistory,
  Character, NPC, Faction, LoreEntry, NewsEntry, Quest,
  Map, MapPin, PartyData, LootItem, CampaignSession, Encounter,
  Combatant, Message, Notification, Invitation, Member,
  CombatantEffect
} from '$lib/types';

function tok() { return auth.token ?? undefined; }

export const Users = {
  list: () => api<Array<{id:string; email:string; display_name:string; role:string; language:string; created_at:string}>>('/users', {}, tok()),
  update: (id: string, patch: { display_name?: string; role?: 'user' | 'admin'; language?: 'en' | 'it' }) =>
    api(`/users/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/users/${id}`, { method: 'DELETE' }, tok()),
  resetPassword: (id: string, new_password: string) =>
    api<void>(`/users/${id}/reset-password`, { method: 'POST', body: JSON.stringify({ new_password }) }, tok()),
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
export const Quests   = crud<Quest>('quests', 'quests');

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
