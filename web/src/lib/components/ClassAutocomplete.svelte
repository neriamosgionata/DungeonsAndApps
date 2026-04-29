<script lang="ts">
  /**
   * Parchment-styled autocomplete for D&D class names.
   * - Suggests standard classes from $lib/dnd/classes.
   * - Accepts any free-form value (homebrew classes).
   * - Commits on blur, Enter, or suggestion click.
   */
  import { onMount } from 'svelte';
  import { DND_CLASSES } from '$lib/dnd/classes';
  import { ChevronDown } from '@lucide/svelte';

  let {
    value,
    onchange,
    placeholder = 'Class',
  }: {
    value: string;
    onchange: (v: string) => void;
    placeholder?: string;
  } = $props();

  let input: HTMLInputElement | undefined = $state();
  let wrap: HTMLDivElement | undefined = $state();
  let draft = $state('');
  let open = $state(false);
  let highlight = $state(-1);

  // keep local draft in sync when parent value changes externally
  $effect(() => { draft = value ?? ''; });

  const matches = $derived.by(() => {
    const q = draft.trim().toLowerCase();
    if (!q) return DND_CLASSES as readonly string[];
    const starts = DND_CLASSES.filter((c) => c.toLowerCase().startsWith(q));
    const contains = DND_CLASSES.filter((c) => !c.toLowerCase().startsWith(q) && c.toLowerCase().includes(q));
    return [...starts, ...contains];
  });

  const isCustom = $derived(
    draft.trim().length > 0 &&
    !DND_CLASSES.some((c) => c.toLowerCase() === draft.trim().toLowerCase())
  );

  function commit(v: string) {
    draft = v;
    onchange(v);
    open = false;
    highlight = -1;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { e.preventDefault(); open = true; highlight = Math.min(matches.length - 1, highlight + 1); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); highlight = Math.max(-1, highlight - 1); }
    else if (e.key === 'Enter') {
      e.preventDefault();
      if (open && highlight >= 0 && matches[highlight]) commit(matches[highlight]);
      else commit(draft);
    } else if (e.key === 'Escape') { open = false; highlight = -1; }
  }

  onMount(() => {
    const click = (e: MouseEvent) => {
      if (!open) return;
      if (wrap && !wrap.contains(e.target as Node)) {
        // commit whatever is in the input when clicking outside
        if (draft !== value) onchange(draft);
        open = false;
      }
    };
    window.addEventListener('mousedown', click);
    return () => window.removeEventListener('mousedown', click);
  });
</script>

<div class="wrap" bind:this={wrap}>
  <div class="field">
    <input bind:this={input} type="text" bind:value={draft} {placeholder}
      onfocus={() => (open = true)}
      oninput={() => (open = true)}
      onkeydown={onKey}
      onchange={() => onchange(draft)}
      autocomplete="off" />
    <button type="button" class="chevron" aria-label="toggle list"
      onclick={() => { open = !open; input?.focus(); }}>
      <ChevronDown size={14} />
    </button>
  </div>

  {#if open}
    <ul class="panel" role="listbox">
      {#each matches as name, i (name)}
        <li>
          <button type="button" class="row {highlight === i ? 'hi' : ''}"
            onmousedown={(e) => { e.preventDefault(); commit(name); }}
            onmouseenter={() => (highlight = i)}>
            {name}
          </button>
        </li>
      {/each}
      {#if isCustom}
        <li class="custom">
          <button type="button" class="row"
            onmousedown={(e) => { e.preventDefault(); commit(draft.trim()); }}>
            <span>Use <b>“{draft.trim()}”</b></span>
            <span class="tag">custom</span>
          </button>
        </li>
      {:else if matches.length === 0}
        <li class="empty">No matches</li>
      {/if}
    </ul>
  {/if}
</div>

<style>
  .wrap { position: relative; flex: 1; min-width: 0; }
  .field {
    display: flex; align-items: center;
    border: 1.5px solid #4e3909;
    border-radius: 0.3rem;
    background: #2c1810;
    box-shadow: inset 0 1px 3px rgba(0,0,0,0.4);
  }
  .field:focus-within {
    border-color: #c9a84c;
    box-shadow: inset 0 1px 3px rgba(0,0,0,0.4), 0 0 0 2px rgba(201,168,76,0.25);
  }
  .field input {
    flex: 1;
    background: transparent !important;
    border: 0 !important;
    padding: 0.35rem 0.6rem !important;
    font-family: 'Cinzel', serif;
    font-size: 0.85rem;
    letter-spacing: 0.03em;
    color: #f4e4c1 !important;
    outline: none;
    box-shadow: none !important;
  }
  .field input::placeholder { color: #8b6355; font-style: italic; }
  .chevron {
    padding: 0 0.5rem;
    color: #c9a84c;
    background: transparent;
  }
  .chevron:hover { color: #f4d97a; }

  .panel {
    position: absolute; top: calc(100% + 4px); left: 0; right: 0;
    z-index: 30;
    max-height: 14rem; overflow-y: auto;
    border: 1.5px solid #8b6914;
    border-radius: 0.4rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow:
      inset 0 1px 0 rgba(255,248,220,0.55),
      0 10px 24px rgba(0,0,0,0.55);
    padding: 0.25rem 0;
  }
  .row {
    width: 100%;
    display: flex; align-items: center; justify-content: space-between;
    gap: 0.5rem;
    padding: 0.35rem 0.75rem;
    text-align: left;
    background: transparent;
    font-family: 'Cinzel', serif;
    font-size: 0.8rem;
    letter-spacing: 0.03em;
    color: #2c1810;
  }
  .row:hover, .row.hi {
    background: rgba(201, 168, 76, 0.35);
    color: #1a0f08;
  }
  .custom { border-top: 1px dashed rgba(139,105,20,0.45); margin-top: 0.25rem; padding-top: 0.1rem; }
  .tag {
    font-size: 0.6rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    padding: 0.05rem 0.4rem;
    border-radius: 0.25rem;
    border: 1px solid #4e3909;
    background: linear-gradient(180deg,#c9a84c,#8b6914);
    color: #1a0f08;
  }
  .empty {
    padding: 0.5rem 0.75rem;
    font-size: 0.8rem;
    font-style: italic;
    color: #8b6355;
  }
</style>
