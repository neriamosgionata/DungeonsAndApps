<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Maps } from '$lib/api/resources';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { Compass, MapPin as MapPinIcon, Users as UsersIcon, X, Upload, Image as ImageIcon, Trash2, Pencil } from '@lucide/svelte';
  import type { Map, MapPin } from '$lib/types';

  const campaign = useCampaign();
  const cid = $derived(page.params.id!);
  let maps = $state<Map[]>([]);
  let pins = $state<MapPin[]>([]);
  let selected = $state<string | null>(null);
  let error = $state('');

  // create form
  let newName = $state('');
  let newImage = $state<string | null>(null);

  // rename inline
  let renaming = $state(false);

  let pinModal = $state<null | { x: number; y: number }>(null);
  let newPinLabel = $state('');
  let newPinIcon  = $state<string | null>(null);
  let newPinIsParty = $state(false);

  async function load() {
    try {
      maps = await Maps.list(cid);
      if (!selected && maps.length) selected = maps[0].id;
      if (selected) pins = await Maps.pins.list(selected);
    } catch (e) { error = (e as Error).message; }
  }
  onMount(load);

  $effect(() => { if (selected) Maps.pins.list(selected).then((p) => pins = p).catch(() => {}); });

  async function createMap(close: () => void) {
    if (!newName.trim()) return;
    try {
      const m = await Maps.create(cid, { name: newName.trim(), image_key: newImage, visibility: 'players' });
      selected = m.id;
      newName = ''; newImage = null;
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function deleteMap(m: Map) {
    if (!confirm($_('map.delete_map_confirm').replace('{{name}}', m.name))) return;
    try {
      await Maps.delete(m.id);
      if (selected === m.id) selected = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function renameMap(m: Map, name: string) {
    if (!name.trim() || name === m.name) return;
    try { await Maps.update(m.id, { name: name.trim() }); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function setMapImage(url: string | null) {
    if (!selected) return;
    try {
      if (url) await Maps.update(selected, { image_key: url });
      else await Maps.update(selected, { image_key: null });
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  const currentMap = $derived(maps.find((m) => m.id === selected) ?? null);

  function openPin(ev: MouseEvent) {
    if (!selected || !campaign().isMaster) return;
    const el = ev.currentTarget as HTMLDivElement;
    const r = el.getBoundingClientRect();
    const x = ((ev.clientX - r.left) / r.width) * 100;
    const y = ((ev.clientY - r.top) / r.height) * 100;
    pinModal = { x, y };
    newPinLabel = '';
    newPinIcon = null;
    newPinIsParty = false;
  }

  async function savePin() {
    if (!selected || !pinModal) return;
    if (!newPinLabel.trim()) return;
    await Maps.pins.create(selected, {
      label: newPinLabel.trim(),
      kind: newPinIsParty ? 'party' : 'note',
      is_party: newPinIsParty,
      x: pinModal.x, y: pinModal.y,
      icon_url: newPinIcon,
      visibility: 'players',
    });
    pinModal = null;
    pins = await Maps.pins.list(selected);
  }

  async function removePin(id: string) {
    if (!confirm($_('map.pin_delete_confirm'))) return;
    await Maps.pins.delete(id);
    if (selected) pins = await Maps.pins.list(selected);
  }

  let mapEl: HTMLDivElement | undefined = $state();
  let dragId = $state<string | null>(null);
  let dragOffset = { dx: 0, dy: 0 };
  let justDragged = false;

  function startDrag(ev: PointerEvent, p: MapPin) {
    if (!campaign().isMaster || !mapEl) return;
    ev.stopPropagation(); ev.preventDefault();
    dragId = p.id;
    const r = mapEl.getBoundingClientRect();
    const px = ((p.x) / 100) * r.width  + r.left;
    const py = ((p.y) / 100) * r.height + r.top;
    dragOffset = { dx: ev.clientX - px, dy: ev.clientY - py };
    (ev.target as Element).setPointerCapture?.(ev.pointerId);
  }

  function onDragMove(ev: PointerEvent) {
    if (!dragId || !mapEl) return;
    const r = mapEl.getBoundingClientRect();
    const x = ((ev.clientX - dragOffset.dx - r.left) / r.width) * 100;
    const y = ((ev.clientY - dragOffset.dy - r.top) / r.height) * 100;
    const nx = Math.max(0, Math.min(100, x));
    const ny = Math.max(0, Math.min(100, y));
    pins = pins.map((p) => (p.id === dragId ? { ...p, x: nx, y: ny } : p));
  }

  async function endDrag(ev: PointerEvent) {
    if (!dragId) return;
    const moved = pins.find((p) => p.id === dragId);
    const id = dragId;
    dragId = null;
    justDragged = true;
    setTimeout(() => { justDragged = false; }, 50);
    (ev.target as Element).releasePointerCapture?.(ev.pointerId);
    if (moved) {
      try { await Maps.pins.update(id, { x: moved.x, y: moved.y }); }
      catch (e) { error = (e as Error).message; if (selected) pins = await Maps.pins.list(selected); }
    }
  }

  function mapClick(ev: MouseEvent) {
    if (justDragged) return;
    openPin(ev);
  }

  const partyCount = $derived(pins.filter((p) => p.is_party).length);
  const noteCount  = $derived(pins.length - partyCount);
</script>

<section class="atlas">
  <!-- header -->
  <header class="atlas-head">
    <div class="hdr-icon"><Compass size={28} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('map.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('map.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="hdr-right">
      {#if campaign().isMaster}
        <CollapsibleAdd label={`+ ${$_('map.new')}`} title={$_('map.new')} alignEnd={true}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); createMap(close); }} class="grid gap-2">
              <input required placeholder={$_('map.name_ph')} bind:value={newName}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <ImageUpload bind:value={newImage} kind="map" size={72} label={$_('map.image')} />
              <div class="flex justify-end">
                <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      {/if}
    </div>
  </header>

  <div class="rule"></div>

  {#if error}<p class="err">{error}</p>{/if}

  {#if maps.length === 0}
    <p class="empty">{$_('map.empty')}</p>
  {:else}
    <!-- chart tabs -->
    <nav class="chart-tabs">
      {#each maps as m (m.id)}
        <button type="button"
          class="chart-tab {selected === m.id ? 'active' : ''}"
          onclick={() => selected = m.id}>
          <MapPinIcon size={12} />
          <span>{m.name}</span>
        </button>
      {/each}
    </nav>

    {#if selected && currentMap}
      <!-- chart toolbar (master only) -->
      {#if campaign().isMaster}
        <div class="chart-toolbar">
          <ImageIcon size={14} style="color:#8b6914;" />
          <span class="tb-label">{$_('map.image')}</span>
          <ImageUpload
            value={currentMap.image_key ?? null}
            kind="map" size={36}
            onchange={(url) => setMapImage(url)} />
          <span class="uploader-hint"><Upload size={11} /> {$_('map.image_hint')}</span>
          <span class="tb-spacer"></span>
          <!-- inline rename -->
          <input class="rename-input"
            value={currentMap.name}
            aria-label={$_('map.rename_map_ph')}
            onblur={(e) => renameMap(currentMap, (e.currentTarget as HTMLInputElement).value)}
            onkeydown={(e) => { if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur(); }} />
          <button class="tb-danger" title={$_('map.delete_map')} onclick={() => deleteMap(currentMap)}>
            <Trash2 size={14} />
          </button>
        </div>
      {/if}

      <!-- chart frame -->
      <div class="chart-wrap">
        <div role="presentation" bind:this={mapEl} onclick={mapClick}
          onpointermove={onDragMove} onpointerup={endDrag} onpointercancel={endDrag}
          class="chart {campaign().isMaster ? 'chart-master' : ''}">
          <span class="corner tl">❧</span>
          <span class="corner tr">❧</span>
          <span class="corner bl">❧</span>
          <span class="corner br">❧</span>
          <div class="compass-rose">
            <div class="compass-star">
              <span class="cn">{$_('map.compass')}</span>
            </div>
          </div>

          {#if currentMap?.image_key}
            <img src={currentMap.image_key} alt="" draggable="false" class="chart-img" />
          {:else}
            <div class="no-chart">
              <Compass size={36} style="color:#8b6914;opacity:0.45;" />
              <p>{$_('map.no_chart_yet')}</p>
            </div>
          {/if}

          {#each pins as p (p.id)}
            {@const dragging = dragId === p.id}
            <div class="pin-wrap {dragging ? 'dragging' : ''}"
                 style="left: {p.x}%; top: {p.y}%;">
              <div class="pin-stack">
                {#if p.icon_url}
                  <img src={p.icon_url} alt="" draggable="false"
                    onpointerdown={(e) => startDrag(e, p)}
                    class="pin-icon {p.is_party ? 'party' : 'note'} {campaign().isMaster ? 'movable' : ''}" />
                {:else}
                  <span role="button" tabindex="-1" aria-label={$_('map.new_pin')}
                    onpointerdown={(e) => startDrag(e, p)}
                    class="pin-dot {p.is_party ? 'party' : 'note'} {campaign().isMaster ? 'movable' : ''}"></span>
                {/if}
                <span class="pin-label">{p.label}</span>
                {#if campaign().isMaster}
                  <button onclick={(e) => { e.stopPropagation(); removePin(p.id); }}
                    class="pin-remove" aria-label={$_('map.remove_pin')}>
                    <X size={10} />
                  </button>
                {/if}
              </div>
            </div>
          {/each}
        </div>

        <footer class="legend">
          <span class="legend-entry"><span class="leg-dot party"></span> {$_('map.party_legend')} <b>{partyCount}</b></span>
          <span class="legend-entry"><span class="leg-dot note"></span> {$_('map.note_legend')} <b>{noteCount}</b></span>
          {#if campaign().isMaster}
            <span class="legend-hint"><MapPinIcon size={11} /> {$_('map.click_hint')}</span>
          {/if}
        </footer>
      </div>
    {:else if !campaign().isMaster}
      <p class="empty">{$_('map.empty')}</p>
    {/if}
  {/if}
</section>

{#if pinModal}
  <div class="modal-back" role="presentation"
    onclick={() => (pinModal = null)}
    onkeydown={(e) => e.key === 'Escape' && (pinModal = null)}>
    <div class="modal" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <div class="modal-head">
        <h3>{$_('map.new_pin')}</h3>
        <button onclick={() => (pinModal = null)} aria-label={$_('common.close')} class="modal-close">
          <X size={16} />
        </button>
      </div>
      <div class="modal-body">
        <ImageUpload bind:value={newPinIcon} kind="pin" size={72} label={$_('map.pin_icon')} />
        <input required placeholder={$_('map.pin_label')} bind:value={newPinLabel} class="pin-input" />
        <label class="pin-check">
          <input type="checkbox" bind:checked={newPinIsParty} />
          <UsersIcon size={12} /> {$_('map.pin_is_party')}
        </label>
      </div>
      <div class="modal-foot">
        <button onclick={() => (pinModal = null)} class="btn-cancel">{$_('common.cancel')}</button>
        <button onclick={savePin} class="btn-save">{$_('common.create')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .atlas { max-width: 100%; margin: 0 auto; padding: 1rem 1.25rem; }

  .atlas-head {
    display: grid; grid-template-columns: auto 1fr auto;
    align-items: center; gap: 1rem;
  }
  .hdr-icon, .hdr-right { display: flex; justify-content: center; }
  .hdr-center { text-align: center; }
  .hdr-title {
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    font-size: clamp(1.6rem, 3vw, 2.4rem);
    font-weight: 900; letter-spacing: 0.08em;
    color: #2c1810; line-height: 1;
  }
  .hdr-sub {
    margin-top: 0.25rem; font-family: 'Crimson Text', serif;
    font-style: italic; font-size: 0.85rem; color: #6d510f;
  }
  .fleuron { color: #8b6914; margin: 0 0.4rem; font-style: normal; }

  .rule {
    height: 3px; margin: 0.85rem 0 1rem;
    background: linear-gradient(90deg, transparent 0%, #8b6914 8%, #c9a84c 50%, #8b6914 92%, transparent 100%);
    border-top: 1px solid rgba(139,105,20,0.35);
    border-bottom: 1px solid rgba(139,105,20,0.35);
    position: relative;
  }
  .rule::before {
    content: "❦"; position: absolute; top: 50%; left: 50%;
    transform: translate(-50%,-50%); color: #6d510f;
    background: #f4e4c1; padding: 0 0.5rem; font-size: 0.9rem;
  }
  .empty { text-align: center; padding: 3rem 1rem; font-style: italic; color: #8b6355; }
  .err { color: #c95a5a; margin-top: 0.5rem; font-size: 0.85rem; }

  /* chart tabs */
  .chart-tabs { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 0.85rem; }
  .chart-tab {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 0.35rem 0.85rem;
    border-radius: 0.35rem; border: 1.5px solid #4e3909;
    background: rgba(139,105,20,0.1); color: #6d510f;
    font-family: 'Cinzel', serif; font-weight: 700;
    font-size: 0.75rem; letter-spacing: 0.06em; text-transform: uppercase;
  }
  .chart-tab:hover { background: rgba(201,168,76,0.25); color: #2c1810; }
  .chart-tab.active {
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 2px 4px rgba(0,0,0,0.45);
  }

  /* toolbar */
  .chart-toolbar {
    display: flex; align-items: center; gap: 0.65rem; flex-wrap: wrap;
    padding: 0.55rem 0.85rem; margin-bottom: 0.85rem;
    border: 1.5px solid rgba(139,105,20,0.5); border-radius: 0.35rem;
    background: rgba(244,228,193,0.85);
  }
  .tb-label {
    font-family: 'IM Fell English SC', serif; font-size: 0.75rem;
    letter-spacing: 0.12em; text-transform: uppercase; color: #6d510f;
  }
  .tb-spacer { flex: 1; }
  .uploader-hint {
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-family: 'Crimson Text', serif; font-style: italic;
    font-size: 0.78rem; color: #8b6355;
  }
  .rename-input {
    border: 0 !important; border-bottom: 1px dashed rgba(139,105,20,0.55) !important;
    background: transparent !important; color: #2c1810 !important;
    font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.82rem;
    padding: 0 0 1px !important; outline: none; min-width: 8rem;
  }
  .rename-input:focus { border-bottom-color: #c9a84c !important; }
  .tb-danger {
    padding: 0.3rem 0.5rem; border-radius: 0.3rem;
    background: rgba(139,26,26,0.12); color: #8b1a1a;
    border: 1px solid rgba(139,26,26,0.35);
  }
  .tb-danger:hover { background: rgba(139,26,26,0.25); }

  /* chart frame */
  .chart-wrap {
    border: 2px solid #8b6914; border-radius: 0.45rem;
    background:
      linear-gradient(180deg, rgba(139,105,20,0.08), transparent 45%), #241810
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='p'><feTurbulence baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 0.09  0 0 0 0 0.06  0 0 0 0 0.03  0 0 0 0.2 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: 0 10px 26px rgba(0,0,0,0.55);
  }
  .chart {
    position: relative; width: 100%;
    min-height: 20rem;
    border-radius: 0.3rem; overflow: hidden;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: inset 0 0 0 1px rgba(139,105,20,0.35), inset 0 0 60px rgba(139,105,20,0.35);
    user-select: none; margin: 0.55rem 0; width: 100%;
  }
  .chart.chart-master { cursor: crosshair; }

  .corner {
    position: absolute; font-family: 'IM Fell English SC', serif;
    font-size: 1.6rem; color: rgba(139,105,20,0.55);
    pointer-events: none; line-height: 1;
  }
  .corner.tl { top: 0.3rem; left: 0.4rem; transform: rotate(-90deg); }
  .corner.tr { top: 0.3rem; right: 0.4rem; }
  .corner.bl { bottom: 0.3rem; left: 0.4rem; transform: rotate(180deg); }
  .corner.br { bottom: 0.3rem; right: 0.4rem; transform: rotate(90deg); }

  .compass-rose {
    position: absolute; top: 0.75rem; right: 0.75rem;
    width: 3.2rem; height: 3.2rem; pointer-events: none; z-index: 2;
  }
  .compass-star {
    width: 100%; height: 100%; border-radius: 9999px;
    border: 2px solid rgba(139,105,20,0.6);
    background: radial-gradient(circle at 50%, rgba(244,228,193,0.85) 0%, rgba(244,228,193,0.25) 100%);
    display: grid; place-items: center; position: relative;
    box-shadow: 0 2px 6px rgba(0,0,0,0.25);
  }
  .compass-star::before, .compass-star::after {
    content: ""; position: absolute; left: 50%; top: 50%;
    width: 2px; height: 80%;
    background: linear-gradient(180deg, #8b6914 0%, transparent 50%, #8b6914 100%);
    transform: translate(-50%,-50%);
  }
  .compass-star::after { transform: translate(-50%,-50%) rotate(90deg); }
  .cn {
    font-family: 'IM Fell English SC', serif; font-weight: 900;
    font-size: 0.85rem; color: #4e3909;
    position: absolute; top: 0.05rem; left: 50%;
    transform: translateX(-50%);
    background: #f4e4c1; padding: 0 0.2rem; border-radius: 0.2rem; z-index: 1;
  }

  .chart-img { display: block; width: 100%; height: auto; pointer-events: none; }
  .no-chart {
    position: absolute; inset: 0; display: grid; place-items: center;
    gap: 0.6rem; font-family: 'IM Fell English SC', serif;
    font-style: italic; color: #8b6355; text-align: center;
  }
  .no-chart p { margin: 0; }

  /* pins */
  .pin-wrap { position: absolute; transform: translate(-50%,-50%); }
  .pin-wrap.dragging { z-index: 20; }
  .pin-stack { display: flex; flex-direction: column; align-items: center; position: relative; }
  .pin-icon {
    width: 2.3rem; height: 2.3rem; border-radius: 9999px;
    object-fit: cover; border: 2px solid #4e3909;
    box-shadow: 0 3px 8px rgba(0,0,0,0.55); transition: transform 0.08s;
  }
  .pin-icon.party { outline: 2px solid #c9a84c; outline-offset: 1px; }
  .pin-icon.note  { outline: 2px solid #6d510f; outline-offset: 1px; }
  .pin-icon.movable { cursor: move; }
  .pin-icon:hover { transform: scale(1.05); }
  .pin-wrap.dragging .pin-icon { transform: scale(1.1); }
  .pin-dot {
    width: 0.85rem; height: 0.85rem; border-radius: 9999px;
    border: 2px solid #4e3909;
    box-shadow: 0 2px 4px rgba(0,0,0,0.55), inset 0 1px 0 rgba(255,248,220,0.4);
    transition: transform 0.08s;
  }
  .pin-dot.party { background: radial-gradient(circle at 35% 30%, #e5c065 0%, #c9a84c 60%, #6d510f 100%); }
  .pin-dot.note  { background: radial-gradient(circle at 35% 30%, #a93535 0%, #7a2323 60%, #3a0a0a 100%); }
  .pin-dot.movable { cursor: move; }
  .pin-dot:hover { transform: scale(1.2); }
  .pin-wrap.dragging .pin-dot { transform: scale(1.25); }
  .pin-label {
    margin-top: 0.25rem; padding: 0.1rem 0.5rem;
    font-family: 'IM Fell English SC', serif; font-size: 0.7rem;
    letter-spacing: 0.08em; text-transform: uppercase;
    color: #f4e4c1; background: rgba(26,15,8,0.88);
    border: 1px solid rgba(201,168,76,0.45); border-radius: 0.25rem;
    white-space: nowrap; pointer-events: none;
    box-shadow: 0 2px 4px rgba(0,0,0,0.4);
  }
  .pin-remove {
    position: absolute; top: -0.35rem; right: -0.35rem;
    width: 1.1rem; height: 1.1rem; border-radius: 9999px;
    display: grid; place-items: center;
    background: #8b1a1a; color: #f4e4c1;
    border: 1px solid #4e0a0a; opacity: 0; transition: opacity 0.1s;
  }
  .pin-stack:hover .pin-remove { opacity: 1; }

  /* legend */
  .legend {
    display: flex; align-items: center; gap: 1rem; flex-wrap: wrap;
    padding: 0.55rem 0.85rem;
    border-top: 1px dashed rgba(201,168,76,0.35);
    background: rgba(26,15,8,0.35); color: #f4e4c1;
    font-family: 'Cinzel', serif; font-size: 0.75rem;
    letter-spacing: 0.08em; text-transform: uppercase;
  }
  .legend-entry { display: inline-flex; align-items: center; gap: 0.45rem; }
  .legend-entry b { color: #c9a84c; font-weight: 700; }
  .leg-dot { width: 0.75rem; height: 0.75rem; border-radius: 9999px; border: 1.5px solid #4e3909; }
  .leg-dot.party { background: radial-gradient(circle at 35% 30%, #e5c065, #c9a84c 60%, #6d510f); }
  .leg-dot.note  { background: radial-gradient(circle at 35% 30%, #a93535, #7a2323 60%, #3a0a0a); }
  .legend-hint {
    margin-left: auto; display: inline-flex; align-items: center; gap: 0.35rem;
    font-family: 'Crimson Text', serif; font-style: italic;
    text-transform: none; letter-spacing: 0; color: rgba(244,228,193,0.7);
  }

  /* modal */
  .modal-back {
    position: fixed; inset: 0; background: rgba(0,0,0,0.7);
    display: grid; place-items: center; z-index: 60; padding: 1rem;
  }
  .modal {
    width: min(22rem, 100%); border: 2px solid #8b6914; border-radius: 0.5rem;
    background: #f4e4c1 url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    color: #2c1810;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 18px 40px rgba(0,0,0,0.65);
  }
  .modal-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.75rem 1rem; border-bottom: 1px dashed rgba(139,105,20,0.45);
  }
  .modal-head h3 {
    margin: 0; font-family: 'IM Fell English SC', serif; color: #2c1810;
    font-size: 1.1rem; letter-spacing: 0.08em;
  }
  .modal-close {
    width: 1.75rem; height: 1.75rem; display: grid; place-items: center;
    border-radius: 9999px; background: #3a2313; color: #c9a84c;
    border: 1px solid #4e3909;
  }
  .modal-close:hover { background: #4e3909; color: #f7e2a5; }
  .modal-body { padding: 1rem; display: grid; gap: 0.65rem; }
  .pin-input {
    border: 1.5px solid #8b6914 !important; background: rgba(244,228,193,0.85) !important;
    color: #2c1810 !important; border-radius: 0.35rem !important;
    padding: 0.45rem 0.7rem !important; font-family: 'Crimson Text', serif;
  }
  .pin-input:focus { border-color: #c9a84c !important; box-shadow: 0 0 0 2px rgba(201,168,76,0.25) !important; }
  .pin-check {
    display: inline-flex; align-items: center; gap: 0.4rem;
    font-family: 'Cinzel', serif; font-size: 0.78rem;
    color: #6d510f; letter-spacing: 0.04em;
  }
  .modal-foot {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    padding: 0.75rem 1rem; border-top: 1px dashed rgba(139,105,20,0.45);
  }
  .btn-cancel {
    padding: 0.45rem 1rem; border-radius: 0.35rem;
    background: #3a2313; color: #f4e4c1; border: 1px solid #6d510f;
    font-family: 'Cinzel', serif; font-size: 0.78rem;
    letter-spacing: 0.06em; text-transform: uppercase;
  }
  .btn-save {
    padding: 0.45rem 1.1rem; border-radius: 0.35rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08; border: 1px solid #4e3909;
    font-family: 'Cinzel', serif; font-weight: 700;
    font-size: 0.78rem; letter-spacing: 0.06em; text-transform: uppercase;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.4);
  }
  .btn-save:hover { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }
</style>
