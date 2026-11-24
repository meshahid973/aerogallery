import { useCallback, useEffect, useRef, useState } from 'react';
import { folderKey, readThumbnail } from '../../lib/desktop';
import type { Folder } from '../../lib/desktop';

type Job = { run: () => Promise<void>; started: boolean };
const queue: Job[] = [];
let running = 0;
let suspended = 0;

export function suspendThumbnailQueue() {
  suspended += 1;
  let resumed = false;
  return () => {
    if (resumed) return;
    resumed = true;
    suspended -= 1;
    drain();
  };
}

function drain() {
  while (!suspended && running < 2 && queue.length) {
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
    if (!job.started) {
      const index = queue.indexOf(job);
      if (index !== -1) queue.splice(index, 1);
    }
  };
}

export function useThumbnail(folder: Folder, name: string, stamp: string, revision: number) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [attempt, setAttempt] = useState(0);
  const scope = folderKey(folder);
  const latestFolder = useRef(folder);
  latestFolder.current = folder;
  const retry = useCallback(() => setAttempt((value) => value + 1), []);
  const markFailed = useCallback(() => {
    setUrl(null);
    setError('Image could not be displayed');
  }, []);

  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    setUrl(null);
    setError('');
    const current = latestFolder.current;
    const cancel = enqueue(async () => {
      try {
        const bytes = await readThumbnail(current, name);
        if (!active) return;
        const signature = new Uint8Array(bytes, 0, Math.min(4, bytes.byteLength));
        const type = signature[0] === 0xff && signature[1] === 0xd8 ? 'image/jpeg' : signature[0] === 0x89 && signature[1] === 0x50 && signature[2] === 0x4e && signature[3] === 0x47 ? 'image/png' : '';
        if (!type) throw new Error('Invalid thumbnail data');
        objectUrl = URL.createObjectURL(new Blob([bytes], { type }));
        setUrl(objectUrl);
      } catch (failure) {
        if (active) setError(typeof failure === 'string' ? failure : failure instanceof Error ? failure.message : 'Thumbnail unavailable');
      }
    });
    return () => {
      active = false;
      cancel();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [scope, name, stamp, attempt, revision]);

  return { url, error, retry, markFailed };
}
