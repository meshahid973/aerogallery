import { memo, useMemo, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { AeroButton, AeroIconButton, AeroInput, AeroRange, AeroSelect, AeroToolbar } from '../../components/aero/controls';
import { AeroIcon } from '../../components/aero/Icon';
import { folderKey, isDesktop } from '../../lib/desktop';
import type { Folder, Listing, Photo, Root } from '../../lib/desktop';
import { useThumbnail } from './thumbnails';
import styles from './Gallery.module.css';

const PAGE_SIZE = 60;
const sizes = ['Small', 'Medium', 'Large'] as const;
const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });
export type SortOrder = 'name' | 'newest' | 'largest';

const PhotoTile = memo(function PhotoTile({ folder, photo, selected, tabbable, onSelect }: {
  folder: Folder;
  photo: Photo;
  selected: boolean;
  tabbable: boolean;
  onSelect: (name: string) => void;
}) {
  const { ref, url, failed } = useThumbnail(folder, photo.name);
  return (
    <li className={styles.photoCell}>
      <button type="button" className={styles.photo} data-photo="" aria-pressed={selected} tabIndex={tabbable ? 0 : -1} onClick={() => onSelect(photo.name)} title={failed ? `${photo.name} — preview unavailable` : photo.name}>
        <span className={styles.imageFrame} ref={ref}>
          {url ? <img className={styles.thumbnail} src={url} alt="" draggable={false} decoding="async" /> : <AeroIcon name="picture" size={40} />}
        </span>
        <span className={styles.filename}>{photo.name}</span>
      </button>
    </li>
  );
});

function navigateGrid(event: KeyboardEvent<HTMLUListElement>) {
  const target = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-photo]');
  if (!target) return;
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return;
  const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('button[data-photo]'));
  const index = buttons.indexOf(target);
  if (index < 0) return;
  const columns = Math.max(1, getComputedStyle(event.currentTarget).gridTemplateColumns.split(' ').length);
  const step: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns };
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : Math.max(0, Math.min(buttons.length - 1, index + (step[event.key] ?? 0)));
  event.preventDefault();
  buttons[next]?.focus();
  buttons[next]?.click();
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface Props {
  folder: Folder | null;
  root: Root | undefined;
  listing: Listing | null;
  revision: number;
  busy: boolean;
  error: string;
  sort: SortOrder;
  onSort: (sort: SortOrder) => void;
  size: number;
  onSize: (size: number) => void;
  onSelectFolder: (folder: Folder) => void;
  onAdd: () => void;
}

export function GalleryPane({ folder, root, listing, revision, busy, error, sort, onSort, size, onSize, onSelectFolder, onAdd }: Props) {
  const [query, setQuery] = useState('');
  const [page, setPage] = useState(0);
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
  const pageCount = Math.max(1, Math.ceil(photos.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount - 1);
  const visible = photos.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE);
  const selectedPhoto = visible.find((photo) => photo.name === selected);
  const tabbableName = selectedPhoto?.name ?? visible[0]?.name;
  const breadcrumbs = folder && root ? [
    { name: root.name, parts: [] as string[] },
    ...folder.parts.map((name, index) => ({ name, parts: folder.parts.slice(0, index + 1) })),
  ] : [];

  function changePage(next: number) {
    setPage(next);
    setSelected(null);
  }

  const empty = !busy && !error && !visible.length;

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
          <AeroInput type="search" aria-label="Search pictures" placeholder="Search pictures" value={query} disabled={!listing || busy} onChange={(event) => { setQuery(event.currentTarget.value); changePage(0); }} />
        </div>
        <AeroSelect aria-label="Sort pictures" title="Sort pictures" value={sort} disabled={!listing || busy} onChange={(event) => {
          const value = event.currentTarget.value;
          if (value === 'name' || value === 'newest' || value === 'largest') {
            onSort(value);
            changePage(0);
          }
        }}>
          <option value="name">Name</option>
          <option value="newest">Date modified</option>
          <option value="largest">Size</option>
        </AeroSelect>
      </AeroToolbar>
      <div className={styles.content} aria-busy={busy}>
        {error && <p role="alert" className={styles.error}>{error}</p>}
        {busy && !listing && <div className={styles.loading} role="status"><span className="aero-spinner" /> Opening folder…</div>}
        {empty && (
          <div className={styles.empty}>
            <AeroIcon name={folder ? 'folder' : 'picture'} size={48} />
            {folder ? <p>{query.trim() ? 'No matching pictures' : 'No pictures in this folder'}</p> : isDesktop ? <AeroButton variant="accent" onClick={onAdd}><AeroIcon name="openFolder" size={20} /> Add folder</AeroButton> : <p>Open the desktop app</p>}
          </div>
        )}
        {folder && visible.length > 0 && (
          <ul key={`${folderKey(folder)}:${revision}:${currentPage}`} className={styles.grid} data-size={size} onKeyDown={navigateGrid} aria-label="Picture thumbnails">
            {visible.map((photo) => <PhotoTile key={photo.name} folder={folder} photo={photo} selected={photo.name === selectedPhoto?.name} tabbable={photo.name === tabbableName} onSelect={setSelected} />)}
          </ul>
        )}
      </div>
      <AeroToolbar className={styles.status}>
        <span className={styles.count} role="status">{busy ? 'Opening…' : selectedPhoto ? `${selectedPhoto.name} · ${formatBytes(selectedPhoto.bytes)}` : `${photos.length.toLocaleString()} ${photos.length === 1 ? 'picture' : 'pictures'}${listing?.skipped ? ` · ${listing.skipped} unreadable` : ''}`}</span>
        {pageCount > 1 && <div className={styles.pager}>
          <AeroIconButton icon="back" label="Previous page" disabled={currentPage === 0 || busy} onClick={() => changePage(currentPage - 1)} />
          <span>{currentPage + 1} / {pageCount}</span>
          <AeroIconButton icon="forward" label="Next page" disabled={currentPage + 1 >= pageCount || busy} onClick={() => changePage(currentPage + 1)} />
        </div>}
        <AeroRange min={0} max={2} step={1} value={size} disabled={!listing} aria-label="Thumbnail size" aria-valuetext={sizes[size]} title={`Thumbnail size: ${sizes[size]}`} onChange={(event) => onSize(Number(event.currentTarget.value))} />
      </AeroToolbar>
    </main>
  );
}
