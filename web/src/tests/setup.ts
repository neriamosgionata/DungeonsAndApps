import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// Setup jsdom globals
global.document = window.document;
global.window = window;

// Mock SvelteKit runtime
vi.mock('$app/environment', () => ({
  browser: true,
  dev: true,
  building: false,
  version: 'test'
}));

vi.mock('$app/state', () => ({
  page: {
    url: new URL('http://localhost:5173'),
    params: {},
    route: { id: null },
    status: 200,
    error: null,
    data: {},
    form: undefined
  },
  navigating: { current: null, complete: Promise.resolve() }
}));
