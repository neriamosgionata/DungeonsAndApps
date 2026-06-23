import { browser } from '$app/environment';
import type { User } from '$lib/types';

const STORAGE_KEY_TOKEN = 'dungeonsandapps.token';
const STORAGE_KEY_USER = 'dungeonsandapps.user';

// jsdom v29 throws on localStorage access for opaque origins, and SSR/test
// runs may run before window is fully wired. Guard every call.
function safeStorage(): Storage | null {
  if (typeof browser === 'undefined' || !browser) return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

class AuthStore {
  token = $state<string | null>(null);
  user  = $state<User | null>(null);
  initialized = $state(false);

  constructor() {
    const store = safeStorage();
    if (store) {
      this.token = store.getItem(STORAGE_KEY_TOKEN);
      const u = store.getItem(STORAGE_KEY_USER);
      if (u) this.user = JSON.parse(u);
      this.initialized = true;
      // Sync across tabs
      window.addEventListener('storage', (e) => {
        if (e.key === STORAGE_KEY_TOKEN) {
          this.token = e.newValue;
        } else if (e.key === STORAGE_KEY_USER) {
          this.user = e.newValue ? JSON.parse(e.newValue) : null;
        }
      });
    } else {
      this.initialized = true;
    }
  }

  set(token: string, user: User) {
    this.token = token;
    this.user  = user;
    const store = safeStorage();
    if (store) {
      store.setItem(STORAGE_KEY_TOKEN, token);
      store.setItem(STORAGE_KEY_USER, JSON.stringify(user));
    }
  }

  clear() {
    this.token = null;
    this.user  = null;
    const store = safeStorage();
    if (store) {
      store.removeItem(STORAGE_KEY_TOKEN);
      store.removeItem(STORAGE_KEY_USER);
    }
  }

  get authenticated() { return this.token !== null; }
  get isAdmin()  { return this.user?.role === 'admin'; }
  // App-wide administrator — NOT campaign master. Campaign master
  // status is per-campaign (use campaign().isMaster).
  // L-F13: removed the isMaster alias. It was confusing (same name as
  // the per-campaign master role) and pointed at app-wide admin. No
  // component code referenced it (test only).
  get isAppAdmin() { return this.user?.role === 'admin'; }
}

export const auth = new AuthStore();
