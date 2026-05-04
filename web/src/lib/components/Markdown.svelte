<script lang="ts">
  // Lightweight markdown → HTML renderer with wiki-link support.
  // Supports: **bold**, *italic*, `code`, [links](url), - lists, --- hr, [[Wiki Link]]

  let {
    text,
    onWikiLink,
    emptyLabel = '',
  }: { text?: string | null; onWikiLink?: (title: string) => void; emptyLabel?: string } = $props();

  function escapeHtml(str: string): string {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function inlineToHtml(src: string): string {
    let html = escapeHtml(src);
    // Code `text` — do first so inner * don't get processed
    html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
    // Bold **text**
    html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
    // Italic *text* (but not ** which is already handled)
    html = html.replace(/(?<!\*)\*(?!\*)(.+?)(?<!\*)\*(?!\*)/g, '<em>$1</em>');
    // Link [text](url)
    html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
    // Wiki link [[Title]]
    html = html.replace(/\[\[([^\]]+)\]\]/g, '<button type="button" class="wiki-link" data-wiki="$1">$1</button>');
    return html;
  }

  function toHtml(raw: string): string {
    const lines = raw.replace(/\r\n?/g, '\n').trim().split('\n');
    const out: string[] = [];
    let inList = false;

    const closeList = () => { if (inList) { out.push('</ul>'); inList = false; } };

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) { closeList(); continue; }
      if (/^---+$/.test(trimmed)) { closeList(); out.push('<hr />'); continue; }
      const h = trimmed.match(/^(#{1,3})\s+(.+)$/);
      if (h) {
        closeList();
        const tag = h[1].length === 1 ? 'h3' : h[1].length === 2 ? 'h4' : 'h5';
        out.push(`<${tag}>${inlineToHtml(h[2])}</${tag}>`);
        continue;
      }
      if (/^\s*[-*]\s+/.test(trimmed)) {
        if (!inList) { out.push('<ul>'); inList = true; }
        out.push(`<li>${inlineToHtml(trimmed.replace(/^\s*[-*]\s+/, ''))}</li>`);
        continue;
      }
      closeList();
      out.push(`<p>${inlineToHtml(trimmed)}</p>`);
    }
    closeList();
    return out.join('');
  }

  const html = $derived(text ? toHtml(text) : '');

  function handleClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.classList.contains('wiki-link') && target.dataset.wiki) {
      e.preventDefault();
      onWikiLink?.(target.dataset.wiki);
    }
  }
</script>

{#if !html}
  {#if emptyLabel}<p class="text-sm italic" style="color:#8b6355;">{emptyLabel}</p>{/if}
{:else}
  <div class="md" role="presentation" onclick={handleClick} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleClick(e as unknown as MouseEvent); }}>{@html html}</div>
{/if}

<style>
  .md { display: flex; flex-direction: column; gap: 0.75rem; }
  .md :global(h3), .md :global(h4), .md :global(h5) {
    font-family: 'Cinzel', serif; font-weight: 700;
    color: #8b6914; margin: 0;
  }
  .md :global(h3) { font-size: 1.1rem; letter-spacing: 0.06em; }
  .md :global(h4) { font-size: 0.95rem; letter-spacing: 0.08em; }
  .md :global(h5) { font-size: 0.8rem; letter-spacing: 0.1em; text-transform: uppercase; }
  .md :global(p) { margin: 0; line-height: 1.6; white-space: pre-wrap; word-break: break-word; }
  .md :global(ul) { margin: 0; padding-left: 1.2rem; }
  .md :global(li) { margin: 0.15rem 0; }
  .md :global(hr) { border: none; border-top: 1px solid rgba(201,168,76,0.2); margin: 0.25rem 0; }
  .md :global(code) {
    background: rgba(0,0,0,0.25); padding: 0.1rem 0.3rem; border-radius: 0.2rem;
    font-family: 'Special Elite', monospace; font-size: 0.85em; color: #c9a84c;
  }
  .md :global(a) { color: #6b8a4f; text-decoration: underline; }
  .md :global(.wiki-link) {
    background: none; border: none; padding: 0;
    color: #c9a84c; text-decoration: underline; cursor: pointer;
    font-family: inherit; font-size: inherit;
  }
  .md :global(.wiki-link:hover) { color: #e8d69a; }
</style>
