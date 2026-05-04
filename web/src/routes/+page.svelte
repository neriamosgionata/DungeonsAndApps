<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { Auth } from '$lib/api/resources';
  import { LogIn, Sparkles } from '@lucide/svelte';

  let checked = $state(false);
  onMount(() => {
    if (auth.authenticated) { goto('/campaigns'); return; }
    checked = true;
  });
</script>

<section class="hero">
  <div class="torch torch-l"></div>
  <div class="torch torch-r"></div>

  <div class="crest-wrap">
    <svg class="crest" viewBox="0 0 160 160" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <defs>
        <radialGradient id="shield" cx="50%" cy="40%" r="60%">
          <stop offset="0%"  stop-color="#f7e2a5"/>
          <stop offset="55%" stop-color="#c9a84c"/>
          <stop offset="100%" stop-color="#5a3a17"/>
        </radialGradient>
        <radialGradient id="gem" cx="50%" cy="50%" r="50%">
          <stop offset="0%" stop-color="#ffd7a8"/>
          <stop offset="60%" stop-color="#c9a84c"/>
          <stop offset="100%" stop-color="#4e3909"/>
        </radialGradient>
      </defs>
      <!-- outer shield -->
      <path d="M80 8 L140 28 V80 Q140 120 80 152 Q20 120 20 80 V28 Z"
            fill="url(#shield)" stroke="#4e3909" stroke-width="2"/>
      <!-- inner bevel -->
      <path d="M80 16 L132 32 V80 Q132 114 80 142 Q28 114 28 80 V32 Z"
            fill="none" stroke="#f7e2a5" stroke-width="1" opacity="0.55"/>
      <!-- central gem -->
      <circle cx="80" cy="62" r="9" fill="url(#gem)" stroke="#4e3909" stroke-width="1.5"/>
      <!-- boar -->
      <g transform="translate(80, 96)">
        <path d="M-28 10 q3-8 10-8 q2-7 7-7 l6-5 q2-3 6-2 l8-1 q9 0 15 5 q5 3 5 9 l-2 3 l4 3 v5 l-6 4 l-3-2 l-2 4 l-10 0 l-2-4 l-6 4 l-3-2 l-2 1 l-6-1 l-4 3 l-8-3 l-3 0 z"
              fill="#2c1810" stroke="#1a0f08" stroke-width="1"/>
        <!-- tusks -->
        <path d="M-20 8 l-3-2 M-22 5 l-2-3" stroke="#f4e4c1" stroke-width="1.5" stroke-linecap="round"/>
        <!-- eye -->
        <circle cx="8" cy="-2" r="1.2" fill="#c9a84c"/>
      </g>
      <!-- cross-pattee -->
      <g fill="#4e3909" opacity="0.6" transform="translate(80, 40)">
        <path d="M-2-10 h4 v6 h6 v4 h-6 v6 h-4 v-6 h-6 v-4 h6 z"/>
      </g>
    </svg>
  </div>

  <div class="latin">— Ordo Aper Silvarum, Est. MDCCCXXVII —</div>
  <h1 class="title">DUNGEONSANDAPPS</h1>

  <div class="rule">
    <svg viewBox="0 0 300 20" class="ornament" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <line x1="0" y1="10" x2="120" y2="10" stroke="#8b6914" stroke-width="1"/>
      <line x1="180" y1="10" x2="300" y2="10" stroke="#8b6914" stroke-width="1"/>
      <circle cx="150" cy="10" r="5" fill="none" stroke="#c9a84c" stroke-width="1.2"/>
      <circle cx="150" cy="10" r="2" fill="#c9a84c"/>
      <circle cx="130" cy="10" r="1.5" fill="#8b6914"/>
      <circle cx="170" cy="10" r="1.5" fill="#8b6914"/>
    </svg>
  </div>

  <p class="tagline">{$_('home.tagline')}</p>

  {#if checked}
    <div class="cta">
      <a href="/login" class="btn btn-primary">
        <LogIn size={18} />
        {$_('auth.login')}
      </a>
      <a href="/register" class="btn btn-accent">
        <Sparkles size={18} />
        {$_('auth.register')}
      </a>
    </div>
  {/if}
</section>

<style>
  .hero {
    position: relative;
    max-width: 44rem;
    margin: 0 auto;
    padding: 4rem 1.5rem 6rem;
    text-align: center;
    overflow: hidden;
  }
  .torch {
    position: absolute; top: 10%;
    width: 10rem; height: 16rem;
    pointer-events: none;
    filter: blur(40px);
    animation: flicker 4s ease-in-out infinite;
  }
  .torch-l { left: -4rem; background: radial-gradient(ellipse at center, rgba(201,168,76,0.35), transparent 70%); }
  .torch-r { right: -4rem; background: radial-gradient(ellipse at center, rgba(168,88,50,0.28), transparent 70%); animation-delay: -2s; }
  @keyframes flicker {
    0%, 100% { opacity: 1; transform: scale(1); }
    35% { opacity: 0.7; transform: scale(1.05); }
    65% { opacity: 0.9; transform: scale(0.98); }
  }

  .crest-wrap { display: flex; justify-content: center; }
  .crest {
    width: 9rem; height: 9rem;
    filter: drop-shadow(0 8px 20px rgba(0,0,0,0.7)) drop-shadow(0 0 30px rgba(201,168,76,0.2));
  }
  .latin {
    margin-top: 1rem;
    font-family: 'Cinzel', serif;
    font-size: 0.75rem;
    letter-spacing: 0.4em;
    color: #8b6914;
  }
  .title {
    margin-top: 0.5rem;
    font-family: 'Cinzel', serif;
    font-weight: 900;
    font-size: 4.25rem;
    letter-spacing: 0.08em;
    color: #c9a84c;
    text-shadow:
      0 2px 0 rgba(0, 0, 0, 0.8),
      0 4px 12px rgba(0, 0, 0, 0.7),
      0 0 40px rgba(201, 168, 76, 0.25);
    line-height: 1;
  }
  .rule { display: flex; justify-content: center; margin-top: 1rem; }
  .ornament { width: 18rem; height: 20px; opacity: 0.85; }
  .tagline {
    margin-top: 1.25rem;
    font-family: 'Crimson Text', serif;
    font-style: italic;
    font-size: 1.125rem;
    color: #e8d5a3;
  }
  .cta {
    margin-top: 2.5rem;
    display: flex;
    justify-content: center;
    gap: 0.75rem;
  }
  .btn {
    display: inline-flex; align-items: center; gap: 0.5rem;
    padding: 0.75rem 1.5rem;
    border-radius: 0.5rem;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-size: 0.9rem;
  }
</style>
