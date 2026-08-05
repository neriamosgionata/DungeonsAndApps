<script lang="ts">
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { Campaigns } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import type { Calendar } from '$lib/types';
  import { CalendarDays, ChevronLeft, ChevronRight, Save } from '@lucide/svelte';

  const cid = $derived(page.params.id!);
  const campaign = useCampaign();

  let cal = $state<Calendar | null>(null);
  let error = $state('');
  let loading = $state(true);
  let busy = $state(false);
  let editing = $state(false);
  let editNotes = $state('');
  let editMonths = $state('');
  let editDaysPerMonth = $state(30);

  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    load();
  });

  async function load() {
    try {
      const c = await Campaigns.calendar(cid);
      cal = c;
      editNotes = c.notes;
      editMonths = (c.months ?? []).join('\n');
      editDaysPerMonth = c.days_per_month;
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }

  async function advance(days: number) {
    busy = true; error = '';
    try { cal = await Campaigns.calendarAdvance(cid, days); }
    catch (e) { error = (e as Error).message; } finally { busy = false; }
  }

  async function saveSettings() {
    busy = true; error = '';
    try {
      cal = await Campaigns.calendarUpdate(cid, {
        notes: editNotes,
        months: editMonths.split('\n').map((m) => m.trim()).filter(Boolean),
        days_per_month: Math.max(1, editDaysPerMonth),
      });
      editing = false;
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }

  const monthName = $derived(cal ? (cal.months[cal.month - 1] ?? String(cal.month)) : '');
  const weekday = $derived(cal ? (cal.weekdays[((cal.day - 1) % (cal.weekdays.length || 7))] ?? '') : '');
</script>

<section class="mx-auto max-w-3xl px-3 sm:px-6 py-6">
  <h2 class="text-xl font-semibold flex items-center gap-2">
    <CalendarDays size={20} style="color:#c9a84c;" />
    {$_('calendar.title')}
  </h2>

  {#if error}<p class="mt-3 text-sm text-red-600">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  {#if cal}
    <div class="mt-6 rounded-lg border p-8 text-center" style="background:#f4e4c1 url(&quot;data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>&quot;);border-color:#8b6914;color:#2c1810;">
      <p class="text-xs uppercase tracking-[0.3em]" style="color:#8b6914;">{weekday}</p>
      <p class="mt-3 text-5xl font-bold font-display" style="font-family:'Cinzel',serif;">{monthName} {cal.day}</p>
      <p class="mt-1 text-lg" style="color:#6d510f;">{$_('calendar.year')} {cal.year}</p>
    </div>

    <div class="mt-4 flex items-center justify-center gap-2">
      <button onclick={() => advance(1)} disabled={busy} class="rounded px-3 py-1.5 text-sm inline-flex items-center gap-1" style="background:#8b6914;color:#f4e4c1;">
        <ChevronRight size={13} /> {$_(busy ? 'calendar.advancing' : 'calendar.day')}
      </button>
      <button onclick={() => advance(7)} disabled={busy} class="rounded px-3 py-1.5 text-sm" style="background:#8b6914;color:#f4e4c1;">
        {$_('calendar.week')}
      </button>
      <button onclick={() => advance(cal!.days_per_month)} disabled={busy} class="rounded px-3 py-1.5 text-sm" style="background:#8b6914;color:#f4e4c1;">
        {$_('calendar.month')}
      </button>
      <button onclick={() => advance(cal!.days_per_month * 12)} disabled={busy} class="rounded px-3 py-1.5 text-sm" style="background:#8b6914;color:#f4e4c1;">
        {$_('calendar.year_adv')}
      </button>
    </div>

    {#if campaign().isMaster}
      <div class="mt-6 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
        <h3 class="text-sm font-semibold" style="color:#c9a84c;">{$_('calendar.settings')}</h3>
        {#if !editing}
          {#if cal.notes}
            <p class="mt-2 text-sm whitespace-pre-wrap" style="color:#c2a178;">{cal.notes}</p>
          {/if}
          <button onclick={() => editing = true} class="mt-2 rounded px-3 py-1 text-xs" style="background:#8b6914;color:#f4e4c1;">
            {$_('calendar.edit')}
          </button>
        {:else}
          <div class="mt-2 space-y-2 text-sm">
            <label class="block">
              <span class="text-xs" style="color:#a6855c;">{$_('calendar.notes')}</span>
              <textarea bind:value={editNotes} rows="3" class="mt-0.5 w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1"></textarea>
            </label>
            <label class="block">
              <span class="text-xs" style="color:#a6855c;">{$_('calendar.months_ph')}</span>
              <textarea bind:value={editMonths} rows="6" class="mt-0.5 w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1"></textarea>
            </label>
            <label class="flex items-center gap-2">
              <span class="text-xs" style="color:#a6855c;">{$_('calendar.days_per_month')}</span>
              <input type="number" min="1" max="60" bind:value={editDaysPerMonth}
                class="w-20 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            </label>
            <button onclick={saveSettings} disabled={busy} class="rounded px-3 py-1 text-xs inline-flex items-center gap-1" style="background:#8b6914;color:#f4e4c1;">
              <Save size={12} /> {$_('common.save')}
            </button>
          </div>
        {/if}
      </div>
    {/if}
  {/if}
</section>
