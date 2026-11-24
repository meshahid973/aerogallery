import { useEffect, useRef, useState } from 'react';
import { readThumbnail } from '../../lib/desktop';
import type { Folder } from '../../lib/desktop';

type Job = { run: () => Promise<void>; started: boolean };
const queue: Job[] = [];
let running = 0;

function drain() {
  while (running < 2 && queue.length) {
    const job = queue.shift();
    if (!job) continue;
    job.started = true;
    running += 1;
    void job.run().finally(() => {
      running -= 1;
      drain();
    });
  }
}

function enqueue(run: Job['run']) {
  const job = { run, started: false };
  queue.push(job);
  drain();
  return () => {
    if (job.started) return false;
    const index = queue.indexOf(job);
    if (index !== -1) queue.splice(index, 1);
    return true;
  };
}

const observers = new WeakMap<Element, (visible: boolean) => void>();
let observer: IntersectionObserver | undefined;

function observe(element: Element, onVisibility: (visible: boolean) => void) {
  if (!('IntersectionObserver' in window)) {
    onVisibility(true);
    return () => {};
  }
  observer ??= new IntersectionObserver((entries) => {
    for (const entry of entries) {
      observers.get(entry.target)?.(entry.isIntersecting);
    }
  }, { rootMargin: '160px' });
  observers.set(element, onVisibility);
  observer.observe(element);
  return () => {
    observer?.unobserve(element);
    observers.delete(element);
  };
}

export function useThumbnail(folder: Folder, name: string) {
  const ref = useRef<HTMLSpanElement>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    let active = true;
    let settled = false;
    let objectUrl: string | undefined;
    let cancel: (() => boolean) | undefined;
    const stop = observe(element, (visible) => {
      if (!active || settled) return;
      if (!visible) {
        if (cancel?.()) cancel = undefined;
        return;
      }
      if (cancel) return;
      cancel = enqueue(async () => {
        try {
          const bytes = await readThumbnail(folder, name);
          if (!active) return;
          objectUrl = URL.createObjectURL(new Blob([bytes], { type: new Uint8Array(bytes, 0, Math.min(2, bytes.byteLength))[0] === 0xff ? 'image/jpeg' : 'image/png' }));
          setUrl(objectUrl);
        } catch {
          if (active) setFailed(true);
        } finally {
          settled = true;
          if (active) stop();
        }
      });
    });
    return () => {
      active = false;
      stop();
      cancel?.();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [folder, name]);
  return { ref, url, failed };
}
