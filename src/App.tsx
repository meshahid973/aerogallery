import { useDeferredValue, useState } from 'react';
import { AeroButton, AeroIconButton, AeroInput, AeroSelect, AeroToolbar } from './components/aero/controls';
import { AeroIcon } from './components/aero/Icon';
import { glassDensities, readDensity, saveDensity } from './lib/appearance';
import type { GlassDensity } from './lib/appearance';
import { folderKey, isDesktop } from './lib/desktop';
import { FolderTree } from './features/gallery/FolderTree';
import { GalleryPane } from './features/gallery/GalleryPane';
import type { SortOrder } from './features/gallery/GalleryPane';
import { useGallery } from './features/gallery/useGallery';
import styles from './App.module.css';

const nativeGlass = isDesktop && /Windows/i.test(navigator.userAgent);

export default function App() {
  const [density, setDensity] = useState<GlassDensity>(readDensity);
  const [sort, setSort] = useState<SortOrder>('name');
  const [size, setSize] = useState(1);
  const [preferenceError, setPreferenceError] = useState('');
  const [search, setSearch] = useState({ folder: '', value: '' });
  const deferredSearch = useDeferredValue(search);
  const gallery = useGallery();
  const { active, roots, busy, listing } = gallery;
  const key = active ? folderKey(active) : 'library';
  const root = roots.find((item) => item.id === active?.rootId);
  const query = search.folder === key ? search.value : '';
  const galleryQuery = deferredSearch.folder === key ? deferredSearch.value : '';
  const breadcrumbs = active && root ? [
    { name: root.name, parts: [] as string[] },
    ...active.parts.map((name, index) => ({ name, parts: active.parts.slice(0, index + 1) })),
  ] : [];

  function changeDensity(value: string) {
    const next = glassDensities.find((item) => item === value);
    if (!next) return;
    setDensity(next);
    setPreferenceError(saveDensity(next) ? '' : 'Appearance could not be saved.');
  }

  return (
    <div className={styles.shell} data-glass={density} data-native-glass={nativeGlass}>
      <AeroToolbar className={styles.commands} data-tone="glass" aria-label="Library commands">
        <div className={styles.actions}>
          <AeroButton variant="toolbar" disabled={!isDesktop || busy} onClick={() => void gallery.add()}>
            <AeroIcon name="openFolder" size={22} /> Add folder
          </AeroButton>
          <AeroIconButton variant="toolbar" icon="back" label="Back" disabled={!gallery.canBack || busy} onClick={() => void gallery.back()} />
          <AeroIconButton variant="toolbar" icon="up" label="Parent folder" disabled={!active?.parts.length || busy} onClick={() => active && void gallery.select({ ...active, parts: active.parts.slice(0, -1) })} />
          <AeroIconButton variant="toolbar" icon="refresh" label="Refresh folder" disabled={!active || busy} onClick={() => void gallery.refresh()} />
          <AeroIconButton variant="toolbar" icon="remove" label="Remove source folder from library" disabled={!active || busy} onClick={() => void gallery.remove()} />
        </div>
        <nav className={styles.breadcrumbs} aria-label="Current folder">
          {breadcrumbs.length ? breadcrumbs.map((item, index) => (
            <span key={index} className={styles.crumb}>
              {index > 0 && <span aria-hidden="true" className={styles.crumbDivider}>›</span>}
              {index === breadcrumbs.length - 1 ? <span aria-current="location" title={item.name}>{item.name}</span> : active && <AeroButton variant="toolbar" title={item.name} disabled={busy} onClick={() => void gallery.select({ ...active, parts: item.parts })}>{item.name}</AeroButton>}
            </span>
          )) : <span>Pictures</span>}
        </nav>
        <div className={styles.search}>
          <AeroIcon name="search" size={16} />
          <AeroInput type="search" aria-label="Search pictures" placeholder="Search pictures" value={query} disabled={!listing || busy} onChange={(event) => setSearch({ folder: key, value: event.currentTarget.value })} />
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
      <div id="appearance-panel" popover="auto" className={styles.appearance}>
        <label htmlFor="glass-density">Glass density</label>
        <AeroSelect id="glass-density" value={density} onChange={(event) => changeDensity(event.currentTarget.value)}>
          <option value="clear">Clear</option>
          <option value="balanced">Balanced</option>
          <option value="solid">Solid</option>
        </AeroSelect>
        {preferenceError && <p role="status" className={styles.preferenceError}>{preferenceError}</p>}
      </div>
      <div className={styles.workspace}>
        <FolderTree roots={roots} active={active} cache={gallery.cache} expanded={gallery.expanded} busy={busy} onSelect={(folder) => void gallery.select(folder)} onToggle={gallery.toggle} />
        <GalleryPane key={JSON.stringify([key, galleryQuery, sort])} folder={active} listing={listing} busy={busy} error={gallery.error} query={galleryQuery} sort={sort} size={size} onSize={setSize} onAdd={() => void gallery.add()} />
      </div>
    </div>
  );
}
