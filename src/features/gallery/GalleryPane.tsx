import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from 'react';
import { AeroButton, AeroRange, AeroToolbar } from '../../components/aero/controls';
import { AeroIcon } from '../../components/aero/Icon';
import { isDesktop, readMetadata } from '../../lib/desktop';
import type { Folder, Listing, PhotoMetadata } from '../../lib/desktop';
import { VirtualGrid } from './VirtualGrid';
import styles from './Gallery.module.css';

const PhotoViewer = lazy(() => import('../viewer/PhotoViewer'));

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
  listing: Listing | null;
  busy: boolean;
  error: string;
  sort: SortOrder;
  query: string;
  size: number;
  previewRevision: number;
  onSize: (size: number) => void;
  onAdd: () => void;
}

export function GalleryPane({ folder, listing, busy, error, query, sort, size, previewRevision, onSize, onAdd }: Props) {
  const [selected, setSelected] = useState<string | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const open = useCallback((name: string) => { setSelected(name); setViewing(name); }, []);
  const close = useCallback(() => setViewing(null), []);
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
  const width = selectedPhoto?.width;
  const height = selectedPhoto?.height;
  useEffect(() => {
    let active = true;
    setMetadata(null);
    if (folder && photoName && stamp) void readMetadata(folder, photoName).then((result) => {
      if (active) setMetadata(result);
    }, () => {});
    return () => { active = false; };
  }, [folder, photoName, stamp, width, height]);
  const metadataTitle = metadata ? [metadata.camera_make, metadata.camera_model, metadata.captured_at, metadata.iso && `ISO ${metadata.iso}`, metadata.exposure, metadata.aperture, metadata.focal_length].filter(Boolean).join(' · ') : undefined;
  const empty = !busy && !error && !photos.length;

  return (
    <main className={styles.gallery} aria-label="Pictures">
      {folder && photos.length > 0 ? <VirtualGrid key={JSON.stringify([query, sort])} previewRevision={previewRevision} folder={folder} photos={photos} selected={selected} size={size} onSelect={setSelected} onOpen={open} error={error} busy={busy} /> : <div className={styles.content} aria-busy={busy}>
        {error && <p role="alert" className={styles.error}>{error}</p>}
        {empty && (
          <div className={styles.empty}>
            <AeroIcon name={folder ? 'folder' : 'picture'} size={48} />
            {folder ? <p>{query.trim() ? 'No matching pictures' : 'No pictures in this folder'}</p> : isDesktop ? <AeroButton variant="accent" onClick={onAdd}><AeroIcon name="openFolder" size={20} /> Add folder</AeroButton> : <p>Open the desktop app</p>}
          </div>
        )}
      </div>}
      <AeroToolbar className={styles.status}>
        <span className={styles.count} role="status" title={metadataTitle}>{busy ? <span className={styles.progress}><span className="aero-spinner" aria-hidden="true" /> Opening folder…</span> : selectedPhoto ? `${selectedPhoto.name} · ${formatBytes(selectedPhoto.bytes)}${(selectedPhoto.width ?? metadata?.width) && (selectedPhoto.height ?? metadata?.height) ? ` · ${selectedPhoto.width ?? metadata?.width} × ${selectedPhoto.height ?? metadata?.height}` : ''}` : `${photos.length.toLocaleString()} ${photos.length === 1 ? 'picture' : 'pictures'}${listing?.skipped ? ` · ${listing.skipped} unreadable` : ''}`}</span>
        <AeroRange min={0} max={2} step={1} value={size} disabled={!listing} aria-label="Thumbnail size" aria-valuetext={sizes[size]} title={`Thumbnail size: ${sizes[size]}`} onChange={(event) => onSize(Number(event.currentTarget.value))} />
        <AeroButton variant="toolbar" disabled={!selectedPhoto} onClick={() => selectedPhoto && open(selectedPhoto.name)}>View</AeroButton>
      </AeroToolbar>
      {folder && viewing !== null && <Suspense fallback={null}><PhotoViewer folder={folder} photos={photos} name={viewing} onNavigate={open} onClose={close} /></Suspense>}
    </main>
  );
}
