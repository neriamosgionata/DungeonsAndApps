<script lang="ts">
  import type { Snippet } from 'svelte';
  let {
    children,
    compact = false,
    tone = 'parchment',
  }: {
    children: Snippet;
    compact?: boolean;
    tone?: 'parchment' | 'dark';
  } = $props();
</script>

<div class="ornate-frame tone-{tone} {compact ? 'compact' : ''}">
  <span class="corner corner-tl"></span>
  <span class="corner corner-tr"></span>
  <span class="corner corner-bl"></span>
  <span class="corner corner-br"></span>
  <div class="inner">
    {@render children()}
  </div>
</div>

<style>
  .ornate-frame { position: relative; padding: 1.25rem; }
  .compact      { padding: 0.85rem; }
  .tone-parchment {
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.08 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    border: 1.5px solid #8b6914;
    color: #2c1810;
    box-shadow:
      inset 0 1px 0 rgba(255, 248, 220, 0.55),
      0 6px 14px rgba(0, 0, 0, 0.6);
    border-radius: 0.5rem;
  }
  .tone-dark {
    background: #1a0f08
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='n'><feTurbulence baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 0.09  0 0 0 0 0.06  0 0 0 0 0.03  0 0 0 0.35 0'/></filter><rect width='100%' height='100%' filter='url(%23n)' opacity='0.6'/></svg>");
    border: 1.5px solid #4e3909;
    color: #f4e4c1;
    border-radius: 0.5rem;
  }

  /* gold corner flourishes — pseudo-element curls */
  .corner {
    position: absolute;
    width: 24px; height: 24px;
    pointer-events: none;
    background-repeat: no-repeat;
    background-size: contain;
  }
  .corner-tl { top: -2px;    left: -2px;    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23c9a84c' stroke-width='1.5'><path d='M2 22V8c0-3.3 2.7-6 6-6h14'/><circle cx='4' cy='4' r='1.5' fill='%23c9a84c'/></svg>"); }
  .corner-tr { top: -2px;    right: -2px;   transform: scaleX(-1); background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23c9a84c' stroke-width='1.5'><path d='M2 22V8c0-3.3 2.7-6 6-6h14'/><circle cx='4' cy='4' r='1.5' fill='%23c9a84c'/></svg>"); }
  .corner-bl { bottom: -2px; left: -2px;    transform: scaleY(-1); background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23c9a84c' stroke-width='1.5'><path d='M2 22V8c0-3.3 2.7-6 6-6h14'/><circle cx='4' cy='4' r='1.5' fill='%23c9a84c'/></svg>"); }
  .corner-br { bottom: -2px; right: -2px;   transform: scale(-1, -1); background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23c9a84c' stroke-width='1.5'><path d='M2 22V8c0-3.3 2.7-6 6-6h14'/><circle cx='4' cy='4' r='1.5' fill='%23c9a84c'/></svg>"); }

  .inner { position: relative; z-index: 1; }
</style>
