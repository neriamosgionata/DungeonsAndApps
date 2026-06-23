<script lang="ts">
  import type { Snippet } from 'svelte';
  import { onMount, tick } from 'svelte';

  let {
    onClose,
    title,
    dark = false,
    children,
  }: {
    onClose: () => void;
    title?: string;
    dark?: boolean;
    children: Snippet;
  } = $props();

  let dialogEl: HTMLDivElement | undefined = $state();
  let previouslyFocused: HTMLElement | null = null;

  // M-F5: focus trap + initial focus + focus restoration.
  // - On mount, remember the previously focused element and focus the
  //   first focusable child (or the dialog itself if none).
  // - On Tab/Shift+Tab, cycle within the focusable children.
  // - On close (component destroy), restore focus to the previously
  //   focused element.
  const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  function getFocusable(): HTMLElement[] {
    if (!dialogEl) return [];
    return Array.from(dialogEl.querySelectorAll<HTMLElement>(FOCUSABLE))
      .filter((el) => !el.hasAttribute('aria-hidden'));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== 'Tab') return;
    const focusable = getFocusable();
    if (focusable.length === 0) {
      e.preventDefault();
      dialogEl?.focus();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement as HTMLElement | null;
    if (e.shiftKey) {
      if (active === first || !dialogEl?.contains(active)) {
        e.preventDefault();
        last.focus();
      }
    } else {
      if (active === last || !dialogEl?.contains(active)) {
        e.preventDefault();
        first.focus();
      }
    }
  }
  // Backdrop click-to-close + Escape handled here too (so the a11y rule
  // sees a keyboard handler on the clickable element).
  function handleBackdropKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onClose();
    }
  }

  onMount(() => {
    previouslyFocused = document.activeElement as HTMLElement | null;
    (async () => {
      await tick();
      const focusable = getFocusable();
      if (focusable.length > 0) {
        focusable[0].focus();
      } else {
        dialogEl?.focus();
      }
    })();
    return () => {
      // Restore focus on close.
      if (previouslyFocused && document.contains(previouslyFocused)) {
        previouslyFocused.focus();
      }
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center"
  role="presentation"
  style="background:rgba(0,0,0,0.75);"
  onclick={onClose}
  onkeydown={handleBackdropKeydown}
>
  <div
    bind:this={dialogEl}
    class="max-w-lg w-full max-h-[80vh] overflow-y-auto rounded-lg border p-4 space-y-2"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    style={dark
      ? 'border-color:#a6855c; background:#171717; color:#f4e4c1;'
      : 'border-color:#a6855c; background:#f4e4c1; color:#2c1810;'}
    onclick={(e) => e.stopPropagation()}
  >
    {#if title}
      <div class="flex items-center justify-between">
        <h3
          class="font-display font-bold text-lg"
          style={dark ? 'color:#c9a84c;' : 'color:#2c1810;'}>
          {title}
        </h3>
        <button onclick={onClose} class="text-sm" style="color:#8b6355;" aria-label="Close">✕</button>
      </div>
    {/if}
    {@render children()}
  </div>
</div>
