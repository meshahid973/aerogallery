import { useEffect, useRef, useState } from 'react';
import { readThumbnail } from '../../lib/desktop';
import type { Folder } from '../../lib/desktop';

type Job = { run: () => Promise<void>; cancelled: boolean };
const queue: Job[] = [];
let running = 0;

function drain() {
  while (running < 2 && queue.length) {
    const job = queue.shift();
    if (!job || job.cancelled) continue;
    running += 1;
    void job.run().finally(() => {
      running -= 1;
      drain();
    });
  }
}

function enqueue(run: Job['run']) {
  const job = { run, cancelled: false };
  queue.push(job);
  drain();
  return () => {
    job.cancelled = true;
    const index = queue.indexOf(job);
    if (index !== -1) queue.splice(index, 1);
  };
}

const observers = new WeakMap<Element, () => void>();
let observer: IntersectionObserver | undefined;

function observe(element: Element, onVisible: () => void) {
  if (!('IntersectionObserver' in window)) {
    onVisible();
    return () => {};
  }
  observer ??= new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      observer?.unobserve(entry.target);
      observers.get(entry.target)?.();
      observers.delete(entry.target);
    }
  }, { rootMargin: '160px' });
  observers.set(element, onVisible);
  observer.observe(element);
  return () => {
    observer?.unobserve(element);
    observers.delete(element);
  };
}

export function useThumbnail(folder: Folder, name: string) {
  const ref = useRef<HTMLSpanElement>(null);
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    let active = true;
    let objectUrl: string | undefined;
    let cancel: (() => void) | undefined;
    const stop = observe(element, () => {
      cancel = enqueue(async () => {
        try {
          const bytes = await readThumbnail(folder, name);
          if (!active) return;
          objectUrl = URL.createObjectURL(new Blob([bytes], { type: 'image/png' }));
          setUrl(objectUrl);
        } catch {
          if (active) setUrl(null);
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
  return { ref, url };
}
