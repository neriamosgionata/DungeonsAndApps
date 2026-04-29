import { api } from './client';
import { auth } from '$lib/stores/auth.svelte';
import type { User, Campaign, Spell, DiceRollResult } from '$lib/types';

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
  mine: () => api<Array<{
    id: string; campaign_id: string; user_id: string; role: string;
    message: string | null; created_at: string; accepted: boolean | null;
    campaign_name: string; inviter_name: string | null;
  }>>('/invitations', {}, tok()),
  create: (cid: string, email: string, role: 'player' | 'master' = 'player', message?: string) =>
    api(`/campaigns/${cid}/invitations`, { method: 'POST',
      body: JSON.stringify({ email, role, message }) }, tok()),
  forCampaign: (cid: string) =>
    api<Array<{
      id: string; campaign_id: string; user_id: string; role: string;
      message: string | null; created_at: string; accepted: boolean | null;
      campaign_name: string; inviter_name: string | null;
    }>>(`/campaigns/${cid}/invitations`, {}, tok()),
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
  members: (id: string) =>
    api<{ user_id: string; display_name: string; email: string; role: string; character_limit: number }[]>(
      `/campaigns/${id}/members`, {}, tok()),
  addMember: (id: string, email: string, role: 'player' | 'master') =>
    api(`/campaigns/${id}/members`, { method: 'POST', body: JSON.stringify({ email, role }) }, tok()),
  updateMember: (id: string, userId: string, patch: { character_limit?: number; role?: 'player' | 'master' }) =>
    api(`/campaigns/${id}/members/${userId}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  removeMember: (id: string, userId: string) =>
    api<void>(`/campaigns/${id}/members/${userId}`, { method: 'DELETE' }, tok()),
  presence: (id: string) => api<string[]>(`/campaigns/${id}/presence`, {}, tok()),
};

type AnyVal = Record<string, unknown>;

export const Characters = {
  list: (campaign: string) => api<AnyVal[]>(`/campaigns/${campaign}/characters`, {}, tok()),
  create: (campaign: string, body: AnyVal) =>
    api<AnyVal>(`/campaigns/${campaign}/characters`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  get: (id: string) => api<AnyVal>(`/characters/${id}`, {}, tok()),
  update: (id: string, patch: AnyVal) =>
    api<AnyVal>(`/characters/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/characters/${id}`, { method: 'DELETE' }, tok()),
};

export const Sessions = {
  list: (cid: string) => api<AnyVal[]>(`/campaigns/${cid}/sessions`, {}, tok()),
  create: (cid: string, body: AnyVal) =>
    api<AnyVal>(`/campaigns/${cid}/sessions`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  update: (id: string, patch: AnyVal) =>
    api<AnyVal>(`/sessions/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/sessions/${id}`, { method: 'DELETE' }, tok()),
};

export const Maps = {
  list: (cid: string) => api<AnyVal[]>(`/campaigns/${cid}/maps`, {}, tok()),
  create: (cid: string, body: AnyVal) =>
    api<AnyVal>(`/campaigns/${cid}/maps`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  update: (id: string, patch: AnyVal) =>
    api<AnyVal>(`/maps/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/maps/${id}`, { method: 'DELETE' }, tok()),
  pins: {
    list: (mapId: string) => api<AnyVal[]>(`/maps/${mapId}/pins`, {}, tok()),
    create: (mapId: string, body: AnyVal) =>
      api<AnyVal>(`/maps/${mapId}/pins`, { method: 'POST', body: JSON.stringify(body) }, tok()),
    update: (id: string, patch: AnyVal) =>
      api<AnyVal>(`/pins/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
    delete: (id: string) => api<void>(`/pins/${id}`, { method: 'DELETE' }, tok()),
  },
};

