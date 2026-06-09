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
    const ar = anchor.getBoundingClientRect();
    const sheet = document.querySelector('#ob-sheet');
    const sr = sheet?.getBoundingClientRect();
    if (!sr) { visible = false; return; }

    const anchorTop = ar.top - sr.top;
    const anchorLeft = ar.left - sr.left;
    const anchorWidth = ar.width;
    const tooltipHeight = 80; // approx height
    const below = anchorTop < tooltipHeight + 20;
    arrowBelow = below;

    const top = below
      ? anchorTop + ar.height + 12
      : anchorTop - 8;
    const left = anchorLeft + anchorWidth / 2;

    tooltipStyle = {
      top: `${top}px`,
      left: `${left}px`,
      transform: below ? 'translate(-50%, 0)' : 'translate(-50%, -100%)',
    };
    arrowStyle = below
      ? { top: '-6px', left: '50%', transform: 'translateX(-50%) rotate(45deg)', borderTop: '1px solid #8b6914', borderLeft: '1px solid #8b6914' }
      : { bottom: '-6px', left: '50%', transform: 'translateX(-50%) rotate(45deg)', borderRight: '1px solid #8b6914', borderBottom: '1px solid #8b6914' };
    ringStyle = {
      top: `${anchorTop}px`,
      left: `${anchorLeft}px`,
      width: `${anchorWidth}px`,
      height: `${ar.height}px`,
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
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      id="ob-ring"
      style={Object.entries(ringStyle).map(([k, v]) => `${k}:${v}`).join(';') || 'display:none'}
      class={visible ? 'ob-visible' : ''}
    ></div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      id="ob-tooltip"
      onscroll={updatePos}
      onclick={handleTooltipClick}
      style={Object.entries(tooltipStyle).map(([k, v]) => `${k}:${v}`).join(';') || 'display:none'}
      class={['fixed z-50 max-w-64 rounded-lg border px-4 py-3 shadow-xl pointer-events-auto', visible ? 'ob-visible' : '', currentStep.tab && onSwitchTab ? 'cursor-pointer' : ''].join(' ')}
    >
      <div class="flex items-center gap-1 mb-1.5">
        <span class="text-[10px] font-bold tracking-widest uppercase" style="color:#8b6914;">
          {$_('character.onboarding.step_count').replace('{{current}}', String(stepNum)).replace('{{total}}', String(totalSteps))}
        </span>
        <span class="flex-1"></span>
        <button type="button"
          onclick={(e: MouseEvent) => { e.stopPropagation(); skipAll(); }}
          class="text-[10px] underline hover:opacity-80"
          style="color:#8b6355;">
          {$_('character.onboarding.skip_all')}
        </button>
      </div>
      <div class="flex items-start gap-2">
        <span class="text-[13px] leading-snug flex-1" style="color:#f4e4c1;">{msg}</span>
        <button type="button"
          onclick={(e: MouseEvent) => { e.stopPropagation(); dismissCurrent(); }}
          class="shrink-0 rounded-full w-5 h-5 flex items-center justify-center hover:opacity-80"
          style="background:rgba(139,26,26,0.5);color:#f4e4c1;"
          title="{$_('character.onboarding.next')}"><ChevronRight size={11} /></button>
      </div>
      {#if currentStep.tab && onSwitchTab}
        <div class="mt-1.5 text-[10px] italic text-center" style="color:#6d510f;">
          {$_('character.onboarding.click_to_switch')}
        </div>
      {/if}
      <div
        style={Object.entries(arrowStyle).map(([k, v]) => `${k}:${v}`).join(';')}
        class="absolute w-3 h-3"
      ></div>
    </div>
  {/if}
{/if}

<style>
  #ob-tooltip {
    background: linear-gradient(180deg, #3a2313, #2c1810);
    border-color: #8b6914;
    opacity: 0;
  }
  #ob-tooltip.ob-visible {
    opacity: 1;
    transition: opacity 0.2s ease;
  }
  #ob-tooltip .absolute {
    background: #2c1810;
  }
  #ob-ring {
    position: absolute;
    border-radius: 6px;
    pointer-events: none;
    opacity: 0;
    border: 2px solid transparent;
  }
  #ob-ring.ob-visible {
    opacity: 1;
    border-color: #c9a84c;
    animation: ob-pulse 1.8s ease-in-out infinite;
  }
  @keyframes ob-pulse {
    0%, 100% { box-shadow: 0 0 0 0 rgba(201,168,76,0.4); border-color: #c9a84c; }
    50% { box-shadow: 0 0 0 6px rgba(201,168,76,0); border-color: #f4e4c1; }
  }
</style>
