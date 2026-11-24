import { useEffect, useRef, useState } from 'react';
import { readFullImage } from '../../lib/desktop';
import type { Folder, Photo } from '../../lib/desktop';

const CACHE_LIMIT = 96 * 1024 * 1024;
const PRELOAD_FILE_LIMIT = 16 * 1024 * 1024;
const PRELOAD_PIXEL_LIMIT = 12_000_000;

export interface ViewerAsset { key: string; url: string; bytes: number }
interface Result { key: string; asset?: ViewerAsset; error?: string }

export const photoKey = (photo: Photo | undefined) => photo ? JSON.stringify([photo.name, photo.stamp]) : '';

let readTail: Promise<unknown> = Promise.resolve();
function serializeRead(folder: Folder, photo: Photo, valid: () => boolean) {
  const job = readTail.then(() => {
    if (!valid()) throw new Error('Cancelled');
    return readFullImage(folder, photo);
  });
  readTail = job.catch(() => {});
  return job;
}

function mime(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer, 0, Math.min(12, buffer.byteLength));
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return 'image/jpeg';
  if (bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47) return 'image/png';
  if (String.fromCharCode(...bytes.subarray(0, 4)) === 'RIFF' && String.fromCharCode(...bytes.subarray(8, 12)) === 'WEBP') return 'image/webp';
  throw new Error('Unsupported viewer image');
}

export function useViewerAsset(folder: Folder, photo: Photo | undefined, next: Photo | undefined, previous: Photo | undefined, retry: number) {
  const cache = useRef(new Map<string, ViewerAsset>());
  const [result, setResult] = useState<Result>({ key: '' });
  const currentKey = photoKey(photo);
  const nextKey = photoKey(next);
  const previousKey = photoKey(previous);
  const requested = useRef({ photo, next, previous });
  requested.current = { photo, next, previous };

  useEffect(() => {
    const assets = cache.current;
    return () => {
      for (const asset of assets.values()) URL.revokeObjectURL(asset.url);
      assets.clear();
    };
  }, []);

  useEffect(() => {
    let active = true;
    const assets = cache.current;
    const keep = new Set([currentKey, nextKey, previousKey]);
    for (const [key, asset] of assets) {
      if (!keep.has(key)) { URL.revokeObjectURL(asset.url); assets.delete(key); }
    }
    const { photo: current, next: following, previous: preceding } = requested.current;
    if (!current) return;
    const cached = assets.get(currentKey);
    setResult({ key: currentKey, asset: cached });

    async function load(item: Photo, foreground: boolean) {
      const key = photoKey(item);
      const existing = assets.get(key);
      if (existing) return existing;
      const bytes = await serializeRead(folder, item, () => active);
      if (!active) return;
      let used = [...assets.values()].reduce((sum, asset) => sum + asset.bytes, 0);
      if (foreground) {
        for (const [otherKey, asset] of assets) {
          if (used + bytes.byteLength <= CACHE_LIMIT) break;
          URL.revokeObjectURL(asset.url);
          assets.delete(otherKey);
          used -= asset.bytes;
        }
      }
      if (used + bytes.byteLength > CACHE_LIMIT) return;
      const asset = { key, bytes: bytes.byteLength, url: URL.createObjectURL(new Blob([bytes], { type: mime(bytes) })) };
      assets.set(key, asset);
      return asset;
    }

    async function run(item: Photo) {
      try {
        const asset = cached ?? await load(item, true);
        if (!active) return;
        if (!asset) throw new Error('Picture exceeds the viewer memory budget.');
        setResult({ key: currentKey, asset });
      } catch (failure) {
        if (active) setResult({ key: currentKey, error: typeof failure === 'string' ? failure : failure instanceof Error ? failure.message : 'Cannot open picture.' });
        return;
      }
      for (const adjacent of [following, preceding]) {
        if (!active) break;
        if (!adjacent || photoKey(adjacent) === currentKey || adjacent.bytes > PRELOAD_FILE_LIMIT ||
          (adjacent.width !== null && adjacent.height !== null && adjacent.width * adjacent.height > PRELOAD_PIXEL_LIMIT)) continue;
        if ([...assets.values()].reduce((sum, asset) => sum + asset.bytes, 0) + adjacent.bytes > CACHE_LIMIT) continue;
        try { await load(adjacent, false); } catch { /* A foreground request can retry. */ }
      }
    }
    void run(current);
    return () => { active = false; };
  }, [folder, currentKey, nextKey, previousKey, retry]);

  return result.key === currentKey ? result : { key: currentKey };
}
