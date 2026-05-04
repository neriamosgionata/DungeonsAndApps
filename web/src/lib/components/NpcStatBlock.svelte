<script lang="ts">
  import { Heart, Shield, Zap, Footprints, Eye, Skull } from '@lucide/svelte';

  export interface NpcAbilities {
    str: number;
    dex: number;
    con: number;
    int: number;
    wis: number;
    cha: number;
  }
  export interface NpcHp {
    max?: number;
    current?: number;
    hit_dice?: string;
  }
  export interface NpcWeapon {
    id?: string;
    name: string;
    properties?: string;
    damage?: string;
    damage_type?: string;
    attack_bonus?: number;
    range?: string;
  }
  export interface NpcAction {
    name: string;
    description?: string;
    attack_bonus?: number;
    damage?: string;
    damage_type?: string;
    range?: string;
    recharge?: string;
  }
  export interface NpcSenses {
    darkvision?: number;
    blindsight?: number;
    truesight?: number;
    tremorsense?: number;
    passive_perception?: number;
  }
  export interface NpcStats {
    abilities?: NpcAbilities;
    ac?: number;
    hp?: NpcHp;
    speed?: number;
    saves?: Record<string, boolean>;
    skills?: Record<string, string>;
    weapons?: NpcWeapon[];
    casting?: { ability?: string; spell_attack_bonus?: number; spell_save_dc?: number };
    equipment?: Array<{ name: string; quantity?: number; type?: string }>;
    cr?: string;
    xp?: number;
    pb?: number;
    resistances?: string[];
    vulnerabilities?: string[];
    immunities?: string[];
    condition_immunities?: string[];
    senses?: NpcSenses;
    languages?: string[];
    actions?: NpcAction[];
    legendary_actions?: NpcAction[];
    reactions?: NpcAction[];
    traits?: NpcAction[];
    // legacy
    attitude?: string;
    status?: string;
  }

  type Props = {
    stats: Record<string, unknown>;
    edit?: boolean;
    onchange?: (stats: Record<string, unknown>) => void;
  };

  let { stats = $bindable(), edit = false, onchange }: Props = $props();

  // Derive typed view of stats
  const s = $derived<NpcStats>((stats ?? {}) as NpcStats);

  const ABILITIES = ['str', 'dex', 'con', 'int', 'wis', 'cha'] as const;
  const SKILLS = [
    ['athletics','str'],['acrobatics','dex'],['sleight_of_hand','dex'],['stealth','dex'],
    ['arcana','int'],['history','int'],['investigation','int'],['nature','int'],['religion','int'],
    ['animal_handling','wis'],['insight','wis'],['medicine','wis'],['perception','wis'],['survival','wis'],
    ['deception','cha'],['intimidation','cha'],['performance','cha'],['persuasion','cha'],
  ] as const;
  const SAVE_ABILITIES = ['str','dex','con','int','wis','cha'] as const;

  function mod(score: number): string {
    const m = Math.floor((score - 10) / 2);
    return m >= 0 ? `+${m}` : `${m}`;
  }

  function update(partial: Partial<NpcStats>) {
    const next = { ...s, ...partial };
    stats = next as Record<string, unknown>;
    onchange?.(stats);
  }

  function setAbility(ab: string, val: number) {
    update({ abilities: { ...(s.abilities ?? { str:10, dex:10, con:10, int:10, wis:10, cha:10 }), [ab]: val } });
  }
  function setSave(ab: string, val: boolean) {
    update({ saves: { ...(s.saves ?? {}), [ab]: val } });
  }
  function setSkill(name: string, val: string) {
    update({ skills: { ...(s.skills ?? {}), [name]: val } });
  }
  function setHp(partial: Partial<NpcHp>) {
    update({ hp: { ...(s.hp ?? {}), ...partial } });
  }

  // Reactive arrays for edit mode
  let weapons = $state<NpcWeapon[]>([]);
  let actions = $state<NpcAction[]>([]);
  let traits = $state<NpcAction[]>([]);
  let reactions = $state<NpcAction[]>([]);
  let legendaryActions = $state<NpcAction[]>([]);
  let resistances = $state('');
  let vulnerabilities = $state('');
  let immunities = $state('');
  let conditionImmunities = $state('');
  let languages = $state('');

  // Sync edit-mode local arrays from stats whenever entering edit or stats change
  $effect(() => {
    weapons = [...(s.weapons ?? [])];
    actions = [...(s.actions ?? [])];
    traits = [...(s.traits ?? [])];
    reactions = [...(s.reactions ?? [])];
    legendaryActions = [...(s.legendary_actions ?? [])];
    resistances = (s.resistances ?? []).join(', ');
    vulnerabilities = (s.vulnerabilities ?? []).join(', ');
    immunities = (s.immunities ?? []).join(', ');
    conditionImmunities = (s.condition_immunities ?? []).join(', ');
    languages = (s.languages ?? []).join(', ');
  });

  function commitLists() {
    update({
      weapons: weapons.filter(w => w.name.trim()),
      actions: actions.filter(a => a.name.trim()),
      traits: traits.filter(t => t.name.trim()),
      reactions: reactions.filter(r => r.name.trim()),
      legendary_actions: legendaryActions.filter(a => a.name.trim()),
      resistances: resistances.split(',').map(x => x.trim()).filter(Boolean),
      vulnerabilities: vulnerabilities.split(',').map(x => x.trim()).filter(Boolean),
      immunities: immunities.split(',').map(x => x.trim()).filter(Boolean),
      condition_immunities: conditionImmunities.split(',').map(x => x.trim()).filter(Boolean),
      languages: languages.split(',').map(x => x.trim()).filter(Boolean),
    });
  }
