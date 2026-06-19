<script lang="ts">
  import { Dice5 } from '@lucide/svelte';
  import { _ } from 'svelte-i18n';
  import type { Combatant, Character } from '$lib/types';

  let {
    myPending,
    partyChars,
    rolling,
    initBonus,
    onRoll,
  }: {
    myPending: Combatant[];
    partyChars: Character[];
    rolling: Record<string, boolean>;
    initBonus: (sheet: Record<string, unknown>) => number;
    onRoll: (c: Combatant) => void;
  } = $props();
</script>

{#if myPending.length}
  <section class="my-rolls">
    <header class="my-rolls-head"><Dice5 size={14} /> <span>{$_('initiative.my_pending')}</span></header>
    <ul>
      {#each myPending as c (c.id)}
        {@const ch = partyChars.find((p) => p.id === c.character_id)}
        {@const sh = (ch?.sheet ?? {}) as Record<string, unknown>}
        {@const bonus = initBonus(sh)}
        <li class="my-roll">
          <span class="my-roll-name">{c.display_name}</span>
          <span class="my-roll-bonus">init {bonus >= 0 ? `+${bonus}` : bonus}</span>
          <button onclick={() => onRoll(c)} disabled={rolling[c.character_id as string]} class="my-roll-btn">
            <Dice5 size={14} />
            {rolling[c.character_id as string] ? '…' : `1d20${bonus >= 0 ? '+' : ''}${bonus}`}
          </button>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .my-rolls {
    margin-top: 1rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.45rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: 0 4px 10px rgba(0,0,0,0.25);
    overflow: hidden;
  }
  .my-rolls-head {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.55rem 0.9rem;
    border-bottom: 1px dashed rgba(139,105,20,0.4);
    font-family: 'IM Fell English SC', serif;
    font-size: 0.8rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #6d510f;
  }
  .my-rolls ul { margin: 0; padding: 0; list-style: none; }
  .my-roll {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.55rem 0.9rem;
    border-top: 1px dashed rgba(139,105,20,0.2);
    color: #2c1810;
  }
  .my-roll:first-child { border-top: 0; }
  .my-roll-name { font-family: 'Cinzel', serif; font-weight: 700; flex: 1; }
  .my-roll-bonus {
    font-family: 'Special Elite', monospace;
    font-size: 0.78rem;
    color: #6d510f;
  }
  .my-roll-btn {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 0.35rem 0.75rem;
    border-radius: 0.35rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    border: 1px solid #4e3909;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.35);
  }
  .my-roll-btn:hover { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }
  .my-roll-btn:disabled { opacity: 0.5; }
</style>
