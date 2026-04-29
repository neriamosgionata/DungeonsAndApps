import { getContext, setContext } from 'svelte';

const KEY = Symbol('campaign-ctx');

export type CampaignCtx = {
  isMaster: boolean;
  campaignId: string;
  leveling: 'xp' | 'milestone';
};

export function provideCampaign(ctx: () => CampaignCtx) {
  setContext(KEY, ctx);
}

export function useCampaign(): () => CampaignCtx {
  return getContext<() => CampaignCtx>(KEY)
    ?? (() => ({ isMaster: false, campaignId: '', leveling: 'xp' as const }));
}
