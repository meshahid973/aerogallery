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
