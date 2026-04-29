<script lang="ts">
  import { debug } from '$lib/debug.svelte';
  import { onMount } from 'svelte';

  onMount(() => {
    debug.attach();
    const kb = (e: KeyboardEvent) => {
      if (e.key === '`' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); debug.toggle(); }
    };
    window.addEventListener('keydown', kb);
    return () => window.removeEventListener('keydown', kb);
  });

  const counts = $derived({
    error: debug.entries.filter((e) => e.level === 'error').length,
    warn:  debug.entries.filter((e) => e.level === 'warn').length,
  });

  const pill = (l: string) => ({
    error: 'bg-red-600',
    warn:  'bg-amber-500',
    info:  'bg-neutral-600',
    api:   'bg-sky-600',
    ws:    'bg-violet-600',
  }[l] ?? 'bg-neutral-600');

  function time(ts: number) {
    return new Date(ts).toISOString().slice(11, 23);
  }
</script>

{#if debug.enabled}
  <button onclick={() => debug.toggle()} aria-label="toggle debug"
    class="fixed bottom-3 right-3 z-50 flex items-center gap-1.5 rounded-full bg-black/80 px-3 py-1.5 text-xs font-mono text-neutral-200 shadow-lg ring-1 ring-white/10 hover:bg-black">
    🪲
    {#if counts.error > 0}<span class="rounded-full bg-red-600 px-1.5 text-white">{counts.error}</span>{/if}
    {#if counts.warn  > 0}<span class="rounded-full bg-amber-500 px-1.5 text-black">{counts.warn}</span>{/if}
  </button>
{/if}

{#if debug.open}
  <aside class="fixed inset-x-3 bottom-14 top-3 z-50 flex flex-col rounded-lg bg-black/95 ring-1 ring-white/10 shadow-2xl text-xs font-mono text-neutral-200 sm:left-auto sm:right-3 sm:w-[32rem]">
    <header class="flex items-center gap-2 border-b border-white/10 px-3 py-2">
      <span class="font-semibold">autodebug</span>
      <span class="text-neutral-500">· {debug.entries.length} events</span>
      <button onclick={() => debug.clear()} class="ml-auto rounded px-2 py-0.5 hover:bg-white/10">clear</button>
      <button onclick={() => debug.toggle()} class="rounded px-2 py-0.5 hover:bg-white/10">×</button>
    </header>
    <div class="flex-1 overflow-y-auto p-2 space-y-1">
      {#each debug.entries as e (e.id)}
        <div class="rounded px-2 py-1 hover:bg-white/5">
          <div class="flex items-start gap-2">
            <span class="rounded px-1.5 text-white {pill(e.level)}">{e.level}</span>
            <span class="text-neutral-500">{time(e.ts)}</span>
            <span class="break-all">{e.msg}</span>
          </div>
          {#if e.detail}
            <pre class="mt-1 ml-6 text-[10px] text-neutral-500 whitespace-pre-wrap break-all">{JSON.stringify(e.detail, null, 2)}</pre>
          {/if}
        </div>
      {/each}
      {#if debug.entries.length === 0}
        <p class="p-4 text-neutral-500">No events yet. Errors, console.warn, fetch + WS all captured.</p>
      {/if}
    </div>
    <footer class="border-t border-white/10 px-3 py-1.5 text-[10px] text-neutral-500">
      Toggle: ⌘/Ctrl + `
    </footer>
  </aside>
{/if}
