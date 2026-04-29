<script lang="ts">
  // Multi-paragraph renderer.
  // - Blank line between paragraphs → new paragraph block.
  // - Single line break inside a paragraph is preserved (rendered via CSS).
  // - Any line that starts with "# Title" or "## Title" becomes a heading,
  //   starting a new block. The body is everything until the next heading
  //   or blank line.
  // - Plain paragraphs (no heading) work unchanged.

  let {
    text,
    emptyLabel = '',
  }: { text?: string | null; emptyLabel?: string } = $props();

  type Para = { title: string | null; body: string };

  const paragraphs = $derived.by<Para[]>(() => {
    const raw = (text ?? '').replace(/\r\n?/g, '\n').trim();
    if (!raw) return [];

    const blocks: Para[] = [];
    let current: Para | null = null;

    const flush = () => {
      if (!current) return;
      const body = current.body.replace(/^\n+|\n+$/g, '');
      if (current.title || body) blocks.push({ title: current.title, body });
      current = null;
    };

    for (const line of raw.split('\n')) {
      // Accept "# Title", "## Title", or the no-space variants "#Title"/"##Title".
      const head = line.match(/^\s*#{1,2}\s*(.+?)\s*$/);
      if (head) {
        // Close the previous block, start a new titled one.
        flush();
        current = { title: head[1].trim(), body: '' };
        continue;
      }
      if (!line.trim()) {
        // Blank line ends the current block.
        flush();
        continue;
      }
      if (!current) current = { title: null, body: line };
      else current.body += (current.body ? '\n' : '') + line;
    }
    flush();
    return blocks;
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
