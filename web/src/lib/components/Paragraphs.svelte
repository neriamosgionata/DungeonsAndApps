<script lang="ts">
  // Multi-paragraph renderer.
  // Paragraphs separated by blank lines.
  // A paragraph may start with "# Title" (or "## Title") on its own first line
  // to render a heading above its body. Plain paragraphs have no heading.

  let {
    text,
    emptyLabel = '',
  }: { text?: string | null; emptyLabel?: string } = $props();

  type Para = { title: string | null; body: string };

  const paragraphs = $derived.by<Para[]>(() => {
    const raw = (text ?? '').trim();
    if (!raw) return [];
    return raw
      .split(/\n\s*\n+/)
      .map((p) => p.replace(/^\n+|\n+$/g, ''))
      .map((block) => {
        const m = block.match(/^\s*#{1,2}\s+(.+?)\s*(?:\n([\s\S]*))?$/);
        if (m) return { title: m[1].trim(), body: (m[2] ?? '').trim() };
        return { title: null, body: block };
      });
  });
</script>

{#if paragraphs.length === 0}
  {#if emptyLabel}<p class="text-sm italic" style="color:#8b6355;">{emptyLabel}</p>{/if}
{:else}
  <div class="paragraphs">
    {#each paragraphs as p, i (i)}
      <section class="para">
        {#if p.title}<h5>{p.title}</h5>{/if}
        {#if p.body}<p>{p.body}</p>{/if}
      </section>
    {/each}
  </div>
{/if}

<style>
  .paragraphs { display: flex; flex-direction: column; gap: 1rem; }
  .para h5 {
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.85rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #8b6914;
    margin-bottom: 0.35rem;
  }
  .para p {
    white-space: pre-wrap;
    line-height: 1.6;
    margin: 0;
  }
</style>
