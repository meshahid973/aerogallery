import { useCallback, useEffect, useRef, useState } from 'react';
import { folderKey, isDesktop, listRoots, pickFolder, readFolder, removeRoot, rootFolder } from '../../lib/desktop';
import type { Folder, Listing, Root } from '../../lib/desktop';

export type FolderCache = ReadonlyMap<string, string[]>;
type TreeState = { cache: FolderCache; expanded: ReadonlySet<string> };

export function useGallery() {
  const [roots, setRoots] = useState<Root[]>([]);
  const [active, setActive] = useState<Folder | null>(null);
  const activeKey = useRef<string | null>(null);
  const [listing, setListing] = useState<Listing | null>(null);
  const [tree, setTree] = useState<TreeState>({ cache: new Map(), expanded: new Set() });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [revision, setRevision] = useState(0);
  const pending = useRef(false);

  const load = useCallback(async (folder: Folder) => {
    const key = folderKey(folder);
    if (activeKey.current !== key) {
      activeKey.current = key;
      setActive(folder);
      setListing(null);
    }
    const result = await readFolder(folder);
    const ancestors = folder.parts.map((_, index) => folderKey({ ...folder, parts: folder.parts.slice(0, index) }));
    const protectedKeys = new Set([...ancestors, key]);
    setTree((previous) => {
      const cache = new Map(previous.cache);
      cache.delete(key);
      cache.set(key, result.folders);
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
    setListing(result);
    setRevision((previous) => previous + 1);
  }, []);

  const execute = useCallback(async (action: () => Promise<void>) => {
    if (!isDesktop || pending.current) return;
    pending.current = true;
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
      const first = items[0];
      if (first) void execute(() => load(rootFolder(first)));
    }, () => {
      if (activeEffect && !pending.current) setError('Cannot connect to the library.');
    });
    return () => { activeEffect = false; };
  }, [execute, load]);

  const select = (folder: Folder) => execute(() => load(folder));

  const add = () => execute(async () => {
    const root = await pickFolder();
    if (root === null) return;
    setRoots((previous) => previous.some((item) => item.id === root.id) ? previous : [...previous, root]);
    await load(rootFolder(root));
  });

  const toggle = (folder: Folder) => {
    const key = folderKey(folder);
    if (!tree.cache.has(key)) {
      void select(folder);
      return;
    }
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
    activeKey.current = null;
    setActive(null);
    setListing(null);
    const belongs = (key: string) => key === `[${id}]` || key.startsWith(`[${id},`);
    setTree((previous) => ({
      cache: new Map([...previous.cache].filter(([key]) => !belongs(key))),
      expanded: new Set([...previous.expanded].filter((key) => !belongs(key))),
    }));
    if (remaining[0]) await load(rootFolder(remaining[0]));
  });

  return { roots, active, listing, cache: tree.cache, expanded: tree.expanded, busy, error, revision, add, select, toggle, remove };
}
