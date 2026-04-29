/**
 * Registry of optional inline actions shown on notification toasts.
 *
 * Each entry maps a notification `kind` to a spec that describes:
 *  - a label (i18n key or raw string resolver),
 *  - an icon component (from `@lucide/svelte`),
 *  - a handler that runs when the user clicks the action.
 *
 * Adding a new action just means registering it here — `NotifToasts.svelte`
 * reads the registry and renders the button automatically.
 */

import type { Component } from 'svelte';
import { Dice5, MessageSquare, ScrollText, Check } from '@lucide/svelte';
import { goto } from '$app/navigation';
import { Characters, Dice, Encounters, Invitations } from '$lib/api/resources';
import { auth } from '$lib/stores/auth.svelte';
import type { Notif } from '$lib/notifications.svelte';

export type NotifActionCtx = {
  dismiss: () => void;
  markRead: () => Promise<void>;
};

export type NotifAction = {
  label: string;
  icon: Component;
  /**
   * Return false to keep the toast visible (e.g. validation error handled
   * inside the handler). Default behavior: dismiss + mark-read on resolve.
   */
  run: (n: Notif, ctx: NotifActionCtx) => Promise<boolean | void>;
  /**
   * Optional gate — hide the button entirely when this returns false.
   * Example: "Roll initiative" only if user owns a pending combatant.
   */
  show?: (n: Notif) => boolean;
};

async function rollInitiativeAction(n: Notif): Promise<boolean> {
  if (!n.ref_id || !n.campaign_id) return false;
  const [chars, combs] = await Promise.all([
    Characters.list(n.campaign_id),
    Encounters.combatants.list(n.ref_id),
  ]);
  const mine = (chars as Record<string, unknown>[]).filter((c) => c.owner_id === auth.user?.id);
  const pending = (combs as Record<string, unknown>[]).find((c) =>
    !c.initiative_rolled && mine.some((m) => m.id === c.character_id));
  if (!pending) return true; // nothing to do; dismiss silently
  const myChar = mine.find((m) => m.id === pending.character_id) as Record<string, unknown>;
  const sheet = (myChar.sheet ?? {}) as Record<string, unknown>;
  const explicit = sheet.initiative as number | undefined;
  const ab = (sheet.abilities ?? {}) as Record<string, number | undefined>;
  const bonus = typeof explicit === 'number' ? explicit : Math.floor(((ab.dex ?? 10) - 10) / 2);
  const expr = bonus >= 0 ? `1d20+${bonus}` : `1d20${bonus}`;
  const roll = await Dice.roll(n.campaign_id, expr, `Initiative — ${myChar.name as string}`, false, myChar.id as string);
  await Encounters.setInitiative(n.ref_id, myChar.id as string, roll.total);
  return true;
}

/** Built-in action registry. Additional actions can be registered at runtime. */
export const notifActions: Record<string, NotifAction> = {
  'campaign.invitation': {
    label: 'Accept',
    icon: Check,
    run: async (n) => {
      if (!n.ref_id || !n.campaign_id) return false;
      await Invitations.accept(n.ref_id);
      await goto(`/campaigns/${n.campaign_id}`);
      return true;
    },
  },
  'combat.roll_initiative': {
    label: 'Roll initiative',
    icon: Dice5,
    run: rollInitiativeAction,
  },
  'combat.created': {
    label: 'Open combat',
    icon: ScrollText,
    run: async (n) => {
      if (!n.campaign_id) return false;
      await goto(`/campaigns/${n.campaign_id}/initiative`);
      return true;
    },
  },
  'combat.started': {
    label: 'Roll initiative',
    icon: Dice5,
    run: rollInitiativeAction,
  },
  'chat.whisper': {
    label: 'Reply',
    icon: MessageSquare,
    run: async (n) => {
      if (!n.campaign_id) return false;
      const base = `/campaigns/${n.campaign_id}/messages`;
      await goto(n.ref_id ? `${base}?whisper=${n.ref_id}` : base);
      return true;
    },
  },
};

/** Register or override an action at runtime (for plugins/features). */
export function registerNotifAction(kind: string, action: NotifAction) {
  notifActions[kind] = action;
}

export function getNotifAction(n: Notif): NotifAction | null {
  const a = notifActions[n.kind];
  if (!a) return null;
  if (a.show && !a.show(n)) return null;
  return a;
}
