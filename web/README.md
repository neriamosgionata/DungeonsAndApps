# DungeonsAndApps — Web

SvelteKit 2 + Svelte 5 (runes) + TailwindCSS v4 + TypeScript. Static build served by nginx in prod.

## Dev

```sh
bun install
bun dev        # :5173, proxies API to :8080
```

## Test

```sh
bunx svelte-check   # type check
bun test            # vitest unit tests
```

## Build

```sh
bun run build       # outputs to build/
```

## Stack notes

- Svelte 5 runes only (`$state`, `$derived`, `$effect`, `$props`) — no legacy stores except `svelte-i18n`
- All user-facing strings via `svelte-i18n` (`$_('key')`) — EN + IT
- Steampunk theme: walnut `#2c1810`, parchment `#f4e4c1`, brass `#c9a84c`
- Auth state: `web/src/lib/stores/auth.svelte.ts` (localStorage + cross-tab sync)
- WS client: `web/src/lib/ws.svelte.ts`
- API client: `web/src/lib/api/client.ts` (auto-redirect on 401)
