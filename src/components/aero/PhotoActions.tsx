import { useState } from 'react';
import type { RefObject } from 'react';
import { AeroButton, AeroSelect } from './controls';
import { isWindows, setWallpaper } from '../../lib/desktop';
import type { Folder, Photo } from '../../lib/desktop';

export function PhotoActions({ panelRef, folder, photo, onOpen }: {
  panelRef: RefObject<HTMLDivElement | null>;
  folder: Folder | null;
  photo?: Photo;
  onOpen?: () => void;
}) {
  const [position, setPosition] = useState('fill');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  return <div ref={panelRef} popover="auto" className="aero-photo-actions" aria-label="Picture actions">
    {onOpen && <AeroButton disabled={!photo} onClick={() => { panelRef.current?.hidePopover(); onOpen(); }}>View picture</AeroButton>}
    {isWindows && <><label>Wallpaper position
      <AeroSelect value={position} onChange={(event) => setPosition(event.currentTarget.value)}>
        <option value="fill">Fill</option><option value="fit">Fit</option><option value="stretch">Stretch</option><option value="center">Center</option><option value="tile">Tile</option><option value="span">Span</option>
      </AeroSelect></label>
      <AeroButton disabled={!photo || !folder || busy} onClick={() => {
        if (!photo || !folder) return;
        setBusy(true); setMessage('');
        void setWallpaper(folder, photo, position).then(() => { setMessage('Wallpaper set.'); }, (error) => setMessage(String(error))).finally(() => setBusy(false));
      }}>Set as desktop wallpaper</AeroButton></>}
    {message && <p role="status">{message}</p>}
  </div>;
}
