import { useDeferredValue, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { AeroButton, AeroIconButton, AeroInput, AeroSelect, AeroToolbar } from './components/aero/controls';
import { AeroIcon } from './components/aero/Icon';
import { CaptionBar } from './components/aero/CaptionBar';
import { AppearancePanel } from './components/aero/AppearancePanel';
import { folderKey, isDesktop } from './lib/desktop';
import { FolderTree } from './features/gallery/FolderTree';
import { GalleryPane } from './features/gallery/GalleryPane';
import type { SortOrder } from './features/gallery/GalleryPane';
import { useGallery } from './features/gallery/useGallery';
import { useNativeChrome } from './lib/chrome';
import { useShell } from './features/gallery/useShell';
import styles from './App.module.css';

export default function App() {
  useNativeChrome();
  const [sort, setSort] = useState<SortOrder>('name');
  const [size, setSize] = useState(1);
  const [previewRevision, setPreviewRevision] = useState(0);
  const shell = useRef<HTMLDivElement>(null);
  const [search, setSearch] = useState({ folder: '', value: '' });
  const deferredSearch = useDeferredValue(search);
  const gallery = useGallery();
  const { active, roots, busy, listing } = gallery;
  const incoming = useShell(busy, gallery.acceptRoots);
  const key = active ? folderKey(active) : 'library';
  const root = roots.find((item) => item.id === active?.rootId);
  const query = search.folder === key ? search.value : '';
  const galleryQuery = deferredSearch.folder === key ? deferredSearch.value : '';
  const breadcrumbs = active && root ? [
    { name: root.name, parts: [] as string[] },
    ...active.parts.map((name, index) => ({ name, parts: active.parts.slice(0, index + 1) })),
  ] : [];

  function refresh() {
    if (!active || busy) return;
    setPreviewRevision((value) => value + 1);
    void gallery.refresh();
  }

  function shortcuts(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented || shell.current?.querySelector('dialog[open]')) return;
    const pressed = event.key.toLowerCase();
    const searchInput = shell.current?.querySelector<HTMLInputElement>('input[type="search"]');
    if (event.ctrlKey && !event.altKey && !event.shiftKey && pressed === 'f') {
      if (searchInput && !searchInput.disabled) { event.preventDefault(); searchInput.focus(); searchInput.select(); }
    } else if (event.ctrlKey && !event.altKey && !event.shiftKey && pressed === 'o') {
      if (isDesktop && !busy) { event.preventDefault(); void gallery.add(); }
    } else if (pressed === 'f5' && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      refresh();
    } else if (event.altKey && !event.ctrlKey && !busy) {
      if (pressed === 'arrowleft' && gallery.canBack) { event.preventDefault(); void gallery.back(); }
      if (pressed === 'arrowup' && active?.parts.length) { event.preventDefault(); void gallery.select({ ...active, parts: active.parts.slice(0, -1) }); }
    } else if (pressed === 'escape' && event.target === searchInput && query) {
      event.preventDefault();
      setSearch({ folder: key, value: '' });
    }
  }

  return (
    <div ref={shell} onKeyDown={shortcuts} className={styles.shell} data-native-glass="false" data-dragging={incoming.dragging}>
      <CaptionBar />
      <AeroToolbar className={styles.commands} data-tone="glass" aria-label="Library commands">
        <div className={styles.actions}>
          <AeroButton variant="accent" disabled={!isDesktop || busy} title="Add folder (Ctrl+O)" onClick={() => void gallery.add()}>
            <AeroIcon name="openFolder" size={20} /> Add folder
          </AeroButton>
          <AeroIconButton variant="toolbar" icon="back" label="Back (Alt+Left)" disabled={!gallery.canBack || busy} onClick={() => void gallery.back()} />
          <AeroIconButton variant="toolbar" icon="up" label="Parent folder (Alt+Up)" disabled={!active?.parts.length || busy} onClick={() => active && void gallery.select({ ...active, parts: active.parts.slice(0, -1) })} />
          <AeroIconButton variant="toolbar" icon="refresh" label="Refresh folder (F5)" disabled={!active || busy} onClick={refresh} />
          <AeroIconButton variant="toolbar" icon="remove" label="Remove source folder from library" disabled={!active || busy} onClick={() => void gallery.remove()} />
        </div>
        <nav className={styles.breadcrumbs} aria-label="Current folder">
          <AeroIcon name="folder" size={17} />
          {breadcrumbs.length ? breadcrumbs.map((item, index) => (
            <span key={index} className={styles.crumb}>
              {index > 0 && <span aria-hidden="true" className={styles.crumbDivider}>›</span>}
              {index === breadcrumbs.length - 1 ? <span aria-current="location" title={item.name}>{item.name}</span> : active && <AeroButton variant="toolbar" title={item.name} disabled={busy} onClick={() => void gallery.select({ ...active, parts: item.parts })}>{item.name}</AeroButton>}
            </span>
          )) : <span>Pictures</span>}
        </nav>
        <div className={`${styles.search} aero-recessed`}>
          <AeroIcon name="search" size={16} />
          <AeroInput type="search" aria-label="Search pictures" placeholder="Search pictures" title="Search pictures (Ctrl+F)" value={query} disabled={!listing || busy} onChange={(event) => setSearch({ folder: key, value: event.currentTarget.value })} />
        </div>
        <AeroSelect className={styles.sort} aria-label="Sort pictures" title="Sort pictures" value={sort} disabled={!listing || busy} onChange={(event) => {
          const value = event.currentTarget.value;
          if (value === 'name' || value === 'newest' || value === 'largest') setSort(value);
        }}>
          <option value="name">Name</option>
          <option value="newest">Date modified</option>
          <option value="largest">Size</option>
        </AeroSelect>
        <AeroIconButton variant="toolbar" icon="appearance" label="Appearance" popoverTarget="appearance-panel" />
      </AeroToolbar>
      <AppearancePanel shell={shell} />
      <div className={styles.workspace}>
        <FolderTree roots={roots} active={active} cache={gallery.cache} expanded={gallery.expanded} busy={busy} onSelect={(folder) => void gallery.select(folder)} onToggle={gallery.toggle} />
        <GalleryPane key={JSON.stringify([key, galleryQuery, sort])} external={incoming.external} onCloseExternal={incoming.close} shellError={incoming.error} onDismissShellError={() => incoming.setError('')} folder={active} listing={listing} busy={busy} error={gallery.error} query={galleryQuery} sort={sort} size={size} previewRevision={previewRevision} onSize={setSize} onAdd={() => void gallery.add()} />
      </div>
      {incoming.dragging && <div className={styles.dropHint} role="status">Drop pictures or folders</div>}
    </div>
  );
}
