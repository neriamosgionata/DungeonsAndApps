<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Maps } from '$lib/api/resources';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { X } from '@lucide/svelte';

  const campaign = useCampaign();
  const cid = $derived(page.params.id!);
  let maps = $state<Record<string, unknown>[]>([]);
  let pins = $state<Record<string, unknown>[]>([]);
  let selected = $state<string | null>(null);
  let error = $state('');
  let name = $state('');
  let mapImage = $state<string | null>(null);

  // new pin modal state
  let pinModal = $state<null | { x: number; y: number }>(null);
  let newPinLabel = $state('');
  let newPinIcon  = $state<string | null>(null);
  let newPinIsParty = $state(false);

  async function load() {
    try {
      maps = await Maps.list(cid);
      if (!selected && maps.length) selected = maps[0].id as string;
      if (selected) pins = await Maps.pins.list(selected);
    } catch (e) { error = (e as Error).message; }
  }
  onMount(load);

  $effect(() => { if (selected) Maps.pins.list(selected).then((p) => pins = p).catch(() => {}); });

  async function createMap(close: () => void) {
    try {
      const m = await Maps.create(cid, { name, image_key: mapImage, visibility: 'players' });
      selected = m.id as string;
      name = ''; mapImage = null;
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function uploadExistingMap(mapId: string, url: string | null) {
    try {
      await Maps.update(mapId, { image_key: url });
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  const currentMap = $derived(maps.find((m) => m.id === selected));

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

  // ---- drag to move (master only) ----
  let mapEl: HTMLDivElement | undefined = $state();
  let dragId = $state<string | null>(null);
  let dragOffset = { dx: 0, dy: 0 };
  let justDragged = false; // suppress click-to-add immediately after a drag

  function startDrag(ev: PointerEvent, p: Record<string, unknown>) {
    if (!campaign().isMaster || !mapEl) return;
    ev.stopPropagation();
    ev.preventDefault();
    dragId = p.id as string;
    const r = mapEl.getBoundingClientRect();
    const px = ((p.x as number) / 100) * r.width  + r.left;
    const py = ((p.y as number) / 100) * r.height + r.top;
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
      try {
        await Maps.pins.update(id, { x: moved.x as number, y: moved.y as number });
      } catch (e) {
        error = (e as Error).message;
        if (selected) pins = await Maps.pins.list(selected);
      }
    }
  }

  function mapClick(ev: MouseEvent) {
    if (justDragged) return;
    openPin(ev);
  }
</script>

<section class="mx-auto max-w-[min(96rem,100%)] px-6 py-6">
  <div class="flex items-center justify-between gap-4">
    <h2 class="text-xl font-semibold">{$_('map.title')}</h2>
    {#if campaign().isMaster}
      <CollapsibleAdd label={$_('map.new')} title={$_('map.new')} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); createMap(close); }} class="space-y-3">
            <ImageUpload bind:value={mapImage} kind="map" size={96} label={$_('map.image')} />
            <input required placeholder={$_('map.name_ph')} bind:value={name}
              class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <div class="flex justify-end">
              <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
            </div>
          </form>
        {/snippet}
      </CollapsibleAdd>
    {/if}
  </div>
  {#if error}<p class="mt-2 text-sm text-red-400">{error}</p>{/if}

  {#if maps.length}
    <div class="mt-4 flex gap-2 flex-wrap">
      {#each maps as m (m.id)}
        <button onclick={() => selected = m.id as string}
          class="rounded-md px-3 py-1 text-sm {selected === m.id ? 'bg-violet-600 text-white' : 'bg-neutral-800 text-neutral-300'}">
          {m.name}
        </button>
      {/each}
    </div>

    {#if selected}
      {#if campaign().isMaster && currentMap}
        <div class="mt-4 flex items-center gap-3 rounded-lg border border-neutral-800 bg-neutral-900 px-3 py-2">
          <span class="text-xs uppercase tracking-widest font-display" style="color:#8b6914;">{$_('map.image')}</span>
          <ImageUpload
            value={(currentMap.image_key as string | null) ?? null}
            kind="map" size={40}
            onchange={(url) => uploadExistingMap(currentMap.id as string, url)} />
          <span class="text-xs text-neutral-400">{$_('map.image_hint')}</span>
        </div>
      {/if}

      <div role="presentation" bind:this={mapEl} onclick={mapClick}
        onpointermove={onDragMove} onpointerup={endDrag} onpointercancel={endDrag}
        class="mt-4 relative w-full rounded-lg border border-neutral-800 bg-neutral-900 overflow-hidden select-none {campaign().isMaster ? 'cursor-crosshair' : ''}"
        style="height: min(85vh, calc(100vw - 3rem));">
        {#if currentMap?.image_key}
          <img src={currentMap.image_key as string} alt="" draggable="false"
            class="absolute inset-0 w-full h-full object-contain pointer-events-none" />
        {/if}
        {#each pins as p (p.id)}
          {@const dragging = dragId === p.id}
          <div class="absolute -translate-x-1/2 -translate-y-1/2 group"
               style="left: {p.x}%; top: {p.y}%;">
            <div class="flex flex-col items-center">
              {#if p.icon_url}
                <img src={p.icon_url as string} alt="" draggable="false"
                  onpointerdown={(e) => startDrag(e, p)}
                  class="h-9 w-9 rounded-full object-cover ring-2 {p.is_party ? 'ring-amber-400' : 'ring-violet-400'} shadow-[0_2px_6px_rgba(0,0,0,0.6)] {campaign().isMaster ? 'cursor-move' : ''} {dragging ? 'scale-110' : ''}" />
              {:else}
                <span role="button" tabindex="-1" aria-label="pin" onpointerdown={(e) => startDrag(e, p)}
                  class="h-3 w-3 rounded-full {p.is_party ? 'bg-amber-400' : 'bg-violet-500'} ring-2 ring-white/20 {campaign().isMaster ? 'cursor-move' : ''} {dragging ? 'scale-125' : ''}"></span>
              {/if}
              <span class="mt-1 rounded bg-black/80 px-1.5 text-xs text-neutral-100 pointer-events-none">{p.label}</span>
              {#if campaign().isMaster}
                <button onclick={(e) => { e.stopPropagation(); removePin(p.id as string); }}
                  class="absolute -top-1 -right-1 rounded-full bg-red-600 text-white w-4 h-4 grid place-items-center opacity-0 group-hover:opacity-100"
                  aria-label="remove pin"><X size={10} /></button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
      {#if campaign().isMaster}
        <p class="mt-2 text-xs text-neutral-500">{$_('map.click_hint')}</p>
      {/if}
    {/if}
  {:else}
    <p class="mt-6 text-neutral-500">{$_('map.empty')}</p>
  {/if}
</section>

{#if pinModal}
  <div class="fixed inset-0 z-40 bg-black/60 flex items-center justify-center p-4"
    onclick={() => (pinModal = null)} onkeydown={(e) => e.key === 'Escape' && (pinModal = null)}
    role="presentation">
    <div class="w-full max-w-sm rounded-lg border border-amber-900 bg-[#241810] p-5 space-y-3"
      role="dialog" tabindex="-1"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <div class="flex items-center justify-between">
        <h3 class="font-display text-lg" style="color:#f4e4c1 !important;">{$_('map.new_pin')}</h3>
        <button onclick={() => (pinModal = null)} aria-label="close" style="color:#c9a84c;"
          class="hover:text-white"><X size={18} /></button>
      </div>
      <ImageUpload bind:value={newPinIcon} kind="pin" size={72} label={$_('map.pin_icon')} />
      <input required placeholder={$_('map.pin_label')} bind:value={newPinLabel}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
      <label class="flex items-center gap-2 text-sm" style="color:#f4e4c1;">
        <input type="checkbox" bind:checked={newPinIsParty} />
        {$_('map.pin_is_party')}
      </label>
      <div class="flex justify-end gap-2">
        <button onclick={() => (pinModal = null)}
          class="rounded-md px-4 py-2 text-sm"
          style="background:#3a2313;color:#f4e4c1;border:1px solid #6d510f;">
          {$_('common.cancel')}
        </button>
        <button onclick={savePin} class="rounded-md bg-violet-600 px-4 py-2 text-sm text-white">{$_('common.create')}</button>
      </div>
    </div>
  </div>
{/if}
