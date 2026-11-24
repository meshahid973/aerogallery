import { useEffect, useMemo, useState } from 'react';
import { AeroButton, AeroInput, AeroRange, AeroSelect, AeroToolbar } from '../../components/aero/controls';
import { AeroIcon } from '../../components/aero/Icon';
import { isDesktop, readMetadata } from '../../lib/desktop';
import type { Folder, Listing, PhotoMetadata, Root } from '../../lib/desktop';
import { VirtualGrid } from './VirtualGrid';
import styles from './Gallery.module.css';

const sizes = ['Small', 'Medium', 'Large'] as const;
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
export type SortOrder = 'name' | 'newest' | 'largest';

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface Props {
  folder: Folder | null;
  root: Root | undefined;
  listing: Listing | null;
  busy: boolean;
  error: string;
  sort: SortOrder;
  onSort: (sort: SortOrder) => void;
  size: number;
  onSize: (size: number) => void;
  onSelectFolder: (folder: Folder) => void;
  onAdd: () => void;
}

export function GalleryPane({ folder, root, listing, busy, error, sort, onSort, size, onSize, onSelectFolder, onAdd }: Props) {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const photos = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const result = (listing?.photos ?? []).filter((photo) => !needle || photo.name.toLowerCase().includes(needle));
    result.sort((a, b) => {
      const primary = sort === 'newest' ? b.modified - a.modified : sort === 'largest' ? b.bytes - a.bytes : 0;
      return primary || collator.compare(a.name, b.name);
    });
    return result;
  }, [listing, query, sort]);
  const selectedPhoto = photos.find((photo) => photo.name === selected);
  const [metadata, setMetadata] = useState<PhotoMetadata | null>(null);
  const photoName = selectedPhoto?.name;
  const stamp = selectedPhoto?.stamp;
  useEffect(() => {
    let active = true;
    setMetadata(null);
    if (folder && photoName && stamp) void readMetadata(folder, photoName).then((result) => {
      if (active) setMetadata(result);
    }, () => {});
    return () => { active = false; };
  }, [folder, photoName, stamp]);
  const metadataTitle = metadata ? [metadata.camera_make, metadata.camera_model, metadata.captured_at, metadata.iso && `ISO ${metadata.iso}`, metadata.exposure, metadata.aperture, metadata.focal_length].filter(Boolean).join(' · ') : undefined;
  const breadcrumbs = folder && root ? [
    { name: root.name, parts: [] as string[] },
    ...folder.parts.map((name, index) => ({ name, parts: folder.parts.slice(0, index + 1) })),
  ] : [];

  const empty = !busy && !error && !photos.length;

  return (
    <main className={styles.gallery} aria-label="Pictures">
      <AeroToolbar className={styles.addressBar}>
        <nav className={styles.breadcrumbs} aria-label="Current folder">
          <AeroIcon name="folder" size={18} />
          {breadcrumbs.length ? breadcrumbs.map((item, index) => (
            <span key={index} className={styles.crumb}>
              {index > 0 && <span aria-hidden="true" className={styles.crumbDivider}>›</span>}
              {index === breadcrumbs.length - 1 ? <span aria-current="location" title={item.name}>{item.name}</span> : folder && <AeroButton variant="toolbar" title={item.name} disabled={busy} onClick={() => onSelectFolder({ ...folder, parts: item.parts })}>{item.name}</AeroButton>}
            </span>
          )) : <span>Pictures</span>}
        </nav>
        <div className={styles.search}>
          <AeroIcon name="search" size={16} />
          <AeroInput type="search" aria-label="Search pictures" placeholder="Search pictures" value={query} disabled={!listing || busy} onChange={(event) => { setQuery(event.currentTarget.value); setSelected(null); }} />
        </div>
        <AeroSelect aria-label="Sort pictures" title="Sort pictures" value={sort} disabled={!listing || busy} onChange={(event) => {
          const value = event.currentTarget.value;
          if (value === 'name' || value === 'newest' || value === 'largest') {
            onSort(value);
            setSelected(null);
          }
        }}>
          <option value="name">Name</option>
          <option value="newest">Date modified</option>
          <option value="largest">Size</option>
        </AeroSelect>
      </AeroToolbar>
      {folder && photos.length > 0 ? <VirtualGrid key={query + sort} folder={folder} photos={photos} selected={selected} size={size} onSelect={setSelected} error={error} busy={busy} /> : <div className={styles.content} aria-busy={busy}>
        {error && <p role="alert" className={styles.error}>{error}</p>}
        {busy && !listing && <div className={styles.loading} role="status"><span className="aero-spinner" /> Opening folder…</div>}
        {empty && (
          <div className={styles.empty}>
            <AeroIcon name={folder ? 'folder' : 'picture'} size={48} />
            {folder ? <p>{query.trim() ? 'No matching pictures' : 'No pictures in this folder'}</p> : isDesktop ? <AeroButton variant="accent" onClick={onAdd}><AeroIcon name="openFolder" size={20} /> Add folder</AeroButton> : <p>Open the desktop app</p>}
          </div>
        )}
      </div>}
      <AeroToolbar className={styles.status}>
        <span className={styles.count} role="status" title={metadataTitle}>{busy ? 'Opening…' : selectedPhoto ? `${selectedPhoto.name} · ${formatBytes(selectedPhoto.bytes)}${selectedPhoto.width && selectedPhoto.height ? ` · ${selectedPhoto.width} × ${selectedPhoto.height}` : ''}` : `${photos.length.toLocaleString()} ${photos.length === 1 ? 'picture' : 'pictures'}${listing?.skipped ? ` · ${listing.skipped} unreadable` : ''}`}</span>
        <AeroRange min={0} max={2} step={1} value={size} disabled={!listing} aria-label="Thumbnail size" aria-valuetext={sizes[size]} title={`Thumbnail size: ${sizes[size]}`} onChange={(event) => onSize(Number(event.currentTarget.value))} />
      </AeroToolbar>
    </main>
  );
}
