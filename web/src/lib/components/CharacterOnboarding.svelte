<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { X } from '@lucide/svelte';

  const LS_PREFIX = 'cg_onboard_';

  type Props = {
    character: {
      id: string;
      name: string;
      race?: string | null;
      level_total: number;
      sheet: Record<string, unknown>;
    };
    canEdit: boolean;
  };
  let { character, canEdit }: Props = $props();

  type Step = {
    id: string;
    msgKey: string;
    tab: string | null;
    anchorSel: string;
    autoComplete: () => boolean;
  };

  function stepsFor(c: Props['character']): Step[] {
    const sheet = (c.sheet ?? {}) as Record<string, unknown>;
    const classes = (sheet.classes as Array<{ name?: string; level?: number; subclass?: string }>) ?? [];
    const named = classes.filter((cl) => !!cl.name?.trim());
    const spells = (sheet.spells as Array<unknown>) ?? [];
    const equipment = (sheet.equipment as Array<unknown>) ?? [];
    const abilities = (sheet.abilities as Record<string, number | undefined>) ?? {};
    const skills = (sheet.skills as Record<string, string> | undefined) ?? {};
    const hasSkills = Object.keys(skills).length > 0;
    const allAbilities10 = (['str','dex','con','int','wis','cha'] as const).every((k) => (abilities[k] ?? 10) === 10);

    const hasCasterClass = classes.some((cl) => {
      const n = (cl.name ?? '').toLowerCase();
      if (!n) return false;
      return ['bard','cleric','druid','paladin','ranger','sorcerer','warlock','wizard','artificer'].some((cn) => n.includes(cn));
    });

    const steps: Step[] = [];
    const raceSet = !!c.race;

    // Step 1: Level > 1
    if (c.level_total <= 1) {
      steps.push({
        id: 'level',
        msgKey: 'onboarding.level_hint',
        tab: null,
        anchorSel: '#ob-level',
        autoComplete: () => c.level_total > 1,
      });
    }

    // Step 2: Race
    if (!raceSet) {
      steps.push({
        id: 'race',
        msgKey: 'onboarding.race_hint',
        tab: null,
        anchorSel: '#ob-race',
        autoComplete: () => !!c.race,
      });
    }

    // Step 3: Class
    if (named.length === 0) {
      steps.push({
        id: 'class',
        msgKey: 'onboarding.class_hint',
        tab: 'features',
        anchorSel: '#ob-tab-features',
        autoComplete: () => {
          const cl = (c.sheet as Record<string, unknown>)?.classes as Array<{ name?: string }> | undefined;
          return (cl ?? []).some((x) => !!x.name?.trim());
        },
      });
    } else {
      // Step 3b: Subclass (if class without subclass)
      const classWithoutSub = named.find((cl) => !cl.subclass?.trim());
      if (classWithoutSub && classWithoutSub.name) {
        const cn = classWithoutSub.name.toLowerCase();
        const hasSubclasses = ['fighter','rogue','wizard','cleric','druid','barbarian','bard','monk','paladin','ranger','sorcerer','warlock'].some((sc) => cn.includes(sc));
        if (hasSubclasses) {
          steps.push({
            id: 'subclass',
            msgKey: 'onboarding.subclass_hint',
            tab: 'features',
            anchorSel: '#ob-tab-features',
            autoComplete: () => {
              const cl = (c.sheet as Record<string, unknown>)?.classes as Array<{ subclass?: string }> | undefined;
              return (cl ?? []).some((x) => !!x.subclass?.trim());
            },
          });
        }
      }
    }

    // Step 4: Ability scores
    if (allAbilities10) {
      steps.push({
        id: 'abilities',
        msgKey: 'onboarding.abilities_hint',
        tab: 'combat',
        anchorSel: '#ob-tab-combat',
        autoComplete: () => {
          const ab = (c.sheet as Record<string, unknown>)?.abilities as Record<string, number | undefined> | undefined;
          if (!ab) return false;
          return (['str','dex','con','int','wis','cha'] as const).some((k) => (ab[k] ?? 10) !== 10);
        },
      });
    }

    // Step 5: Skills
    if (!hasSkills) {
      steps.push({
        id: 'skills',
        msgKey: 'onboarding.skills_hint',
        tab: 'combat',
        anchorSel: '#ob-tab-combat',
        autoComplete: () => Object.keys((c.sheet as Record<string, unknown>)?.skills as Record<string, unknown> || {}).length > 0,
      });
    }

    // Step 6: HP
    const hpMax = (sheet.hp as { max?: number } | undefined)?.max ?? 0;
    if (hpMax <= 0) {
      steps.push({
        id: 'hp',
        msgKey: 'onboarding.hp_hint',
        tab: 'vitals',
        anchorSel: '#ob-tab-vitals',
        autoComplete: () => {
          const hp = (c.sheet as Record<string, unknown>)?.hp as { max?: number } | undefined;
          return (hp?.max ?? 0) > 0;
        },
      });
    }

    // Step 7: AC / armor
    const ac = (sheet.ac as number) ?? 10;
    const armor = sheet.armor as Record<string, unknown> | undefined;
    if (ac <= 10 && !armor?.type) {
      steps.push({
        id: 'ac',
        msgKey: 'onboarding.ac_hint',
        tab: 'combat',
        anchorSel: '#ob-tab-combat',
        autoComplete: () => {
          const s = c.sheet as Record<string, unknown>;
          const a = (s.ac as number) ?? 10;
          const ar = s.armor as Record<string, unknown> | undefined;
          return a !== 10 || !!ar?.type;
        },
      });
    }

    // Step 8: Spells
    if (hasCasterClass && spells.length === 0) {
      steps.push({
        id: 'spells',
        msgKey: 'onboarding.spells_hint',
        tab: 'magic',
        anchorSel: '#ob-tab-magic',
        autoComplete: () => ((c.sheet as Record<string, unknown>)?.spells as Array<unknown> ?? []).length > 0,
      });
    }

    // Step 9: Equipment
    if (equipment.length === 0) {
      steps.push({
        id: 'equipment',
        msgKey: 'onboarding.equipment_hint',
        tab: 'loot',
        anchorSel: '#ob-tab-loot',
        autoComplete: () => ((c.sheet as Record<string, unknown>)?.equipment as Array<unknown> ?? []).length > 0,
      });
    }

    // Step 10: Background / story
    const bg = (sheet.background as { backstory?: string } | undefined)?.backstory;
    if (!bg?.trim()) {
      steps.push({
        id: 'background',
        msgKey: 'onboarding.background_hint',
        tab: 'story',
        anchorSel: '#ob-tab-story',
        autoComplete: () => {
          const b = (c.sheet as Record<string, unknown>)?.background as { backstory?: string } | undefined;
          return !!b?.backstory?.trim();
        },
      });
    }

    return steps;
  }

  let steps = $derived(stepsFor(character));

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
    currentIdx = steps.length; // all done
  }

  let currentStep = $derived(steps[currentIdx] ?? null);

  $effect(() => {
    if (!initialised) return;
    const step = steps[currentIdx];
    if (!step) return;
    if (step.autoComplete()) {
      dismissed = [...dismissed, step.id];
      save();
      advance();
    }
  });

  function isDone() {
    return currentIdx >= steps.length || !currentStep || dismissed.includes(currentStep.id);
  }

  let tooltipStyle = $state<Record<string, string>>({});

  function positionTooltip() {
    const step = currentStep;
    if (!step) return;
    const anchor = document.querySelector(step.anchorSel);
    if (!anchor) {
      tooltipStyle = { display: 'none' };
      return;
    }
    const ar = anchor.getBoundingClientRect();
    const sheet = document.querySelector('#ob-sheet');
    const sr = sheet?.getBoundingClientRect();
    if (!sr) { tooltipStyle = { display: 'none' }; return; }
    const top = ar.top - sr.top - 8;
    const left = ar.left - sr.left + ar.width / 2;
    tooltipStyle = {
      top: `${top}px`,
      left: `${left}px`,
      transform: 'translate(-50%, -100%)',
    };
  }

  let rafId: number;
  function updatePos() {
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      positionTooltip();
      rafId = 0;
    });
  }

  $effect(() => {
    if (isDone()) { tooltipStyle = {}; return; }
    updatePos();
  });
