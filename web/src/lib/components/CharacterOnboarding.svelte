<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { ChevronRight } from '@lucide/svelte';
  import { onboardingSteps, type OnboardingChar } from './onboardingSteps';

  const LS_PREFIX = 'cg_onboard_';

  type Props = {
    character: OnboardingChar;
    canEdit: boolean;
    onSwitchTab?: (tab: string) => void;
  };
  let { character, canEdit, onSwitchTab }: Props = $props();

  let steps = $derived(onboardingSteps(character));

  let dismissed = $state<string[]>([]);
  let initialised = $state(false);
  let currentIdx = $state(0);

  const storageKey = $derived(`${LS_PREFIX}${character.id}`);

  $effect(() => {
    try {
      const raw = localStorage.getItem(storageKey);
      if (raw) dismissed = JSON.parse(raw);
    } catch { dismissed = []; }
    initialised = true;
  });

  function save() {
    try { localStorage.setItem(storageKey, JSON.stringify(dismissed)); } catch {}
  }

  function dismissCurrent() {
    const step = steps[currentIdx];
    if (!step) return;
    dismissed = [...dismissed, step.id];
    save();
    advance();
  }

  function skipAll() {
    const remaining = steps.filter((s) => !dismissed.includes(s.id));
    dismissed = [...dismissed, ...remaining.map((s) => s.id), 'all'];
    save();
    currentIdx = steps.length;
  }

  function advance() {
    if (currentIdx >= steps.length - 1) return;
    for (let i = currentIdx + 1; i < steps.length; i++) {
      if (dismissed.includes(steps[i].id)) continue;
      if (steps[i].autoComplete()) {
        dismissed = [...dismissed, steps[i].id];
        save();
        continue;
      }
      currentIdx = i;
      return;
    }
    currentIdx = steps.length;
  }

  let currentStep = $derived(steps[currentIdx] ?? null);

  $effect(() => {
    if (!initialised) return;
    if (dismissed.includes('all')) { currentIdx = steps.length; return; }
    const step = steps[currentIdx];
    if (!step) return;
    if (step.autoComplete()) {
      dismissed = [...dismissed, step.id];
      save();
      advance();
    }
  });

  let stepNum = $derived(currentIdx + 1);
  let totalSteps = $derived(steps.length);

  function isDone() {
    return currentIdx >= steps.length || !currentStep || dismissed.includes(currentStep.id) || dismissed.includes('all');
  }

  let tooltipStyle = $state<Record<string, string>>({});
  let arrowStyle = $state<Record<string, string>>({});
  let ringStyle = $state<Record<string, string>>({});
  let visible = $state(false);
  let arrowBelow = $state(false);
  let anchorEl = $state<Element | null>(null);

  function computePositions() {
    const step = currentStep;
    if (!step) {
      visible = false;
      return;
    }
    const anchor = document.querySelector(step.anchorSel);
    if (!anchor) {
      visible = false;
      return;
    }
    const sheet = document.querySelector('#ob-sheet');
    const sr = sheet?.getBoundingClientRect();
    if (!sr) { visible = false; return; }
    const ar = anchor.getBoundingClientRect();

    const anchorTop = ar.top - sr.top;
    const anchorLeft = ar.left - sr.left;
    const anchorWidth = ar.width;
    const tooltipHeight = 100;
    const below = anchorTop < tooltipHeight + 20;
    arrowBelow = below;
    anchorEl = anchor;

    const top = below
      ? anchorTop + ar.height + 14
      : anchorTop - 10;
    const left = anchorLeft + anchorWidth / 2;

    tooltipStyle = {
      top: `${top}px`,
      left: `${left}px`,
      transform: below ? 'translate(-50%, 0)' : 'translate(-50%, -100%)',
    };
    arrowStyle = below
      ? { top: '-6px', left: '50%', transform: 'translateX(-50%) rotate(45deg)', borderTop: '1px solid #c9a84c', borderLeft: '1px solid #c9a84c' }
      : { bottom: '-6px', left: '50%', transform: 'translateX(-50%) rotate(45deg)', borderRight: '1px solid #c9a84c', borderBottom: '1px solid #c9a84c' };
    ringStyle = {
      top: `${anchorTop - 4}px`,
      left: `${anchorLeft - 4}px`,
      width: `${anchorWidth + 8}px`,
      height: `${ar.height + 8}px`,
    };
    visible = true;
  }

  let rafId: number;
  function updatePos() {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      computePositions();
      rafId = 0;
    });
  }

  $effect(() => {
    if (isDone() || !canEdit) { visible = false; return; }
    updatePos();
  });

  function handleTooltipClick() {
    if (currentStep?.tab && onSwitchTab) {
      onSwitchTab(currentStep.tab);
    }
  }
