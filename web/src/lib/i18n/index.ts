import { browser } from '$app/environment';
import { addMessages, init, getLocaleFromNavigator, locale, _ } from 'svelte-i18n';
import { auth } from '$lib/stores/auth.svelte';
import en from './en.json';
import it from './it.json';

addMessages('en', en);
addMessages('it', it);

function getInitialLocale(): string {
  if (!browser) return 'en';
  // Use user preference if available, fallback to browser language
  const userLang = auth.user?.language;
  if (userLang === 'en' || userLang === 'it') return userLang;
  return getLocaleFromNavigator() ?? 'en';
}

init({
  fallbackLocale: 'en',
  initialLocale: getInitialLocale(),
});

// Update locale when user changes
if (browser) {
  auth.user?.language && locale.set(auth.user.language);
}

export function initI18n() {}
export { locale, _ };
