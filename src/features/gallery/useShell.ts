import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { isDesktop, takeShellItems, releaseExternal } from '../../lib/desktop';
import type { ExternalSelection, Root } from '../../lib/desktop';

export function useShell(busy: boolean, addRoots: (roots: Root[]) => Promise<void>) {
  const [external, setExternal] = useState<ExternalSelection | null>(null);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const mounted = useRef(false);
  const running = useRef(false);
  const pending = useRef(false);
  const current = useRef({ addRoots });
  current.current = { addRoots };

  useEffect(() => {
    if (!isDesktop) return;
    mounted.current = true;
    let active = true;
    const releases: (() => void)[] = [];
    const wake = () => {
      if (!active) return;
      pending.current = true;
      setRevision((value) => value + 1);
    };
    void listen('shell-selection', wake).then((release) => {
      if (!active) { release(); return; }
      releases.push(release);
      wake();
    }, () => { if (active) setError('Cannot listen for Explorer selections.'); });
    void listen<boolean>('shell-drag', ({ payload }) => { if (active) setDragging(payload); }).then((release) => {
      if (active) releases.push(release); else release();
    }, () => {});
    return () => { mounted.current = false; active = false; for (const release of releases) release(); };
  }, []);

  useEffect(() => {
    if (!isDesktop || busy || running.current || !pending.current) return;
    running.current = true;
    pending.current = false;
    void (async () => {
      try {
        const items = await takeShellItems();
        if (!items) return;
        if (!mounted.current) { if (items.session) void releaseExternal(items.session).catch(() => {}); return; }
        if (items.roots.length) await current.current.addRoots(items.roots);
        if (!mounted.current) { if (items.session) void releaseExternal(items.session).catch(() => {}); return; }
        if (items.photos.length) setExternal(items);
        setError(items.rejected ? `${items.rejected} unsupported or inaccessible item(s).` : '');
      } catch (failure) {
        if (mounted.current) setError(typeof failure === 'string' ? failure : 'Cannot open selected items.');
      } finally {
        running.current = false;
        if (mounted.current && pending.current) setRevision((value) => value + 1);
      }
    })();
  }, [busy, revision]);

  function close() {
    if (external) void releaseExternal(external.session).catch(() => {});
    setExternal(null);
  }
  return { external, dragging, error, setError, close };
}