</script>

{#if initialised && steps.length > 0 && canEdit}
  {#if !isDone() && currentStep}
    {@const msg = $_('character.onboarding.' + currentStep.msgKey.replace('onboarding.', ''))}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      id="ob-tooltip"
      onscroll={updatePos}
      style={Object.entries(tooltipStyle).map(([k, v]) => `${k}:${v}`).join(';') || 'display:none'}
      class="fixed z-50 max-w-64 rounded-lg border px-4 py-3 shadow-xl pointer-events-auto"
    >
      <div class="flex items-start gap-2">
        <span class="text-[13px] leading-snug flex-1" style="color:#f4e4c1;">{msg}</span>
        <button type="button"
          onclick={dismissCurrent}
          class="shrink-0 rounded-full w-5 h-5 flex items-center justify-center hover:opacity-80"
          style="background:rgba(139,26,26,0.5);color:#f4e4c1;"
          title="{$_('common.close')}"><X size={11} /></button>
      </div>
      <div
        class="absolute w-3 h-3 rotate-45"
        style="background:#2c1810;border-right:1px solid #8b6914;border-bottom:1px solid #8b6914;left:50%;bottom:-6px;transform:translateX(-50%) rotate(45deg);"
      ></div>
    </div>
  {/if}
{/if}

<style>
  #ob-tooltip {
    background: linear-gradient(180deg, #3a2313, #2c1810);
    border-color: #8b6914;
  }
</style>