</script>

{#if initialised && steps.length > 0 && canEdit && !dismissed.includes('all')}
  {#if !isDone() && currentStep}
    {@const msg = $_('character.onboarding.' + currentStep.msgKey.replace('onboarding.', ''))}
    <div id="ob-backdrop" class={visible ? 'ob-visible' : ''}></div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      id="ob-ring"
      style={Object.entries(ringStyle).map(([k, v]) => `${k}:${v}`).join(';') || 'display:none'}
      class={visible ? 'ob-visible' : ''}
    ></div>
    <div
      id="ob-tooltip"
      role="button"
      tabindex="0"
      onscroll={updatePos}
      onclick={handleTooltipClick}
      onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleTooltipClick(); } }}
      style={Object.entries(tooltipStyle).map(([k, v]) => `${k}:${v}`).join(';') || 'display:none'}
      class={['absolute z-50 max-w-80 rounded-lg border-2 px-5 py-4 pointer-events-auto', visible ? 'ob-visible' : '', currentStep.tab && onSwitchTab ? 'cursor-pointer' : ''].join(' ')}
    >
      <div class="flex items-center gap-1 mb-2">
        <span class="text-[11px] font-bold tracking-widest uppercase" style="color:#c9a84c;">
          {$_('character.onboarding.step_count').replace('{{current}}', String(stepNum)).replace('{{total}}', String(totalSteps))}
        </span>
        <span class="flex-1"></span>
        <button type="button"
          onclick={(e: MouseEvent) => { e.stopPropagation(); skipAll(); }}
          class="text-[10px] underline hover:opacity-70"
          style="color:#8b6355;">
          {$_('character.onboarding.skip_all')}
        </button>
      </div>
      <div class="flex items-start gap-3">
        <span class="text-[14px] leading-relaxed flex-1 font-medium" style="color:#f4e4c1;">{msg}</span>
        <button type="button"
          onclick={(e: MouseEvent) => { e.stopPropagation(); dismissCurrent(); }}
          class="shrink-0 rounded-full w-6 h-6 flex items-center justify-center hover:opacity-80"
          style="background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #f4e4c1;color:#1a0f08;"
          title="{$_('character.onboarding.next')}"><ChevronRight size={14} /></button>
      </div>
      {#if currentStep.tab && onSwitchTab}
        <div class="mt-2 text-[10px] italic text-center" style="color:#a6855c;">
          {$_('character.onboarding.click_to_switch')}
        </div>
      {/if}
      <div
        style={Object.entries(arrowStyle).map(([k, v]) => `${k}:${v}`).join(';')}
        class="absolute w-3.5 h-3.5"
      ></div>
    </div>
  {/if}
{/if}

<style>
  #ob-tooltip {
    background: linear-gradient(180deg, #3a2313 0%, #2c1810 100%);
    border-color: #c9a84c;
    opacity: 0;
    box-shadow: 0 0 24px rgba(201,168,76,0.15), 0 4px 20px rgba(0,0,0,0.6);
  }
  #ob-tooltip.ob-visible {
    opacity: 1;
    transition: opacity 0.25s ease;
  }
  #ob-tooltip .absolute {
    background: #3a2313;
  }
  #ob-backdrop {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: rgba(0,0,0,0);
    transition: background 0.3s ease;
    border-radius: inherit;
    z-index: 45;
  }
  #ob-backdrop.ob-visible {
    background: rgba(0,0,0,0.35);
  }
  #ob-ring {
    position: absolute;
    border-radius: 8px;
    pointer-events: none;
    opacity: 0;
    border: 2px solid transparent;
    z-index: 46;
  }
  #ob-ring.ob-visible {
    opacity: 1;
    border-color: #c9a84c;
    animation: ob-pulse 1.5s ease-in-out infinite;
  }
  @keyframes ob-pulse {
    0%, 100% { box-shadow: 0 0 0 0 rgba(201,168,76,0.5); border-color: #c9a84c; }
    50% { box-shadow: 0 0 0 10px rgba(201,168,76,0); border-color: #f4e4c1; }
  }
</style>