function crud<T>(col: string, itemPath: string) {
  return {
    list: (cid: string) => api<T[]>(`/campaigns/${cid}/${col}`, {}, tok()),
    create: (cid: string, body: AnyVal) =>
      api<T>(`/campaigns/${cid}/${col}`, { method: 'POST', body: JSON.stringify(body) }, tok()),
    get: (id: string) => api<T>(`/${itemPath}/${id}`, {}, tok()),
    update: (id: string, patch: AnyVal) =>
      api<T>(`/${itemPath}/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
    delete: (id: string) => api<void>(`/${itemPath}/${id}`, { method: 'DELETE' }, tok()),
  };
}

export const Factions = crud<AnyVal>('factions', 'factions');
export const NPCs     = crud<AnyVal>('npcs', 'npcs');
export const Lore     = crud<AnyVal>('lore', 'lore');
export const News     = crud<AnyVal>('news', 'news');
export const Quests   = crud<AnyVal>('quests', 'quests');

export const Party = {
  get: (cid: string) => api<AnyVal>(`/campaigns/${cid}/party`, {}, tok()),
  update: (cid: string, patch: AnyVal) =>
    api<AnyVal>(`/campaigns/${cid}/party`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
};

export const Loot = {
  list: (cid: string) => api<AnyVal[]>(`/campaigns/${cid}/loot`, {}, tok()),
  create: (cid: string, body: AnyVal) =>
    api<AnyVal>(`/campaigns/${cid}/loot`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  update: (id: string, patch: AnyVal) =>
    api<AnyVal>(`/loot/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/loot/${id}`, { method: 'DELETE' }, tok()),
};

export const Messages = {
  chat: (cid: string) => api<AnyVal[]>(`/campaigns/${cid}/messages`, {}, tok()),
  whispers: (cid: string, withUser?: string) =>
    api<AnyVal[]>(`/campaigns/${cid}/messages?whispers=true${withUser ? `&with_user=${withUser}` : ''}`, {}, tok()),
  send: (cid: string, body: string, scope: 'campaign' | 'whisper', recipient_id?: string) =>
    api<AnyVal>(`/campaigns/${cid}/messages`, { method: 'POST',
      body: JSON.stringify({ body, scope, recipient_id }) }, tok()),
  delete: (id: string) => api<void>(`/messages/${id}`, { method: 'DELETE' }, tok()),
};

export const Dice = {
  roll: (cid: string, expression: string, label?: string, isPrivate = false, character_id?: string) =>
    api<DiceRollResult>(`/campaigns/${cid}/dice`, { method: 'POST',
      body: JSON.stringify({ expression, label, private: isPrivate, character_id }) }, tok()),
  history: (cid: string, limit = 50) =>
    api<AnyVal[]>(`/campaigns/${cid}/dice?limit=${limit}`, {}, tok()),
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

export const Encounters = {
  list: (cid: string) => api<AnyVal[]>(`/campaigns/${cid}/encounters`, {}, tok()),
  create: (cid: string, body: AnyVal) =>
    api<AnyVal>(`/campaigns/${cid}/encounters`, { method: 'POST', body: JSON.stringify(body) }, tok()),
  get: (id: string) => api<AnyVal>(`/encounters/${id}`, {}, tok()),
  update: (id: string, patch: AnyVal) =>
    api<AnyVal>(`/encounters/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
  delete: (id: string) => api<void>(`/encounters/${id}`, { method: 'DELETE' }, tok()),
  start: (id: string) => api<AnyVal>(`/encounters/${id}/start`, { method: 'POST' }, tok()),
  end: (id: string) => api<AnyVal>(`/encounters/${id}/end`, { method: 'POST' }, tok()),
  nextTurn: (id: string) => api<AnyVal>(`/encounters/${id}/next-turn`, { method: 'POST' }, tok()),
  prevTurn: (id: string) => api<AnyVal>(`/encounters/${id}/prev-turn`, { method: 'POST' }, tok()),
  gotoTurn: (id: string, turn_index: number) =>
    api<AnyVal>(`/encounters/${id}/goto-turn`, { method: 'POST', body: JSON.stringify({ turn_index }) }, tok()),
  setInitiative: (id: string, character_id: string, initiative: number) =>
    api<AnyVal>(`/encounters/${id}/set-initiative`, { method: 'POST',
      body: JSON.stringify({ character_id, initiative }) }, tok()),
  combatants: {
    list: (eid: string) => api<AnyVal[]>(`/encounters/${eid}/combatants`, {}, tok()),
    add: (eid: string, body: AnyVal) =>
      api<AnyVal>(`/encounters/${eid}/combatants`, { method: 'POST', body: JSON.stringify(body) }, tok()),
    update: (id: string, patch: AnyVal) =>
      api<AnyVal>(`/combatants/${id}`, { method: 'PATCH', body: JSON.stringify(patch) }, tok()),
    delete: (id: string) => api<void>(`/combatants/${id}`, { method: 'DELETE' }, tok()),
  },
};
