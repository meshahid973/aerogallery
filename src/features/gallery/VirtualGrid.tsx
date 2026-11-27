import { memo, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { KeyboardEvent, UIEvent } from 'react';
import { AeroIcon } from '../../components/aero/Icon';
import type { Folder, Photo } from '../../lib/desktop';
import { useThumbnail } from './thumbnails';
import styles from './Gallery.module.css';

const TILE_WIDTHS = [104, 142, 206] as const;
const GAP = 8;
const OVERSCAN = 2;

const PhotoTile = memo(function PhotoTile({ folder, photo, index, total, selected, tabbable, previewRevision, onSelect, onOpen, onContext }: {
  folder: Folder;
  photo: Photo;
  index: number;
  total: number;
  selected: boolean;
  tabbable: boolean;
  previewRevision: number;
  onSelect: (name: string) => void;
  onOpen: (name: string) => void;
  onContext: (name: string, x: number, y: number) => void;
}) {
  const { url, error, retry, markFailed } = useThumbnail(folder, photo.name, photo.stamp, previewRevision);
  return (
    <li className={styles.photoCell} aria-posinset={index + 1} aria-setsize={total}>
      <button type="button" className={styles.photo} data-photo-index={index} aria-pressed={selected} tabIndex={tabbable ? 0 : -1} onClick={() => onSelect(photo.name)} onDoubleClick={() => onOpen(photo.name)} onKeyDown={(event) => { if (error && !event.ctrlKey && !event.altKey && !event.metaKey && event.key.toLowerCase() === 'r') { event.preventDefault(); retry(); } }} onContextMenu={(event) => { event.preventDefault(); onContext(photo.name, event.clientX, event.clientY); }} title={error ? `${photo.name} — ${error} (R to retry)` : photo.name}>
        <span className={styles.imageFrame} data-placeholder={!url}>
          {url ? <img className={styles.thumbnail} src={url} alt="" draggable={false} decoding="async" onError={markFailed} /> : error ? <span className={styles.previewError}>Preview unavailable</span> : <span className={styles.previewPending}><AeroIcon name="picture" size={34} /></span>}
        </span>
        <span className={styles.filename}>{photo.name}</span>
      </button>
    </li>
  );
});

export function VirtualGrid({ folder, photos, selected, size, previewRevision, onSelect, onOpen, onContext, busy }: {
  folder: Folder;
  photos: Photo[];
  selected: string | null;
  size: number;
  previewRevision: number;
  onSelect: (name: string) => void;
  onOpen: (name: string) => void;
  onContext: (name: string, x: number, y: number) => void;
  busy: boolean;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  const pendingFocus = useRef<number | null>(null);
  const frame = useRef<number | null>(null);
  const [metrics, setMetrics] = useState({ width: 0, height: 0, top: 0 });
  const preferred = TILE_WIDTHS[size] ?? TILE_WIDTHS[1];
  const columns = Math.max(1, Math.floor((metrics.width + GAP) / (preferred + GAP)));
  const tile = metrics.width > 0 ? Math.max(1, Math.floor((metrics.width - GAP * (columns - 1)) / columns)) : preferred;
  const rowHeight = Math.ceil(Math.max(0, tile - 12) * 0.75) + 33;
  const stride = rowHeight + GAP;
  const rows = Math.ceil(photos.length / columns);
  const start = Math.min(Math.max(0, rows - 1), Math.max(0, Math.floor(metrics.top / stride) - OVERSCAN));
  const end = Math.min(rows, Math.ceil((metrics.top + metrics.height) / stride) + OVERSCAN);
  const first = start * columns;
  const last = Math.min(photos.length, Math.max(start + 1, end) * columns);
  const selectedIndex = useMemo(() => selected === null ? -1 : photos.findIndex((photo) => photo.name === selected), [photos, selected]);
  const tabIndex = selectedIndex >= first && selectedIndex < last ? selectedIndex : first;

  useLayoutEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const measure = () => {
      const css = getComputedStyle(element);
      const width = element.clientWidth - parseFloat(css.paddingLeft) - parseFloat(css.paddingRight);
      setMetrics((previous) => {
        const next = { width: Math.max(0, width), height: element.clientHeight, top: element.scrollTop };
        return previous.width === next.width && previous.height === next.height && previous.top === next.top ? previous : next;
      });
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => {
      observer.disconnect();
      if (frame.current !== null) cancelAnimationFrame(frame.current);
      frame.current = null;
    };
  }, []);

  useLayoutEffect(() => {
    const index = pendingFocus.current;
    if (index === null) return;
    const button = viewport.current?.querySelector<HTMLButtonElement>(`[data-photo-index="${index}"]`);
    if (button) {
      pendingFocus.current = null;
      button.focus({ preventScroll: true });
    }
  }, [first, last, selected]);

  useLayoutEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const top = element.scrollTop;
    setMetrics((previous) => previous.top === top ? previous : { ...previous, top });
  }, [rows, stride]);

  function onScroll(event: UIEvent<HTMLDivElement>) {
    const element = event.currentTarget;
    if (frame.current !== null) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      const top = element.scrollTop;
      setMetrics((previous) => previous.top === top ? previous : { ...previous, top });
    });
  }

  function navigate(event: KeyboardEvent<HTMLUListElement>) {
    const target = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-photo-index]');
    const current = Number(target?.dataset.photoIndex);
    if (!target || !Number.isInteger(current)) return;
    if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) { const photo = photos[current]; if (photo) { event.preventDefault(); const rect = target.getBoundingClientRect(); onContext(photo.name, rect.left, rect.bottom); } return; }
    if (event.key === 'Enter') { const photo = photos[current]; if (photo) { event.preventDefault(); onOpen(photo.name); } return; }
    const steps: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns, PageUp: -Math.max(1, Math.floor(metrics.height / stride)) * columns, PageDown: Math.max(1, Math.floor(metrics.height / stride)) * columns };
    if (!Object.hasOwn(steps, event.key) && event.key !== 'Home' && event.key !== 'End') return;
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? photos.length - 1 : Math.max(0, Math.min(photos.length - 1, current + (steps[event.key] ?? 0)));
    const photo = photos[next];
    const element = viewport.current;
    if (!photo || !element) return;
    event.preventDefault();
    const y = Math.floor(next / columns) * stride;
    if (y < element.scrollTop) element.scrollTop = y;
    else if (y + rowHeight > element.scrollTop + metrics.height) element.scrollTop = y + rowHeight - metrics.height;
    pendingFocus.current = next;
    const button = element.querySelector<HTMLButtonElement>(`[data-photo-index="${next}"]`);
    if (button) {
      pendingFocus.current = null;
      button.focus({ preventScroll: true });
    }
    setMetrics((previous) => ({ ...previous, top: element.scrollTop }));
    onSelect(photo.name);
  }

  return (
    <div ref={viewport} className={styles.content} onScroll={onScroll} aria-busy={busy}>
      <ul className={styles.virtualGrid} style={{
        gridTemplateColumns: `repeat(${columns}, ${tile}px)`,
        gridAutoRows: `${rowHeight}px`,
        paddingTop: start * stride,
        paddingBottom: Math.max(0, rows - Math.max(start + 1, end)) * stride,
      }} onKeyDown={navigate} aria-label="Picture thumbnails">
        {photos.slice(first, last).map((photo, offset) => <PhotoTile key={`${photo.name}:${photo.stamp}`} folder={folder} photo={photo} index={first + offset} total={photos.length} selected={photo.name === selected} tabbable={first + offset === tabIndex} previewRevision={previewRevision} onSelect={onSelect} onOpen={onOpen} onContext={onContext} />)}
      </ul>
    </div>
  );
}
