import { browser } from '$app/environment';
import type { User } from '$lib/types';

const STORAGE_KEY_TOKEN = 'dungeonsandapps.token';
const STORAGE_KEY_USER = 'dungeonsandapps.user';

class AuthStore {
  token = $state<string | null>(null);
  user  = $state<User | null>(null);

  constructor() {
    if (browser) {
      this.token = localStorage.getItem(STORAGE_KEY_TOKEN);
      const u = localStorage.getItem(STORAGE_KEY_USER);
      if (u) this.user = JSON.parse(u);
      // Sync across tabs
      window.addEventListener('storage', (e) => {
        if (e.key === STORAGE_KEY_TOKEN) {
          this.token = e.newValue;
        } else if (e.key === STORAGE_KEY_USER) {
          this.user = e.newValue ? JSON.parse(e.newValue) : null;
        }
      });
    }
  }

  set(token: string, user: User) {
    this.token = token;
    this.user  = user;
    if (browser) {
      localStorage.setItem(STORAGE_KEY_TOKEN, token);
      localStorage.setItem(STORAGE_KEY_USER, JSON.stringify(user));
    }
  }

  clear() {
    this.token = null;
    this.user  = null;
    if (browser) {
      localStorage.removeItem(STORAGE_KEY_TOKEN);
      localStorage.removeItem(STORAGE_KEY_USER);
    }
  }

  get authenticated() { return this.token !== null; }
  get isAdmin()  { return this.user?.role === 'admin'; }
  // App-wide administrator — NOT campaign master.
  get isAppAdmin() { return this.user?.role === 'admin'; }
  // Back-compat alias used by older components — points at app-wide admin now.
  get isMaster() { return this.user?.role === 'admin'; }
}

export const auth = new AuthStore();