</script>

{#if edit}
  <div class="space-y-3">
    <!-- Core stats -->
    <div class="grid grid-cols-3 sm:grid-cols-6 gap-2">
      {#each ABILITIES as ab}
        <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
          {ab}
          <input type="number" value={s.abilities?.[ab] ?? 10}
            onchange={(e) => setAbility(ab, parseInt(e.currentTarget.value) || 10)}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
        </label>
      {/each}
    </div>

    <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        AC
        <input type="number" value={s.ac ?? ''}
          onchange={(e) => update({ ac: e.currentTarget.value ? parseInt(e.currentTarget.value) : undefined })}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        HP Max
        <input type="number" value={s.hp?.max ?? ''}
          onchange={(e) => setHp({ max: e.currentTarget.value ? parseInt(e.currentTarget.value) : undefined })}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        Speed
        <input type="number" value={s.speed ?? ''}
          onchange={(e) => update({ speed: e.currentTarget.value ? parseInt(e.currentTarget.value) : undefined })}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        Prof. Bonus
        <input type="number" value={s.pb ?? ''}
          onchange={(e) => update({ pb: e.currentTarget.value ? parseInt(e.currentTarget.value) : undefined })}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
    </div>

    <div class="grid grid-cols-2 gap-2">
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        CR
        <input value={s.cr ?? ''}
          onchange={(e) => update({ cr: e.currentTarget.value || undefined })}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        XP
        <input type="number" value={s.xp ?? ''}
          onchange={(e) => update({ xp: e.currentTarget.value ? parseInt(e.currentTarget.value) : undefined })}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
    </div>

    <!-- Saves -->
    <div>
      <div class="text-[11px] uppercase tracking-wider text-neutral-400 mb-1">Save Proficiencies</div>
      <div class="flex flex-wrap gap-3">
        {#each SAVE_ABILITIES as ab}
          <label class="inline-flex items-center gap-1 text-sm text-neutral-300">
            <input type="checkbox" checked={s.saves?.[ab] ?? false}
              onchange={(e) => setSave(ab, e.currentTarget.checked)} />
            {ab.toUpperCase()}
          </label>
        {/each}
      </div>
    </div>

    <!-- Skills -->
    <div>
      <div class="text-[11px] uppercase tracking-wider text-neutral-400 mb-1">Skills</div>
      <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
        {#each SKILLS as [name, ab]}
          <label class="flex items-center gap-1 text-[11px]">
            <span class="text-neutral-500 w-24 truncate">{name.replace('_',' ')}</span>
            <select value={s.skills?.[name] ?? ''}
              onchange={(e) => setSkill(name, e.currentTarget.value)}
              class="rounded bg-neutral-900 border border-neutral-700 px-1 py-0.5 text-xs text-white flex-1">
              <option value="">—</option>
              <option value="prof">Prof</option>
              <option value="expert">Expert</option>
            </select>
          </label>
        {/each}
      </div>
    </div>

    <!-- Damage / Condition -->
    <div class="grid grid-cols-2 gap-2">
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        Resistances (comma)
        <input bind:value={resistances} onchange={commitLists}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        Vulnerabilities (comma)
        <input bind:value={vulnerabilities} onchange={commitLists}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        Immunities (comma)
        <input bind:value={immunities} onchange={commitLists}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
      <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
        Condition Immunities (comma)
        <input bind:value={conditionImmunities} onchange={commitLists}
          class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
      </label>
    </div>

    <label class="flex flex-col gap-0.5 text-[11px] uppercase tracking-wider text-neutral-400">
      Languages (comma)
      <input bind:value={languages} onchange={commitLists}
        class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm text-white" />
    </label>

    <!-- Weapons -->
    <div>
      <div class="text-[11px] uppercase tracking-wider text-neutral-400 mb-1">Weapons / Attacks</div>
      {#each weapons as w, i (i)}
        <div class="grid grid-cols-[1fr_1fr_1fr_1fr_auto] gap-1 mb-1">
          <input placeholder="Name" bind:value={w.name} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <input placeholder="Damage" bind:value={w.damage} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <input placeholder="Type" bind:value={w.damage_type} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <input placeholder="Props" bind:value={w.properties} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <button type="button" onclick={() => { weapons.splice(i,1); commitLists(); }}
            class="text-red-400 text-xs">✕</button>
        </div>
      {/each}
      <button type="button" onclick={() => { weapons.push({ name:'', damage:'', damage_type:'', properties:'' }); commitLists(); }}
        class="text-xs text-violet-400 hover:text-violet-300">+ Add weapon</button>
    </div>

    <!-- Actions -->
    <div>
      <div class="text-[11px] uppercase tracking-wider text-neutral-400 mb-1">Actions</div>
      {#each actions as a, i (i)}
        <div class="grid grid-cols-[1fr_4rem_auto] gap-1 mb-1">
          <input placeholder="Name" bind:value={a.name} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <input placeholder="To hit" bind:value={a.attack_bonus} type="number" onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <button type="button" onclick={() => { actions.splice(i,1); commitLists(); }}
            class="text-red-400 text-xs">✕</button>
        </div>
        <textarea placeholder="Description" bind:value={a.description} onchange={commitLists} rows="2"
          class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white mb-2"></textarea>
      {/each}
      <button type="button" onclick={() => { actions.push({ name:'', description:'' }); commitLists(); }}
        class="text-xs text-violet-400 hover:text-violet-300">+ Add action</button>
    </div>

    <!-- Traits -->
    <div>
      <div class="text-[11px] uppercase tracking-wider text-neutral-400 mb-1">Traits</div>
      {#each traits as t, i (i)}
        <div class="grid grid-cols-[1fr_auto] gap-1 mb-1">
          <input placeholder="Name" bind:value={t.name} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <button type="button" onclick={() => { traits.splice(i,1); commitLists(); }}
            class="text-red-400 text-xs">✕</button>
        </div>
        <textarea placeholder="Description" bind:value={t.description} onchange={commitLists} rows="2"
          class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white mb-2"></textarea>
      {/each}
      <button type="button" onclick={() => { traits.push({ name:'', description:'' }); commitLists(); }}
        class="text-xs text-violet-400 hover:text-violet-300">+ Add trait</button>
    </div>

    <!-- Reactions -->
    <div>
      <div class="text-[11px] uppercase tracking-wider text-neutral-400 mb-1">Reactions</div>
      {#each reactions as r, i (i)}
        <div class="grid grid-cols-[1fr_auto] gap-1 mb-1">
          <input placeholder="Name" bind:value={r.name} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <button type="button" onclick={() => { reactions.splice(i,1); commitLists(); }}
            class="text-red-400 text-xs">✕</button>
        </div>
        <textarea placeholder="Description" bind:value={r.description} onchange={commitLists} rows="2"
          class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white mb-2"></textarea>
      {/each}
      <button type="button" onclick={() => { reactions.push({ name:'', description:'' }); commitLists(); }}
        class="text-xs text-violet-400 hover:text-violet-300">+ Add reaction</button>
    </div>

    <!-- Legendary Actions -->
    <div>
      <div class="text-[11px] uppercase tracking-wider text-neutral-400 mb-1">Legendary Actions</div>
      {#each legendaryActions as a, i (i)}
        <div class="grid grid-cols-[1fr_auto] gap-1 mb-1">
          <input placeholder="Name" bind:value={a.name} onchange={commitLists}
            class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white" />
          <button type="button" onclick={() => { legendaryActions.splice(i,1); commitLists(); }}
            class="text-red-400 text-xs">✕</button>
        </div>
        <textarea placeholder="Description" bind:value={a.description} onchange={commitLists} rows="2"
          class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs text-white mb-2"></textarea>
      {/each}
      <button type="button" onclick={() => { legendaryActions.push({ name:'', description:'' }); commitLists(); }}
        class="text-xs text-violet-400 hover:text-violet-300">+ Add legendary action</button>
    </div>
  </div>
{:else}
  <!-- Display mode -->
  <div class="stat-block space-y-2">
    {#if s.ac || s.hp?.max || s.speed}
      <div class="flex flex-wrap gap-2 text-xs">
        {#if s.ac}<span class="inline-flex items-center gap-1 tag-pill bg-amber-900/30 text-amber-100"><Shield size={11}/> AC {s.ac}</span>{/if}
        {#if s.hp?.max}<span class="inline-flex items-center gap-1 tag-pill bg-red-900/30 text-red-100"><Heart size={11}/> HP {s.hp.max}</span>{/if}
        {#if s.speed}<span class="inline-flex items-center gap-1 tag-pill bg-blue-900/30 text-blue-100"><Footprints size={11}/> {s.speed} ft</span>{/if}
        {#if s.pb}<span class="inline-flex items-center gap-1 tag-pill bg-violet-900/30 text-violet-100"><Zap size={11}/> +{s.pb}</span>{/if}
        {#if s.cr}<span class="inline-flex items-center gap-1 tag-pill bg-neutral-800 text-neutral-200"><Skull size={11}/> CR {s.cr}</span>{/if}
        {#if s.xp}<span class="inline-flex items-center gap-1 tag-pill bg-neutral-800 text-neutral-200">{s.xp} XP</span>{/if}
      </div>
    {/if}

    {#if s.abilities}
      <div class="abilities-row">
        {#each ABILITIES as ab}
          {@const score = s.abilities?.[ab] ?? 10}
          <div class="ability-box">
            <div class="ability-name">{ab.toUpperCase()}</div>
            <div class="ability-score">{score}</div>
            <div class="ability-mod">{mod(score)}</div>
          </div>
        {/each}
      </div>
    {/if}

    {#if s.saves && Object.values(s.saves).some(Boolean)}
      <div class="text-xs text-neutral-300">
        <strong>Saves</strong>:
        {#each SAVE_ABILITIES as ab}
          {#if s.saves?.[ab]}
            {ab.toUpperCase()} {mod((s.abilities?.[ab] ?? 10) + (s.pb ?? 2))}{' '}
          {/if}
        {/each}
      </div>
    {/if}

    {#if s.skills && Object.keys(s.skills).length}
      <div class="text-xs text-neutral-300">
        <strong>Skills</strong>:
        {#each SKILLS as [name, ab]}
          {#if s.skills?.[name]}
            {name.replace('_',' ')} {mod((s.abilities?.[ab] ?? 10) + (s.skills[name] === 'expert' ? (s.pb ?? 2) * 2 : s.pb ?? 2))}{' '}
          {/if}
        {/each}
      </div>
    {/if}

    {#if s.resistances?.length}<div class="text-xs text-neutral-300"><strong>Resistances</strong>: {s.resistances.join(', ')}</div>{/if}
    {#if s.vulnerabilities?.length}<div class="text-xs text-neutral-300"><strong>Vulnerabilities</strong>: {s.vulnerabilities.join(', ')}</div>{/if}
    {#if s.immunities?.length}<div class="text-xs text-neutral-300"><strong>Immunities</strong>: {s.immunities.join(', ')}</div>{/if}
    {#if s.condition_immunities?.length}<div class="text-xs text-neutral-300"><strong>Condition Immunities</strong>: {s.condition_immunities.join(', ')}</div>{/if}
    {#if s.senses}
      <div class="text-xs text-neutral-300">
        <strong>Senses</strong>:
        {#if s.senses.darkvision}darkvision {s.senses.darkvision} ft{/if}
        {#if s.senses.blindsight}blindsight {s.senses.blindsight} ft{/if}
        {#if s.senses.truesight}truesight {s.senses.truesight} ft{/if}
        {#if s.senses.tremorsense}tremorsense {s.senses.tremorsense} ft{/if}
        {#if s.senses.passive_perception}passive Perception {s.senses.passive_perception}{/if}
      </div>
    {/if}
    {#if s.languages?.length}<div class="text-xs text-neutral-300"><strong>Languages</strong>: {s.languages.join(', ')}</div>{/if}

    {#if s.weapons?.length}
      <div class="text-xs text-neutral-300">
        <strong>Attacks</strong>:
        {#each s.weapons as w}
          <span class="inline-block mr-2">
            {w.name} {w.attack_bonus ? `+${w.attack_bonus}` : ''} ({w.damage}{w.damage_type ? ` ${w.damage_type}` : ''})
          </span>
        {/each}
      </div>
    {/if}

    {#if s.traits?.length}
      <div class="space-y-1">
        {#each s.traits as t}
          <div class="text-xs text-neutral-300">
            <strong class="text-amber-200/80">{t.name}.</strong>
            <span class="text-neutral-400">{t.description}</span>
          </div>
        {/each}
      </div>
    {/if}

    {#if s.actions?.length}
      <div class="space-y-1">
        <div class="text-[10px] uppercase tracking-widest text-amber-200/60 border-b border-amber-200/20 pb-0.5">Actions</div>
        {#each s.actions as a}
          <div class="text-xs text-neutral-300">
            <strong class="text-amber-200/80">{a.name}.</strong>
            {#if a.attack_bonus}<em>+{a.attack_bonus} to hit</em>.{/if}
            {#if a.damage}<em>{a.damage} {a.damage_type ?? ''}</em>.{/if}
            <span class="text-neutral-400">{a.description}</span>
          </div>
        {/each}
      </div>
    {/if}

    {#if s.reactions?.length}
      <div class="space-y-1">
        <div class="text-[10px] uppercase tracking-widest text-amber-200/60 border-b border-amber-200/20 pb-0.5">Reactions</div>
        {#each s.reactions as r}
          <div class="text-xs text-neutral-300">
            <strong class="text-amber-200/80">{r.name}.</strong>
            <span class="text-neutral-400">{r.description}</span>
          </div>
        {/each}
      </div>
    {/if}

    {#if s.legendary_actions?.length}
      <div class="space-y-1">
        <div class="text-[10px] uppercase tracking-widest text-amber-200/60 border-b border-amber-200/20 pb-0.5">Legendary Actions</div>
        {#each s.legendary_actions as a}
          <div class="text-xs text-neutral-300">
            <strong class="text-amber-200/80">{a.name}.</strong>
            <span class="text-neutral-400">{a.description}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .tag-pill {
    padding: 0.15rem 0.5rem;
    border-radius: 0.25rem;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    letter-spacing: 0.06em;
    border: 1px solid rgba(255,255,255,0.08);
  }
  .abilities-row {
    display: flex; gap: 0.4rem; flex-wrap: wrap;
  }
  .ability-box {
    display: flex; flex-direction: column; align-items: center;
    min-width: 2.8rem;
    padding: 0.3rem 0.4rem;
    border-radius: 0.35rem;
    background: rgba(139,105,20,0.12);
    border: 1px solid rgba(139,105,20,0.25);
  }
  .ability-name {
    font-family: 'Cinzel', serif; font-size: 0.55rem; letter-spacing: 0.1em;
    color: #8b6914; text-transform: uppercase;
  }
  .ability-score {
    font-family: 'Cinzel', serif; font-size: 0.9rem; font-weight: 800;
    color: #2c1810; line-height: 1;
  }
  .ability-mod {
    font-family: 'Crimson Text', serif; font-size: 0.7rem;
    color: #5c3d2e;
  }
</style>
