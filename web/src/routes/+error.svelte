<script lang="ts">
  import { page } from '$app/state';
  import '$lib/i18n';
  import { _ } from 'svelte-i18n';
</script>

<div class="page-panel mx-auto mt-16 max-w-lg p-8 text-center">
  <p class="text-5xl font-bold" style="color:#c9a84c;">{page.status}</p>
  <p class="mt-4 text-xl font-semibold" style="color:#2c1810;">
    {#if page.status === 404}{$_('errors.not_found')}
    {:else if page.status === 401}{$_('errors.unauthorized')}
    {:else if page.status === 403}{$_('errors.forbidden')}
    {:else if page.status === 409}{$_('errors.conflict')}
    {:else if page.status === 400}{$_('errors.bad_request')}
    {:else}{$_('errors.internal')}
    {/if}
  </p>
  {#if page.error?.message}
    <p class="mt-3 text-sm" style="color:#8b6355;">{page.error.message}</p>
  {/if}
  <a href="/" class="btn mt-6 inline-block">← Home</a>
</div>
