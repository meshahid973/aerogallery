const endpoint = 'https://api.github.com/repos/meshahid973/aerogallery/releases/latest';
const cacheKey = 'aerogallery.release-check.v1';
const cacheAge = 6 * 60 * 60 * 1000;

interface Version {
  numbers: [number, number, number];
  prerelease: boolean;
}

function parseVersion(tag: string): Version | null {
  if (tag.length > 100) return null;
  const match = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(tag);
  if (!match || match[0] !== tag) return null;
  const numbers: Version['numbers'] = [Number(match[1]), Number(match[2]), Number(match[3])];
  return numbers.every(Number.isSafeInteger) ? { numbers, prerelease: !!match[4] } : null;
}

export function isNewerRelease(tag: string, installed: string): boolean {
  const latest = parseVersion(tag);
  const current = parseVersion(installed);
  if (!latest || latest.prerelease || !current) throw new Error('Unsupported release version.');
  for (const index of [0, 1, 2] as const) {
    if (latest.numbers[index] !== current.numbers[index]) return latest.numbers[index] > current.numbers[index];
  }
  return current.prerelease;
}

export interface ReleaseCheck { tag: string | null; checkedAt: number }

function stableTag(value: unknown): value is string {
  if (typeof value !== 'string') return false;
  const version = parseVersion(value);
  return version !== null && !version.prerelease;
}

function readCache(): ReleaseCheck | null {
  try {
    const data: unknown = JSON.parse(localStorage.getItem(cacheKey) ?? 'null');
    if (!data || typeof data !== 'object') return null;
    const { tag, checkedAt } = data as Partial<ReleaseCheck>;
    if ((tag !== null && !stableTag(tag)) || typeof checkedAt !== 'number' || !Number.isFinite(checkedAt)) return null;
    const age = Date.now() - checkedAt;
    return age >= 0 && age < cacheAge ? { tag, checkedAt } : null;
  } catch { return null; }
}

let inFlight: Promise<ReleaseCheck> | null = null;

async function requestRelease(): Promise<ReleaseCheck> {
  const response = await fetch(endpoint, {
    headers: { Accept: 'application/vnd.github+json' },
    credentials: 'omit',
    redirect: 'error',
    cache: 'no-store',
    signal: AbortSignal.timeout(8000),
  });
  let tag: string | null = null;
  if (response.status !== 404) {
    if (!response.ok) throw new Error(response.status === 403 || response.status === 429 ? 'GitHub is limiting requests. Try again later.' : 'Could not check GitHub releases.');
    const data: unknown = await response.json();
    if (!data || typeof data !== 'object') throw new Error('Invalid release response.');
    const release = data as { tag_name?: unknown; draft?: unknown; prerelease?: unknown };
    if (release.draft !== false || release.prerelease !== false || !stableTag(release.tag_name)) throw new Error('Unsupported release version.');
    tag = release.tag_name;
  }
  const result = { tag, checkedAt: Date.now() };
  try { localStorage.setItem(cacheKey, JSON.stringify(result)); } catch { /* The check still succeeded. */ }
  return result;
}

export function checkRelease(force = false): Promise<ReleaseCheck> {
  if (inFlight) return inFlight;
  if (!force) {
    const cached = readCache();
    if (cached) return Promise.resolve(cached);
  }
  inFlight = requestRelease().finally(() => { inFlight = null; });
  return inFlight;
}
