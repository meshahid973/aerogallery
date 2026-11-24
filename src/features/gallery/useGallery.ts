import { useCallback, useEffect, useRef, useState } from 'react';
import { folderKey, folderRevision, isDesktop, listRoots, pickFolder, readFolder, removeRoot, rootFolder } from '../../lib/desktop';
import type { Folder, Listing, Root } from '../../lib/desktop';

export type FolderCache = ReadonlyMap<string, string[]>;
type TreeState = { cache: FolderCache; expanded: ReadonlySet<string> };

export function useGallery() {
  const [roots, setRoots] = useState<Root[]>([]);
  const [active, setActive] = useState<Folder | null>(null);
  const activeKey = useRef<string | null>(null);
  const activeRevision = useRef<number | null>(null);
  const history = useRef<Folder[]>([]);
  const lastLoaded = useRef<Folder | null>(null);
  const [canBack, setCanBack] = useState(false);
  const [listing, setListing] = useState<Listing | null>(null);
  const [tree, setTree] = useState<TreeState>({ cache: new Map(), expanded: new Set() });
  const [busy, setBusy] = useState(isDesktop);
  const [initializing, setInitializing] = useState(isDesktop);
  const actionEpoch = useRef(0);
  const [error, setError] = useState('');
  const pending = useRef(false);

  const apply = useCallback((folder: Folder, result: Listing) => {
    const key = folderKey(folder);
    const ancestors = folder.parts.map((_, index) => folderKey({ ...folder, parts: folder.parts.slice(0, index) }));
    const protectedKeys = new Set([...ancestors, key]);
    setTree((previous) => {
      const cache = new Map(previous.cache);
      cache.delete(key);
      cache.set(key, result.folders);
      const valid = (candidate: string) => {
        if (candidate === key || !candidate.startsWith(key.slice(0, -1) + ',')) return true;
        const parts = JSON.parse(candidate) as (number | string)[];
        return result.folders.includes(String(parts[folder.parts.length + 1]));
      };
      for (const candidate of cache.keys()) if (!valid(candidate)) cache.delete(candidate);
      let count = [...cache.values()].reduce((total, children) => total + children.length, 0);
      for (const [candidate, children] of cache) {
        if (cache.size <= 64 && count <= 100_000) break;
        if (!protectedKeys.has(candidate)) {
          cache.delete(candidate);
          count -= children.length;
        }
      }
      const expanded = new Set([...previous.expanded, ...protectedKeys].filter((candidate) => cache.has(candidate)));
      return { cache, expanded };
    });
    activeRevision.current = result.revision;
    setListing(result);
  }, []);

  const load = useCallback(async (folder: Folder, refresh = false, record = true) => {
    const key = folderKey(folder);
    if (activeKey.current !== key) {
      activeKey.current = key;
      activeRevision.current = null;
      setActive(folder);
      setListing(null);
    }
    const result = await readFolder(folder, refresh);
    if (record && lastLoaded.current && folderKey(lastLoaded.current) !== key) {
      history.current = [...history.current.slice(-63), lastLoaded.current];
      setCanBack(true);
    }
    lastLoaded.current = folder;
    apply(folder, result);
  }, [apply]);

  const execute = useCallback(async (action: () => Promise<void>) => {
    if (!isDesktop || pending.current) return;
    pending.current = true;
    actionEpoch.current += 1;
    setBusy(true);
    setError('');
    try {
      await action();
    } catch (failure) {
      setError(typeof failure === 'string' ? failure : 'Unable to complete the action.');
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    if (!isDesktop) return;
    let activeEffect = true;
    void listRoots().then((items) => {
      if (!activeEffect || pending.current) return;
      setRoots(items);
      setInitializing(false);
      setBusy(false);
      const first = items[0];
      if (first) void execute(() => load(rootFolder(first)));
    }, () => {
      if (activeEffect && !pending.current) { setInitializing(false); setBusy(false); setError('Cannot connect to the library.'); }
    });
    return () => { activeEffect = false; };
  }, [execute, load]);

  useEffect(() => {
    if (!isDesktop || !active) return;
    let stopped = false;
    let running = false;
    const key = folderKey(active);
    async function check() {
      if (stopped || running || pending.current || activeKey.current !== key || !active) return;
      running = true;
      const epoch = actionEpoch.current;
      try {
        const revision = await folderRevision(active);
        if (stopped || pending.current || epoch !== actionEpoch.current || activeKey.current !== key || revision === activeRevision.current) return;
        const result = await readFolder(active);
        if (!stopped && !pending.current && epoch === actionEpoch.current && activeKey.current === key) {
          apply(active, result);
          setError('');
        }
      } catch (failure) {
        if (!stopped && !pending.current && epoch === actionEpoch.current && activeKey.current === key) {
          activeRevision.current = null;
          setError(typeof failure === 'string' ? failure : 'Cannot read this folder.');
        }
      } finally { running = false; }
    }
    const interval = window.setInterval(() => { void check(); }, 2500);
    return () => { stopped = true; window.clearInterval(interval); };
  }, [active, apply]);

  const select = (folder: Folder) => execute(() => load(folder));
  const refresh = () => execute(async () => { if (active) await load(active, true); });
  const back = () => execute(async () => {
    const previous = history.current.pop();
    setCanBack(history.current.length > 0);
    if (previous) await load(previous, false, false);
  });

  const add = () => execute(async () => {
    const root = await pickFolder();
    if (root === null) return;
    setRoots((previous) => previous.some((item) => item.id === root.id) ? previous : [...previous, root]);
    await load(rootFolder(root));
  });

  const toggle = (folder: Folder) => {
    const key = folderKey(folder);
    if (!tree.cache.has(key)) { void select(folder); return; }
    setTree((previous) => {
      const expanded = new Set(previous.expanded);
      if (expanded.has(key)) expanded.delete(key);
      else expanded.add(key);
      return { cache: previous.cache, expanded };
    });
  };

  const remove = () => execute(async () => {
    if (!active) return;
    const id = active.rootId;
    await removeRoot(id);
    const remaining = roots.filter((root) => root.id !== id);
    setRoots(remaining);
    history.current = history.current.filter((folder) => folder.rootId !== id);
    setCanBack(history.current.length > 0);
    lastLoaded.current = null;
    activeKey.current = null;
    activeRevision.current = null;
    setActive(null);
    setListing(null);
    const belongs = (key: string) => key === `[${id}]` || key.startsWith(`[${id},`);
    setTree((previous) => ({
      cache: new Map([...previous.cache].filter(([key]) => !belongs(key))),
      expanded: new Set([...previous.expanded].filter((key) => !belongs(key))),
    }));
    if (remaining[0]) await load(rootFolder(remaining[0]));
  });

  return { roots, active, listing, cache: tree.cache, expanded: tree.expanded, busy: busy || initializing, error, canBack, back, add, select, refresh, toggle, remove };
}
