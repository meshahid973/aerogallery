import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import { AeroButton, AeroSelect } from './controls';
import { copyImage, isWindows, setWallpaper } from '../../lib/desktop';
import type { Folder, Photo } from '../../lib/desktop';

function positionMenu(panel: HTMLDivElement, x: number, y: number) {
  const bounds = panel.getBoundingClientRect();
  panel.style.left = `${Math.max(4, Math.min(x, window.innerWidth - bounds.width - 4))}px`;
  panel.style.top = `${Math.max(4, Math.min(y, window.innerHeight - bounds.height - 4))}px`;
}

export function showPhotoActions(panel: HTMLDivElement | null, x: number, y: number) {
  if (!panel) return;
  if (!panel.matches(':popover-open')) panel.showPopover();
  positionMenu(panel, x, y);
  panel.querySelector<HTMLElement>('button:not(:disabled)')?.focus({ preventScroll: true });
}

export function PhotoActions({ panelRef, folder, photo, onOpen }: {
  panelRef: RefObject<HTMLDivElement | null>;
  folder: Folder | null;
  photo?: Photo;
  onOpen?: () => void;
}) {
  const [position, setPosition] = useState('fill');
  const [wallpaper, setWallpaperOptions] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const pending = useRef(false);
  const identity = photo ? `${folder?.rootId}:${folder?.parts.join('/')}:${photo.name}:${photo.stamp}` : '';
  const current = useRef(identity);
  current.current = identity;
  useEffect(() => { setWallpaperOptions(false); setMessage(''); }, [identity]);
  useLayoutEffect(() => {
    const panel = panelRef.current;
    if (!panel?.matches(':popover-open')) return;
    const bounds = panel.getBoundingClientRect();
    positionMenu(panel, bounds.left, bounds.top);
  }, [panelRef, wallpaper, message]);
  async function action(kind: 'copy' | 'wallpaper') {
    if (!folder || !photo || pending.current) return;
    pending.current = true;
    setBusy(true);
    setMessage('');
    try {
      if (kind === 'copy') await copyImage(folder, photo);
      else await setWallpaper(folder, photo, position);
      if (current.current === identity && panelRef.current?.matches(':popover-open')) setMessage(kind === 'copy' ? 'Image copied.' : 'Wallpaper set.');
    } catch (error) {
      if (current.current === identity && panelRef.current?.matches(':popover-open')) setMessage(typeof error === 'string' ? error : 'Picture action failed.');
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }
  return <div ref={panelRef} popover="auto" role="group" className="aero-photo-actions" aria-label="Picture actions" onToggle={(event) => {
    if (event.newState === 'closed' && !event.currentTarget.matches(':popover-open')) { setWallpaperOptions(false); setMessage(''); }
  }}>
    {isWindows && <AeroButton variant="toolbar" disabled={!photo || busy} onClick={() => void action('copy')}>Copy image</AeroButton>}
    {onOpen && <AeroButton variant="toolbar" disabled={!photo} onClick={() => { panelRef.current?.hidePopover(); onOpen(); }}>View picture</AeroButton>}
    {isWindows && <><AeroButton variant="toolbar" disabled={!photo || busy} aria-expanded={wallpaper} onClick={() => {
      setWallpaperOptions(!wallpaper);
      setMessage('');
    }}>Set as wallpaper</AeroButton>
      {wallpaper && <div className="aero-wallpaper-options">
        <AeroSelect aria-label="Wallpaper position" value={position} onChange={(event) => setPosition(event.currentTarget.value)}>
          <option value="fill">Fill</option><option value="fit">Fit</option><option value="stretch">Stretch</option><option value="center">Center</option><option value="tile">Tile</option><option value="span">Span</option>
        </AeroSelect>
        <AeroButton disabled={!photo || busy} onClick={() => void action('wallpaper')}>Apply</AeroButton>
      </div>}</>}
    {message && <p role="status">{message}</p>}
  </div>;
}
