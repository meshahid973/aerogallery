export const themes = ['aero', 'aqua', 'aurora', 'frost'] as const;
export type Theme = typeof themes[number];
export const themeNames: Record<Theme, string> = { aero: 'Aero', aqua: 'Aqua', aurora: 'Aurora', frost: 'Frost' };
export const themeTints: Record<Theme, string> = { aero: '#b2dff7', aqua: '#92e8df', aurora: '#c7b4ef', frost: '#dde7f2' };
export interface Appearance { theme: Theme; tint: string | null; opacity: number; reflection: number }
export const defaults: Appearance = { theme: 'aero', tint: null, opacity: 78, reflection: 65 };
const key = 'aerogallery.appearance.v2';
const themeOf = (value: unknown): Theme => themes.find((theme) => theme === value) ?? defaults.theme;
const percent = (value: unknown, fallback: number, min = 0) => typeof value === 'number' && Number.isFinite(value) ? Math.max(min, Math.min(100, Math.round(value))) : fallback;
export const tintOf = (value: Appearance) => value.tint ?? themeTints[value.theme];
export function tintChannels(value: Appearance): [number, number, number] {
  const tint = tintOf(value);
  return [1, 3, 5].map((offset) => parseInt(tint.slice(offset, offset + 2), 16)) as [number, number, number];
}
export function readAppearance(): Appearance {
  try {
    const raw = localStorage.getItem(key);
    if (raw) {
      const data = JSON.parse(raw);
      if (data && typeof data === 'object') return {
        theme: themeOf(data.theme),
        tint: typeof data.tint === 'string' && /^#[0-9a-f]{6}$/i.test(data.tint) ? data.tint : null,
        opacity: percent(data.opacity, defaults.opacity, 35),
        reflection: percent(data.reflection, defaults.reflection),
      };
    }
  } catch { /* Recover old preferences or defaults. */ }
  try {
    const density = localStorage.getItem('aerogallery.glassDensity');
    return { ...defaults, theme: themeOf(localStorage.getItem('aerogallery.theme')), opacity: density === 'clear' ? 48 : density === 'solid' ? 100 : defaults.opacity };
  } catch { return { ...defaults }; }
}
export function saveAppearance(value: Appearance): boolean {
  try { localStorage.setItem(key, JSON.stringify(value)); return true; }
  catch { return false; }
}
