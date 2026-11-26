export const glassDensities = ['clear', 'balanced', 'solid'] as const;
export type GlassDensity = typeof glassDensities[number];

const key = 'aerogallery.glassDensity';

export function readDensity(): GlassDensity {
  try {
    const value = localStorage.getItem(key);
    return glassDensities.find((density) => density === value) ?? 'balanced';
  } catch {
    return 'balanced';
  }
}

export function saveDensity(value: GlassDensity): boolean {
  try {
    localStorage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

export const themes = ['aero', 'aqua', 'aurora', 'frost'] as const;
export type Theme = typeof themes[number];
export const themeNames: Record<Theme, string> = { aero: 'Aero', aqua: 'Aqua', aurora: 'Aurora', frost: 'Frost' };

export function readTheme(): Theme {
  try { return themes.find((theme) => theme === localStorage.getItem('aerogallery.theme')) ?? 'aero'; }
  catch { return 'aero'; }
}

export function saveTheme(theme: Theme): boolean {
  try { localStorage.setItem('aerogallery.theme', theme); return true; }
  catch { return false; }
}
