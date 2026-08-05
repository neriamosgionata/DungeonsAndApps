<script lang="ts">
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { Campaigns, Sessions } from '$lib/api/resources';
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
  let weather = $state('');
  let holidays = $state<Array<{ day: number; month: number; name: string }>>([]);
  let newHolDay = $state(1);
  let newHolMonth = $state(1);
  let newHolName = $state('');
  let customDays = $state(1);
  let pinnedSessions = $state<Array<{ id: string; title: string; calendar_date: string }>>([]);

  // Sessions pinned to in-game dates (calendar_date = "3 Mirtul 1492").
  async function loadPinnedSessions() {
    try {
      const sess = await Sessions.list(cid);
      pinnedSessions = (sess as Array<{ id: string; title: string; calendar_date?: string | null }>)
        .filter((s) => s.calendar_date)
        .map((s) => ({ id: s.id, title: s.title, calendar_date: s.calendar_date! }));
    } catch { pinnedSessions = []; }
  }

  // Next holiday: smallest days-until across the 12-month cycle (1-based).
  const nextHoliday = $derived.by(() => {
    if (!cal || !holidays.length) return null;
    const todayIdx = (cal.month - 1) * cal.days_per_month + cal.day;
    let best: { name: string; days: number } | null = null;
    for (const h of holidays) {
      const idx = (h.month - 1) * cal.days_per_month + h.day;
      let days = idx - todayIdx;
      if (days < 0) days += 12 * cal.days_per_month;
      if (days === 0) days = 12 * cal.days_per_month; // today's holiday already listed
      if (!best || days < best.days) best = { name: h.name, days };
    }
    return best;
  });

  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    load();
    loadPinnedSessions();
  });

  async function load() {
    try {
      const c = await Campaigns.calendar(cid);
      cal = c;
      weather = c.weather ?? '';
      holidays = c.holidays ?? [];
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

  async function removeHoliday(name: string) {
    const next = holidays.filter((h) => h.name !== name);
    try {
      cal = await Campaigns.calendarUpdate(cid, { holidays: next });
      holidays = cal.holidays ?? [];
    } catch (e) { error = (e as Error).message; }
  }

  async function addHoliday() {
    if (!newHolName.trim()) return;
    const next = [...holidays.filter((h) => h.name !== newHolName.trim()), { day: newHolDay, month: newHolMonth, name: newHolName.trim() }];
    try {
      cal = await Campaigns.calendarUpdate(cid, { holidays: next });
      holidays = cal.holidays ?? [];
      newHolName = '';
    } catch (e) { error = (e as Error).message; }
  }

  async function saveSettings() {
    busy = true; error = '';
    try {
      cal = await Campaigns.calendarUpdate(cid, {
        weather: weather,
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
      {#if (cal.moon_phases ?? []).length}
        <p class="mt-2 text-xs uppercase tracking-widest" style="color:#8b6914;">
          {$_('calendar.moon')}: {(cal.moon_phases ?? [])[(cal.day - 1) % (cal.moon_phases ?? []).length]}
        </p>
      {/if}
    </div>

    {#if weather}
      <div class="mt-4 rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-2 text-center">
        <span class="text-xs uppercase tracking-widest" style="color:#a6855c;">{$_('calendar.weather')}</span>
        <p class="mt-0.5 text-sm" style="color:#f4e4c1;">{weather}</p>
      </div>
    {/if}

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
              <span class="text-xs" style="color:#a6855c;">{$_('calendar.weather')}</span>
              <input bind:value={weather} placeholder={$_('calendar.weather_ph')}
                class="mt-0.5 w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            </label>
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
  {#if cal && (cal.holidays ?? []).length}
    <div class="mt-6 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
      <h3 class="text-sm font-semibold" style="color:#c9a84c;">{$_('calendar.holidays')}</h3>
      <ul class="mt-2 space-y-1">
        {#each holidays as h (h.name)}
          <li class="text-sm flex items-center gap-2" style="color:#c2a178;">
            <span>{h.name} — {cal.months[h.month - 1] ?? h.month} {h.day}</span>
            {#if h.day === cal.day && h.month === cal.month}
              <span class="text-xs" style="color:#2a8a2a;">· {$_('calendar.today')}</span>
            {/if}
            {#if campaign().isMaster}
              <button type="button" onclick={() => removeHoliday(h.name)} class="ml-auto text-xs" style="color:#8b1a1a;" title={$_('calendar.holiday_remove')}>×</button>
            {/if}
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if pinnedSessions.length}
    <div class="mt-6 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
      <h3 class="text-sm font-semibold" style="color:#c9a84c;">{$_('calendar.pinned_sessions')}</h3>
      <ul class="mt-2 space-y-1">
        {#each pinnedSessions as ps (ps.id)}
          <li class="text-sm flex items-center gap-2" style="color:#c2a178;">
            <a href={`/campaigns/${cid}/recap`} class="underline hover:opacity-80">{ps.title}</a>
            <span class="text-xs" style="color:#8b6914;">— {ps.calendar_date}</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if campaign().isMaster}
    <div class="mt-4 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
      <h3 class="text-sm font-semibold" style="color:#c9a84c;">{$_('calendar.holidays_add')}</h3>
      <div class="mt-2 flex flex-wrap items-center gap-2 text-sm">
        <input type="number" min="1" max="60" bind:value={newHolDay} class="w-16 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
        <input type="number" min="1" max="24" bind:value={newHolMonth} class="w-16 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
        <input bind:value={newHolName} placeholder={$_('calendar.holiday_ph')} class="flex-1 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
        <button onclick={addHoliday} class="rounded px-3 py-1" style="background:#8b6914;color:#f4e4c1;">{ $_('common.add') }</button>
      </div>
    </div>
  {/if}
  {/if}
</section>
