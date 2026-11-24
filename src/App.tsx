import { useState } from 'react';
import { AeroButton, AeroGlass, AeroIconButton, AeroSelect, AeroToolbar } from './components/aero/controls';
import { AeroIcon } from './components/aero/Icon';
import { glassDensities, readDensity, saveDensity } from './lib/appearance';
import type { GlassDensity } from './lib/appearance';
import { folderKey, isDesktop } from './lib/desktop';
import { FolderTree } from './features/gallery/FolderTree';
import { GalleryPane } from './features/gallery/GalleryPane';
import type { SortOrder } from './features/gallery/GalleryPane';
import { useGallery } from './features/gallery/useGallery';
import styles from './App.module.css';

export default function App() {
  const [density, setDensity] = useState<GlassDensity>(readDensity);
  const [sort, setSort] = useState<SortOrder>('name');
  const [size, setSize] = useState(1);
  const [preferenceError, setPreferenceError] = useState('');
  const gallery = useGallery();
  const { active, roots, busy } = gallery;
  const root = roots.find((item) => item.id === active?.rootId);

  function changeDensity(value: string) {
    const next = glassDensities.find((item) => item === value);
    if (!next) return;
    setDensity(next);
    setPreferenceError(saveDensity(next) ? '' : 'Appearance could not be saved.');
  }

  return (
    <div className={styles.environment} data-glass={density}>
      <AeroGlass className={styles.shell}>
        <AeroToolbar className={styles.commands} data-tone="command" aria-label="Library commands">
          <AeroButton variant="toolbar" disabled={!isDesktop || busy} onClick={() => void gallery.add()}>
            <AeroIcon name="openFolder" size={22} /> Add folder
          </AeroButton>
          <AeroIconButton variant="toolbar" icon="remove" label="Remove source folder from library" disabled={!active || busy} onClick={() => void gallery.remove()} />
          <span className="aero-separator" aria-hidden="true" />
          <AeroIconButton variant="toolbar" icon="up" label="Parent folder" disabled={!active?.parts.length || busy} onClick={() => active && void gallery.select({ ...active, parts: active.parts.slice(0, -1) })} />
          <AeroIconButton variant="toolbar" icon="refresh" label="Refresh folder" disabled={!active || busy} onClick={() => active && void gallery.select(active)} />
          <span className={styles.spacer} />
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
          <GalleryPane key={active ? folderKey(active) : 'library'} folder={active} root={root} listing={gallery.listing} revision={gallery.revision} busy={busy} error={gallery.error} sort={sort} onSort={setSort} size={size} onSize={setSize} onSelectFolder={(folder) => void gallery.select(folder)} onAdd={() => void gallery.add()} />
        </div>
      </AeroGlass>
    </div>
  );
}
