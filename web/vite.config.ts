import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    host: '0.0.0.0',
    port: 5173,
  },
  ssr: {
    // @lucide/svelte ships raw .svelte sources; force Vite to compile them via the Svelte plugin
    noExternal: ['@lucide/svelte'],
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.{js,ts}'],
    exclude: ['**/e2e/**', '**/node_modules/**', '**/build/**', '**/.svelte-kit/**'],
    globals: true,
    setupFiles: ['./src/tests/setup.ts'],
    alias: {
      '$app/environment': new URL('./src/tests/mocks/app-environment.ts', import.meta.url).pathname,
      '$app/state': new URL('./src/tests/mocks/app-state.ts', import.meta.url).pathname,
    },
  },
});
