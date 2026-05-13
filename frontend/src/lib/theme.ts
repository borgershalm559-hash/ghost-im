import { getSetting, setSetting } from './tauri';

export type Theme = 'dark' | 'light';

const KEY = 'theme';

/** Apply theme by writing `data-theme` to <html>. Pure DOM, no I/O. */
export function applyTheme(t: Theme) {
  document.documentElement.dataset.theme = t;
}

/** Load theme from settings (default `dark`) and apply. Idempotent. */
export async function bootTheme(): Promise<Theme> {
  let stored: string | null = null;
  try {
    stored = await getSetting(KEY);
  } catch {
    // Client may not be open yet (onboarding flow). Default to dark.
    stored = null;
  }
  const t: Theme = stored === 'light' ? 'light' : 'dark';
  applyTheme(t);
  return t;
}

/** Set theme: persist + apply. */
export async function persistTheme(t: Theme): Promise<void> {
  await setSetting(KEY, t);
  applyTheme(t);
}
