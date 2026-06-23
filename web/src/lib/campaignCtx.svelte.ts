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
  // I-F1: throw if no context is provided. Pre-fix silently returned a
  // fake context with isMaster=false, which hid GM-only features
  // (e.g. showGrid toggle, zone creation buttons) without any visible
  // error. The parent layout is responsible for calling provideCampaign()
  // before any child that needs the context.
  const ctx = getContext<() => CampaignCtx>(KEY);
  if (!ctx) {
    throw new Error(
      'useCampaign() called outside of a layout that called provideCampaign(). ' +
      'Wrap this component in <CampaignCtx> or call provideCampaign() in the parent layout.'
    );
  }
  return ctx;
}
