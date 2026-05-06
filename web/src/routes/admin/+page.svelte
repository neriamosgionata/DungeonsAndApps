<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { Users, Admin, type UserRow } from '$lib/api/resources';
  import { ApiError } from '$lib/api/client';

  type CampaignRow = { id: string; name: string; owner_name: string; member_count: number; created_at: string };
  type Stats = { users: number; campaigns: number; characters: number; messages: number; encounters: number; spells: number };

  let tab = $state<'users' | 'campaigns' | 'system' | 'backup'>('users');
  let users = $state<UserRow[]>([]);
  let campaigns = $state<CampaignRow[]>([]);
  let stats = $state<Stats | null>(null);
  let loading = $state(true);
  let error = $state('');

  // create user form
  let showCreate = $state(false);
  let createEmail = $state('');
  let createName = $state('');
  let createPassword = $state('');
  let createRole = $state<'user' | 'admin'>('user');
  let createLang = $state<'en' | 'it'>('en');
  let createBusy = $state(false);
  let createError = $state('');

  // edit user
  let editId = $state<string | null>(null);
  let editName = $state('');
  let editRole = $state<'user' | 'admin'>('user');
  let editLang = $state<'en' | 'it'>('en');
  let editBusy = $state(false);

  // backup/restore
  let backupBusy = $state(false);
  let restoreBusy = $state(false);
  let restoreFile: File | null = $state(null);

  onMount(async () => {
    if (!auth.isAdmin) { goto('/campaigns'); return; }
    await loadAll();
  });

  async function loadAll() {
    loading = true; error = '';
    try {
      [users, campaigns, stats] = await Promise.all([
        Users.list(),
        Admin.campaigns(),
        Admin.stats(),
      ]);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      loading = false;
    }
  }

  async function createUser() {
    createBusy = true; createError = '';
    try {
      const u = await Users.create({ email: createEmail, password: createPassword, display_name: createName, role: createRole, language: createLang });
      users = [...users, u];
      showCreate = false;
      createEmail = ''; createName = ''; createPassword = ''; createRole = 'user'; createLang = 'en';
    } catch (e) {
      createError = (e as ApiError).message;
    } finally {
      createBusy = false;
    }
  }

  function startEdit(u: UserRow) {
    editId = u.id;
    editName = u.display_name;
    editRole = u.role as 'user' | 'admin';
    editLang = u.language as 'en' | 'it';
  }

  async function saveEdit() {
    if (!editId) return;
    editBusy = true;
    try {
      const updated = await Users.update(editId, { display_name: editName, role: editRole, language: editLang });
      users = users.map(u => u.id === editId ? updated : u);
      editId = null;
    } catch (e) {
      alert((e as Error).message);
    } finally {
      editBusy = false;
    }
  }

  async function resetPassword(u: UserRow) {
    const pw = prompt($_('admin.reset_prompt', { values: { name: u.display_name } }));
    if (!pw) return;
    try {
      await Users.resetPassword(u.id, pw);
      alert($_('admin.reset_ok'));
    } catch (e) {
      alert((e as Error).message);
    }
  }

  async function deleteUser(u: UserRow) {
    if (!confirm($_('admin.delete_user_confirm', { values: { name: u.display_name } }))) return;
    try {
      await Users.delete(u.id);
      users = users.filter(x => x.id !== u.id);
    } catch (e) {
      alert((e as Error).message);
    }
  }

  async function deleteCampaign(c: CampaignRow) {
    if (!confirm($_('admin.delete_campaign_confirm', { values: { name: c.name } }))) return;
    try {
      await Admin.deleteCampaign(c.id);
      campaigns = campaigns.filter(x => x.id !== c.id);
      if (stats) stats = { ...stats, campaigns: stats.campaigns - 1 };
    } catch (e) {
      alert((e as Error).message);
    }
  }

  function fmt(iso: string) {
    return new Date(iso).toLocaleDateString();
  }

  async function downloadBackup() {
    backupBusy = true;
    try {
      const data = await Admin.backup();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `dungeonsandapps-backup-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      alert((e as Error).message);
    } finally {
      backupBusy = false;
    }
  }

  async function uploadBackup() {
    if (!restoreFile) return;
    if (!confirm($_('admin.restore_confirm'))) return;

    restoreBusy = true;
    try {
      const text = await restoreFile.text();
      const backup = JSON.parse(text);
      await Admin.restore(backup);
      alert($_('admin.restore_ok'));
      restoreFile = null;
      await loadAll();
    } catch (e) {
      alert((e as Error).message);
    } finally {
      restoreBusy = false;
    }
  }
</script>

<div class="page-panel">
  <div class="panel-header">
    <h1>{$_('admin.title')}</h1>
  </div>

  {#if loading}
    <p class="muted">{$_('common.loading')}</p>
  {:else if error}
    <p class="err">{error}</p>
  {:else}
    <!-- Tabs -->
    <div class="tabs">
      <button class="tab" class:active={tab === 'users'} onclick={() => tab = 'users'}>{$_('admin.tab_users')} ({users.length})</button>
      <button class="tab" class:active={tab === 'campaigns'} onclick={() => tab = 'campaigns'}>{$_('admin.tab_campaigns')} ({campaigns.length})</button>
      <button class="tab" class:active={tab === 'system'} onclick={() => tab = 'system'}>{$_('admin.tab_system')}</button>
      <button class="tab" class:active={tab === 'backup'} onclick={() => tab = 'backup'}>{$_('admin.tab_backup')}</button>
    </div>

    <!-- ── Users ──────────────────────────────────────────────────── -->
    {#if tab === 'users'}
      <div class="section-head">
        <button class="btn-brass" onclick={() => showCreate = !showCreate}>{$_('admin.create_user')}</button>
      </div>

      {#if showCreate}
        <form class="create-form" onsubmit={(e) => { e.preventDefault(); createUser(); }}>
          <h3>{$_('admin.create_user_title')}</h3>
          <div class="form-grid">
            <label>
              <span>{$_('admin.user_email')}</span>
              <input type="email" required bind:value={createEmail} />
            </label>
            <label>
              <span>{$_('admin.user_name')}</span>
              <input type="text" required bind:value={createName} />
            </label>
            <label>
              <span>{$_('admin.user_password')}</span>
              <input type="password" required minlength="8" bind:value={createPassword} />
            </label>
            <label>
              <span>{$_('admin.user_role')}</span>
              <select bind:value={createRole}>
                <option value="user">{$_('admin.role_user')}</option>
                <option value="admin">{$_('admin.role_admin')}</option>
              </select>
            </label>
            <label>
              <span>{$_('admin.user_language')}</span>
              <select bind:value={createLang}>
                <option value="en">English</option>
                <option value="it">Italiano</option>
              </select>
            </label>
          </div>
          {#if createError}<p class="err">{createError}</p>{/if}
          <div class="form-actions">
            <button type="submit" class="btn-brass" disabled={createBusy}>{createBusy ? '…' : $_('common.create')}</button>
            <button type="button" class="btn-ghost" onclick={() => showCreate = false}>{$_('common.cancel')}</button>
          </div>
        </form>
      {/if}

      {#if users.length === 0}
        <p class="muted">{$_('admin.no_users')}</p>
      {:else}
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{$_('admin.user_name')}</th>
                <th>{$_('admin.user_email')}</th>
                <th>{$_('admin.user_role')}</th>
                <th>{$_('admin.user_language')}</th>
                <th>{$_('admin.user_created')}</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {#each users as u (u.id)}
                {#if editId === u.id}
                  <tr class="edit-row">
                    <td><input bind:value={editName} class="inline-input" /></td>
                    <td class="muted">{u.email}</td>
                    <td>
                      <select bind:value={editRole} class="inline-select" disabled={u.id === auth.user?.id}>
                        <option value="user">{$_('admin.role_user')}</option>
                        <option value="admin">{$_('admin.role_admin')}</option>
                      </select>
                    </td>
                    <td>
                      <select bind:value={editLang} class="inline-select">
                        <option value="en">EN</option>
                        <option value="it">IT</option>
                      </select>
                    </td>
                    <td>{fmt(u.created_at)}</td>
                    <td class="actions">
                      <button class="btn-xs btn-brass" onclick={saveEdit} disabled={editBusy}>{editBusy ? '…' : $_('common.save')}</button>
                      <button class="btn-xs btn-ghost" onclick={() => editId = null}>{$_('common.cancel')}</button>
                    </td>
                  </tr>
                {:else}
                  <tr>
                    <td>
                      {u.display_name}
                      {#if u.id === auth.user?.id}<span class="you-badge">{$_('admin.user_you')}</span>{/if}
                    </td>
                    <td class="muted">{u.email}</td>
                    <td><span class="role-badge role-{u.role}">{u.role === 'admin' ? $_('admin.role_admin') : $_('admin.role_user')}</span></td>
                    <td class="muted">{u.language.toUpperCase()}</td>
                    <td class="muted">{fmt(u.created_at)}</td>
                    <td class="actions">
                      <button class="btn-xs btn-ghost" onclick={() => startEdit(u)}>{$_('common.edit')}</button>
                      <button class="btn-xs btn-ghost" onclick={() => resetPassword(u)}>{$_('admin.reset_password')}</button>
                      {#if u.id !== auth.user?.id}
                        <button class="btn-xs btn-danger" onclick={() => deleteUser(u)}>{$_('common.delete')}</button>
                      {/if}
                    </td>
                  </tr>
                {/if}
              {/each}
            </tbody>
          </table>
        </div>
      {/if}

    <!-- ── Campaigns ───────────────────────────────────────────────── -->
    {:else if tab === 'campaigns'}
      <h2 class="section-title">{$_('admin.campaigns_title')}</h2>
      {#if campaigns.length === 0}
        <p class="muted">{$_('admin.no_campaigns')}</p>
      {:else}
        <div class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{$_('common.name')}</th>
                <th>{$_('admin.campaign_owner')}</th>
                <th>{$_('admin.campaign_members')}</th>
                <th>{$_('admin.campaign_created')}</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {#each campaigns as c (c.id)}
                <tr>
                  <td>{c.name}</td>
                  <td class="muted">{c.owner_name}</td>
                  <td class="muted">{c.member_count}</td>
                  <td class="muted">{fmt(c.created_at)}</td>
                  <td class="actions">
                    <button class="btn-xs btn-danger" onclick={() => deleteCampaign(c)}>{$_('common.delete')}</button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}

    <!-- ── System ──────────────────────────────────────────────────── -->
    {:else if tab === 'system'}
      <h2 class="section-title">{$_('admin.system_title')}</h2>
      {#if stats}
        <div class="stats-grid">
          <div class="stat-card"><span class="stat-val">{stats.users}</span><span class="stat-label">{$_('admin.stat_users')}</span></div>
          <div class="stat-card"><span class="stat-val">{stats.campaigns}</span><span class="stat-label">{$_('admin.stat_campaigns')}</span></div>
          <div class="stat-card"><span class="stat-val">{stats.characters}</span><span class="stat-label">{$_('admin.stat_characters')}</span></div>
          <div class="stat-card"><span class="stat-val">{stats.messages}</span><span class="stat-label">{$_('admin.stat_messages')}</span></div>
          <div class="stat-card"><span class="stat-val">{stats.encounters}</span><span class="stat-label">{$_('admin.stat_encounters')}</span></div>
          <div class="stat-card"><span class="stat-val">{stats.spells}</span><span class="stat-label">{$_('admin.stat_spells')}</span></div>
        </div>
      {/if}

    <!-- ── Backup ──────────────────────────────────────────────────── -->
    {:else if tab === 'backup'}
      <h2 class="section-title">{$_('admin.backup_title')}</h2>
      <p class="muted">{$_('admin.backup_desc')}</p>

      <div class="backup-section">
        <h3>{$_('admin.download_backup')}</h3>
        <p class="muted">{$_('admin.download_backup_desc')}</p>
        <button class="btn-brass" onclick={downloadBackup} disabled={backupBusy}>
          {backupBusy ? $_('common.loading') : $_('admin.download_backup_btn')}
        </button>
      </div>

      <div class="backup-section">
        <h3>{$_('admin.restore_backup')}</h3>
        <p class="muted danger">{$_('admin.restore_backup_desc')}</p>
        <div class="restore-form">
          <input
            type="file"
            accept=".json,application/json"
            onchange={(e) => { restoreFile = (e.target as HTMLInputElement).files?.[0] ?? null; }}
          />
          <button
            class="btn-danger"
            onclick={uploadBackup}
            disabled={restoreBusy || !restoreFile}
          >
            {restoreBusy ? $_('common.loading') : $_('admin.restore_backup_btn')}
          </button>
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .page-panel {
    max-width: 80rem;
    margin: 0 auto;
    padding: 2rem 1.5rem 4rem;
  }
  .panel-header { margin-bottom: 1.5rem; }
  h1 {
    font-family: 'Cinzel', serif;
    font-size: 1.75rem;
    font-weight: 700;
    color: #c9a84c;
  }
  h2.section-title {
    font-family: 'Cinzel', serif;
    font-size: 1.1rem;
    color: #c9a84c;
    margin: 1.5rem 0 1rem;
    border-bottom: 1px solid #3a2313;
    padding-bottom: 0.5rem;
  }

  /* Tabs */
  .tabs {
    display: flex;
    gap: 0;
    border-bottom: 2px solid #3a2313;
    margin-bottom: 1.5rem;
  }
  .tab {
    font-family: 'Cinzel', serif;
    font-size: 0.85rem;
    padding: 0.6rem 1.25rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: #8b6914;
    cursor: pointer;
    margin-bottom: -2px;
    transition: color 0.15s, border-color 0.15s;
  }
  .tab:hover { color: #c9a84c; }
  .tab.active { color: #c9a84c; border-bottom-color: #c9a84c; }

  /* Section head */
  .section-head {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 1rem;
  }

  /* Create form */
  .create-form {
    background: #1e0f09;
    border: 1px solid #3a2313;
    border-radius: 0.5rem;
    padding: 1.25rem;
    margin-bottom: 1.5rem;
  }
  .create-form h3 {
    font-family: 'Cinzel', serif;
    font-size: 1rem;
    color: #c9a84c;
    margin-bottom: 1rem;
  }
  .form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 0.75rem;
    margin-bottom: 1rem;
  }
  .form-grid label span {
    display: block;
    font-size: 0.75rem;
    color: #8b6914;
    margin-bottom: 0.25rem;
  }
  .form-grid input, .form-grid select {
    width: 100%;
    background: #0f0704;
    border: 1px solid #3a2313;
    border-radius: 0.25rem;
    color: #f4e4c1;
    padding: 0.4rem 0.6rem;
    font-size: 0.875rem;
  }
  .form-actions { display: flex; gap: 0.5rem; }

  /* Table */
  .table-wrap { overflow-x: auto; }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }
  th {
    text-align: left;
    font-family: 'Cinzel', serif;
    font-size: 0.75rem;
    color: #8b6914;
    border-bottom: 1px solid #3a2313;
    padding: 0.5rem 0.75rem;
  }
  td {
    padding: 0.6rem 0.75rem;
    border-bottom: 1px solid #1e0f09;
    color: #f4e4c1;
    vertical-align: middle;
  }
  tr:hover td { background: #1a0d07; }
  .muted { color: #8b7355; }
  .actions { display: flex; gap: 0.4rem; flex-wrap: wrap; justify-content: flex-end; }

  /* Inline edit */
  .inline-input {
    background: #0f0704;
    border: 1px solid #3a2313;
    border-radius: 0.25rem;
    color: #f4e4c1;
    padding: 0.3rem 0.5rem;
    font-size: 0.875rem;
    width: 100%;
  }
  .inline-select {
    background: #0f0704;
    border: 1px solid #3a2313;
    border-radius: 0.25rem;
    color: #f4e4c1;
    padding: 0.3rem 0.4rem;
    font-size: 0.8rem;
  }

  /* Badges */
  .role-badge {
    font-size: 0.7rem;
    font-family: 'Cinzel', serif;
    padding: 0.15rem 0.5rem;
    border-radius: 0.25rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .role-admin { background: #3a2800; color: #c9a84c; border: 1px solid #6d510f; }
  .role-user  { background: #1a2a1a; color: #7ab87a; border: 1px solid #3a6a3a; }
  .you-badge {
    font-size: 0.65rem;
    color: #8b6914;
    margin-left: 0.4rem;
    font-style: italic;
  }

  /* Buttons */
  .btn-brass {
    background: linear-gradient(135deg, #8b6914, #c9a84c);
    color: #1a0f05;
    border: none;
    border-radius: 0.375rem;
    padding: 0.5rem 1rem;
    font-family: 'Cinzel', serif;
    font-size: 0.8rem;
    font-weight: 700;
    cursor: pointer;
    letter-spacing: 0.05em;
  }
  .btn-brass:hover { filter: brightness(1.1); }
  .btn-brass:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-ghost {
    background: transparent;
    color: #8b6914;
    border: 1px solid #3a2313;
    border-radius: 0.375rem;
    padding: 0.5rem 1rem;
    font-family: 'Cinzel', serif;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .btn-ghost:hover { color: #c9a84c; border-color: #8b6914; }
  .btn-xs {
    padding: 0.25rem 0.6rem;
    font-size: 0.72rem;
    border-radius: 0.25rem;
    cursor: pointer;
    font-family: 'Cinzel', serif;
    border: none;
    white-space: nowrap;
  }
  .btn-xs.btn-brass { padding: 0.25rem 0.6rem; }
  .btn-xs.btn-ghost { border: 1px solid #3a2313; background: transparent; color: #8b6914; }
  .btn-xs.btn-ghost:hover { color: #c9a84c; }
  .btn-xs.btn-danger { background: #3a0a0a; color: #e57373; border: 1px solid #6a1a1a; }
  .btn-xs.btn-danger:hover { background: #5a1010; }

  /* Stats grid */
  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 1rem;
    margin-top: 1rem;
  }
  .stat-card {
    background: #1e0f09;
    border: 1px solid #3a2313;
    border-radius: 0.5rem;
    padding: 1.25rem 1rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
  }
  .stat-val {
    font-family: 'Cinzel', serif;
    font-size: 2rem;
    font-weight: 700;
    color: #c9a84c;
    line-height: 1;
  }
  .stat-label {
    font-size: 0.75rem;
    color: #8b6914;
    text-align: center;
  }

  .err { color: #e57373; font-size: 0.875rem; margin: 0.5rem 0; }

  /* Backup section */
  .backup-section {
    background: #1e0f09;
    border: 1px solid #3a2313;
    border-radius: 0.5rem;
    padding: 1.25rem;
    margin-top: 1.5rem;
  }
  .backup-section h3 {
    font-family: 'Cinzel', serif;
    font-size: 1rem;
    color: #c9a84c;
    margin-bottom: 0.5rem;
  }
  .backup-section .muted { margin-bottom: 1rem; }
  .backup-section .danger {
    color: #e57373;
  }
  .restore-form {
    display: flex;
    gap: 1rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .restore-form input[type="file"] {
    color: #f4e4c1;
    font-size: 0.875rem;
  }
  .btn-danger {
    background: #3a0a0a;
    color: #e57373;
    border: 1px solid #6a1a1a;
    border-radius: 0.375rem;
    padding: 0.5rem 1rem;
    font-family: 'Cinzel', serif;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .btn-danger:hover:not(:disabled) { background: #5a1010; }
  .btn-danger:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
