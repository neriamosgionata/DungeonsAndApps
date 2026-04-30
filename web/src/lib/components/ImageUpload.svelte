<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { browser } from '$app/environment';
  import { auth } from '$lib/stores/auth.svelte';
  import { Upload, X, Image as ImageIcon } from '@lucide/svelte';

  let {
    value = $bindable<string | null>(null),
    kind = 'misc',
    size = 72,
    label = '',
    onchange,
  }: {
    value?: string | null;
    kind?: string;
    size?: number;
    label?: string;
    onchange?: (url: string | null) => void;
  } = $props();

  function uploadBase(): string {
    if (import.meta.env.PUBLIC_API_URL) return import.meta.env.PUBLIC_API_URL as string;
    if (browser) return `${window.location.protocol}//${window.location.hostname}:8080`;
    return 'http://localhost:8080';
  }
  const BASE = uploadBase();

  let fileInput: HTMLInputElement | undefined = $state();
  let busy = $state(false);
  let error = $state('');

  async function onPick(e: Event) {
    const f = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!f) return;
    error = ''; busy = true;
    try {
      const fd = new FormData();
      fd.append('kind', kind);
      fd.append('file', f);
      const res = await fetch(`${BASE}/api/v1/uploads/file`, {
        method: 'POST',
        headers: auth.token ? { authorization: `Bearer ${auth.token}` } : {},
        body: fd,
      });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body?.error?.message || res.statusText);
      }
      const out = await res.json();
      value = out.url as string;
      onchange?.(value);
    } catch (e) {
      error = (e as Error).message;
    } finally { busy = false; }
  }

  function clear() { value = null; onchange?.(null); }
</script>

<div class="img-upload">
  {#if label}<span class="lbl">{label}</span>{/if}
  <div class="box" style="width: {size}px; height: {size}px;">
    {#if value}
      <img src={value} alt="" />
      <button type="button" class="x" onclick={clear} aria-label={$_('upload.remove')}><X size={12} /></button>
    {:else}
      <button type="button" class="pick" onclick={() => fileInput?.click()} disabled={busy}
        aria-label={busy ? $_('upload.uploading') : $_('upload.upload_image')}>
        {#if busy}<span class="dots">…</span>{:else}<Upload size={18} /><ImageIcon size={14} class="ghost" />{/if}
      </button>
    {/if}
  </div>
  <input bind:this={fileInput} type="file" accept="image/*" onchange={onPick} hidden />
  {#if error}<p class="err">{error}</p>{/if}
</div>

<style>
  .img-upload { display: inline-flex; flex-direction: column; gap: 0.25rem; align-items: flex-start; }
  .lbl {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.15em;
    font-family: 'Cinzel', serif;
    color: #8b6914;
  }
  .box {
    position: relative;
    border-radius: 9999px;
    border: 1.5px solid #4e3909;
    background: radial-gradient(circle at 35% 30%, #d4b896 0%, #8b6355 70%);
    overflow: hidden;
    box-shadow: inset 0 1px 0 rgba(255, 248, 220, 0.4), 0 2px 6px rgba(0,0,0,0.5);
  }
  .box img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .pick {
    position: absolute; inset: 0;
    display: grid; place-items: center;
    color: #1a0f08;
    background: transparent;
  }
  .pick:hover { background: radial-gradient(circle at 35% 30%, #e8d5a3 0%, #a6855c 70%); }
  .x {
    position: absolute; top: 2px; right: 2px;
    width: 18px; height: 18px;
    border-radius: 9999px;
    background: rgba(139, 26, 26, 0.9);
    color: #f4e4c1;
    display: grid; place-items: center;
    border: 1px solid #4e3909;
  }
  .dots { font-size: 1.5rem; color: #1a0f08; }
  .err { color: #c95a5a; font-size: 0.7rem; }
</style>
