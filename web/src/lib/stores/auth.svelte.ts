import { browser } from '$app/environment';
import type { User } from '$lib/types';

class AuthStore {
  token = $state<string | null>(null);
  user  = $state<User | null>(null);

  constructor() {
    if (browser) {
      this.token = localStorage.getItem('cinghialapp.token');
      const u = localStorage.getItem('cinghialapp.user');
      if (u) this.user = JSON.parse(u);
    }
  }

  set(token: string, user: User) {
    this.token = token;
    this.user  = user;
    if (browser) {
      localStorage.setItem('cinghialapp.token', token);
      localStorage.setItem('cinghialapp.user', JSON.stringify(user));
    }
  }

  clear() {
    this.token = null;
    this.user  = null;
    if (browser) {
      localStorage.removeItem('cinghialapp.token');
      localStorage.removeItem('cinghialapp.user');
    }
  }

  get authenticated() { return this.token !== null; }
  get isAdmin()  { return this.user?.role === 'admin'; }
  // Back-compat alias used by older components — points at app-wide admin now.
  get isMaster() { return this.user?.role === 'admin'; }
}

export const auth = new AuthStore();
