import { browser } from '$app/environment';
import { addMessages, init, getLocaleFromNavigator, locale, _ } from 'svelte-i18n';
import en from './en.json';
import it from './it.json';

addMessages('en', en);
addMessages('it', it);
init({
  fallbackLocale: 'en',
  initialLocale: browser ? (getLocaleFromNavigator() ?? 'en') : 'en',
});

// kept for backward compat; safe no-op
export function initI18n() {}

export { locale, _ };
