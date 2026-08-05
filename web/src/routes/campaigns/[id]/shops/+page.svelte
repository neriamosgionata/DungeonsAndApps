<script lang="ts">
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { Shops, Characters } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { Store, Plus, Pencil, Trash2, X, Coins } from '@lucide/svelte';

  const cid = $derived(page.params.id!);
  const campaign = useCampaign();

  type Shop = { id: string; name: string; description: string; visibility: string };
  type Item = { id: string; name: string; price_gp: number; quantity: number | null };
  let shops = $state<Array<{ shop: Shop; items: Item[] }>>([]);
  let myChars = $state<Array<{ id: string; name: string; gp: number }>>([]);
  let loading = $state(true);
  let error = $state('');
  let msg = $state('');

  let newShopName = $state('');
  let newShopDesc = $state('');
  let editItem = $state<{ shop_id: string; id?: string; name: string; price: number; qty: number | '' } | null>(null);
  let buyTarget = $state<Record<string, string>>({});
  let buyQty = $state<Record<string, number>>({});
  let sellTarget = $state<Record<string, string>>({});
  let sellQty = $state<Record<string, number>>({});

  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    load();
  });

  async function load() {
    try {
      const [res, chars] = await Promise.all([
        Shops.list(cid),
        Characters.list(cid) as Promise<Array<{ id: string; name: string; sheet?: { coin?: { gp?: number } } }>>,
      ]);
      shops = res.shops;
      myChars = chars.map((c) => ({ id: c.id, name: c.name, gp: c.sheet?.coin?.gp ?? 0 }));
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }

  async function createShop() {
    if (!newShopName.trim()) return;
    error = ''; msg = '';
    try { await Shops.create(cid, newShopName.trim(), newShopDesc.trim()); newShopName = ''; newShopDesc = ''; await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function deleteShop(id: string) {
    if (!confirm($_('shops.delete_shop_confirm'))) return;
    try { await Shops.delete(id); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function saveItem() {
    if (!editItem) return;
    error = ''; msg = '';
    try {
      const body = { name: editItem.name.trim(), price_gp: Number(editItem.price) || 0, quantity: editItem.qty === '' ? null : Number(editItem.qty) || 0 };
      if (editItem.id) await Shops.updateItem(editItem.id, body);
      else await Shops.addItem(editItem.shop_id, body);
      editItem = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function removeItem(id: string) {
    if (!confirm($_('shops.delete_item_confirm'))) return;
    try { await Shops.removeItem(id); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function doBuy(shopId: string, item: Item) {
    const charId = buyTarget[shopId];
    if (!charId) { msg = $_('shops.select_char'); return; }
    const qty = buyQty[shopId] ?? 1;
    try {
      const res = await Shops.buy(shopId, { character_id: charId, item_id: item.id, qty });
      msg = $_('shops.bought_ok').replace('{{qty}}', String(res.qty)).replace('{{item}}', res.item).replace('{{gp}}', String(res.cost_gp));
      await load();
    } catch (e) { msg = (e as Error).message; }
  }

  async function doSell(shopId: string, item: Item) {
    const charId = sellTarget[shopId];
    if (!charId) { msg = $_('shops.select_char'); return; }
    const qty = sellQty[shopId] ?? 1;
    try {
      const res = await Shops.sell(shopId, { character_id: charId, item_id: item.id, shop_id: shopId, qty });
      msg = $_('shops.sold_ok').replace('{{qty}}', String(res.qty)).replace('{{item}}', res.item).replace('{{gp}}', String(res.gold));
      await load();
    } catch (e) { msg = (e as Error).message; }
  }
</script>

<section class="mx-auto max-w-5xl px-3 sm:px-6 py-6">
  <h2 class="text-xl font-semibold flex items-center gap-2">
    <Store size={20} style="color:#c9a84c;" />
    {$_('shops.title')}
  </h2>

  {#if error}<p class="mt-3 text-sm" style="color:#8b1a1a;">{error}</p>{/if}
  {#if msg}<p class="mt-3 text-sm" style="color:#2a8a2a;">{msg}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  {#if campaign().isMaster}
    <div class="mt-4 rounded-lg border p-4" style="border-color:rgba(139,105,20,0.5);background:#2c1810;">
      <div class="flex flex-wrap gap-2">
        <input bind:value={newShopName} placeholder={$_('shops.name_ph')}
          class="flex-1 min-w-40 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
        <input bind:value={newShopDesc} placeholder={$_('shops.desc_ph')}
          class="flex-1 min-w-40 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
        <button onclick={createShop} class="rounded px-3 py-1 text-sm inline-flex items-center gap-1" style="background:#8b6914;color:#f4e4c1;">
          <Plus size={13} /> {$_('shops.create')}
        </button>
      </div>
    </div>
  {/if}

  <div class="mt-5 grid gap-4 md:grid-cols-2">
    {#each shops as { shop, items } (shop.id)}
      <article class="rounded-lg border p-4" style="border-color:rgba(139,105,20,0.4);background:#3a2313;">
        <div class="flex items-start justify-between gap-2">
          <div>
            <h3 class="font-semibold" style="color:#f4e4c1;">{shop.name}</h3>
            {#if shop.description}
              <p class="mt-1 text-xs" style="color:#c2a178;">{shop.description}</p>
            {/if}
          </div>
          {#if campaign().isMaster}
            <div class="flex gap-1 shrink-0">
              <button onclick={() => deleteShop(shop.id)} class="icon-btn" style="color:#8b1a1a;" title={$_('common.delete')}><Trash2 size={13} /></button>
            </div>
          {/if}
        </div>

        {#if campaign().isMaster}
          <button onclick={() => editItem = { shop_id: shop.id, name: '', price: 0, qty: '' }}
            class="mt-3 rounded px-2 py-1 text-xs inline-flex items-center gap-1" style="background:#8b6914;color:#f4e4c1;">
            <Plus size={11} /> {$_('shops.add_item')}
          </button>
        {/if}

        {#if editItem && editItem.shop_id === shop.id}
          <div class="mt-2 flex flex-wrap gap-1 items-center text-xs">
            <input bind:value={editItem.name} placeholder={$_('shops.item_name_ph')} class="flex-1 min-w-32 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            <input type="number" min="0" bind:value={editItem.price} placeholder="gp" class="w-20 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            <input type="number" min="0" bind:value={editItem.qty} placeholder="∞" title={$_('shops.qty_inf')} class="w-16 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            <button onclick={saveItem} class="rounded px-2 py-1" style="background:#8b6914;color:#f4e4c1;">{ $_('common.save') }</button>
            <button onclick={() => editItem = null} class="rounded px-2 py-1" style="background:#3a2313;color:#f4e4c1;"><X size={11} /></button>
          </div>
        {/if}

        <ul class="mt-3 space-y-2">
          {#each items as it (it.id)}
            <li class="rounded border p-2 text-sm" style="border-color:rgba(139,105,20,0.3);">
              <div class="flex items-center justify-between gap-2">
                <span style="color:#f4e4c1;">{it.name}</span>
                <span class="inline-flex items-center gap-1 text-xs" style="color:#c9a84c;"><Coins size={11} /> {it.price_gp} gp</span>
              </div>
              <div class="mt-1 flex flex-wrap items-center gap-1 text-xs">
                <span style="color:#8b6355;">{it.quantity === null ? $_('shops.unlimited') : $_('shops.stock').replace('{{n}}', String(it.quantity))}</span>
                {#if campaign().isMaster}
                  <button onclick={() => editItem = { shop_id: shop.id, id: it.id, name: it.name, price: it.price_gp, qty: it.quantity ?? '' }} class="underline" style="color:#a6855c;">{ $_('shops.edit') }</button>
                  <button onclick={() => removeItem(it.id)} class="underline" style="color:#8b1a1a;">{ $_('shops.remove') }</button>
                {:else}
                  <select value={buyTarget[shop.id] ?? ''} onchange={(e) => buyTarget[shop.id] = (e.currentTarget as HTMLSelectElement).value} class="rounded bg-neutral-900 border border-neutral-700 px-1 py-0.5">
                    <option value="">{ $_('shops.buyer') }</option>
                    {#each myChars as c (c.id)}
                      <option value={c.id}>{c.name} ({c.gp} gp)</option>
                    {/each}
                  </select>
                  <input type="number" min="1" value={buyQty[shop.id] ?? 1} onchange={(e) => buyQty[shop.id] = Math.max(1, +(e.currentTarget as HTMLInputElement).value)}
                    class="w-14 rounded bg-neutral-900 border border-neutral-700 px-1 py-0.5" />
                  <button onclick={() => doBuy(shop.id, it)} class="rounded px-2 py-0.5" style="background:#8b6914;color:#f4e4c1;">{ $_('shops.buy') }</button>
                  <button onclick={() => doSell(shop.id, it)} class="rounded px-2 py-0.5" style="background:#3a2313;color:#c9a84c;">{ $_('shops.sell') }</button>
                {/if}
              </div>
            </li>
          {:else}
            <li class="text-xs italic" style="color:#8b6355;">{$_('shops.empty_items')}</li>
          {/each}
        </ul>
      </article>
    {:else}
      <p class="italic text-sm col-span-2" style="color:#8b6355;">{$_('shops.empty')}</p>
    {/each}
  </div>
</section>
