import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { scheduleChromeRegions } from '../../lib/chrome';
import { isDesktop, isWindows, showSystemMenu } from '../../lib/desktop';
import { AeroIcon } from './Icon';
import styles from './CaptionBar.module.css';

export const customChrome = !isDesktop || /Windows/i.test(navigator.userAgent);
type Direction = Parameters<ReturnType<typeof getCurrentWindow>['startResizeDragging']>[0];
const edges: Direction[] = ['North', 'South', 'East', 'West', 'NorthEast', 'NorthWest', 'SouthEast', 'SouthWest'];

export function CaptionBar({ title = 'AeroGallery', fullscreen = false, leading, trailing }: { title?: string; fullscreen?: boolean; leading?: ReactNode; trailing?: ReactNode }) {
  const [maximized, setMaximized] = useState(false);
  const [focused, setFocused] = useState(true);
  const [error, setError] = useState('');
  const pending = useRef(false);

  useLayoutEffect(() => { scheduleChromeRegions(); return scheduleChromeRegions; }, [fullscreen]);

  useEffect(() => {
    if (!isDesktop || !customChrome || fullscreen) return;
    let active = true;
    let timer: number | undefined;
    const releases: (() => void)[] = [];
    const appWindow = getCurrentWindow();
    const sync = () => {
      void appWindow.isMaximized().then((value) => { if (active) setMaximized(value); }, () => {});
    };
    const attach = (subscription: Promise<() => void>) => {
      void subscription.then((release) => { if (active) releases.push(release); else release(); }, () => {});
    };
    sync();
    void appWindow.isFocused().then((value) => { if (active) setFocused(value); }, () => {});
    attach(appWindow.onResized(() => {
      window.clearTimeout(timer);
      timer = window.setTimeout(sync, 100);
    }));
    attach(appWindow.onFocusChanged(({ payload }) => { if (active) setFocused(payload); }));
    return () => {
      active = false;
      window.clearTimeout(timer);
      for (const release of releases) release();
    };
  }, [fullscreen]);

  async function action(kind: 'minimize' | 'maximize' | 'close') {
    if (!isDesktop || pending.current) return;
    pending.current = true;
    setError('');
    try {
      const window = getCurrentWindow();
      if (kind === 'minimize') await window.minimize();
      else if (kind === 'close') await window.close();
      else { await window.toggleMaximize(); setMaximized(await window.isMaximized()); }
    } catch {
      setError('Window action unavailable.');
    } finally {
      pending.current = false;
    }
  }

  if (!customChrome || fullscreen) return null;
  return (
    <div className={styles.caption} data-focused={focused} data-maximized={maximized}>
      {leading && <div className={styles.tools}>{leading}</div>}
      <div className={styles.drag} data-tauri-drag-region onContextMenu={(event) => { if (isWindows) { event.preventDefault(); void showSystemMenu(true).catch(() => {}); } }}>
        {!leading && <AeroIcon name="picture" size={18} />}
        <span title={title}>{title}</span>
        {error && <span role="alert" className={styles.error}>{error}</span>}
      </div>
      {trailing && <div className={styles.tools}>{trailing}</div>}
      <div className={styles.buttons} role="group" aria-label="Window controls">
        <button type="button" title="Minimize" aria-label="Minimize window" disabled={!isDesktop} onClick={() => void action('minimize')}>
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="M3 11.5h10" /></svg>
        </button>
        <button type="button" data-max-button title={maximized ? 'Restore' : 'Maximize'} aria-label={maximized ? 'Restore window' : 'Maximize window'} disabled={!isDesktop} onClick={() => void action('maximize')}>
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">{maximized ? <path d="M6 5V2.5h7.5V10H11M2.5 5.5H11V13H2.5Z" /> : <path d="M3 3.5h10v9H3zM3 5.5h10" />}</svg>
        </button>
        <button type="button" className={styles.close} title="Close" aria-label="Close window" disabled={!isDesktop} onClick={() => void action('close')}>
          <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path d="m4 4 8 8m0-8-8 8" /></svg>
        </button>
      </div>
      {isDesktop && !maximized && edges.map((edge) => <span key={edge} className={styles.resize} data-edge={edge} aria-hidden="true" onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        void getCurrentWindow().startResizeDragging(edge).catch(() => {});
      }} />)}
    </div>
  );
}
