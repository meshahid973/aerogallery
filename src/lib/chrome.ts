import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isWindows, setChromeRegions, showSystemMenu } from './desktop';

let frame: number | null = null;
let previous = '';
export function scheduleChromeRegions() {
  if (!isWindows || frame !== null) return;
  frame = requestAnimationFrame(() => {
    frame = null;
    const modal = document.querySelector('dialog[open]');
    const button = (modal ?? document).querySelector<HTMLElement>('[data-max-button]');
    const rect = button?.getBoundingClientRect();
    const ratio = window.devicePixelRatio;
    const bounds = rect && button && getComputedStyle(button).visibility === 'visible' && rect.width > 0 ?
      [Math.round(rect.left * ratio), Math.round(rect.top * ratio), Math.round(rect.right * ratio), Math.round(rect.bottom * ratio)] : null;
    const key = JSON.stringify(bounds);
    if (key === previous) return;
    previous = key;
    void setChromeRegions(bounds).catch(() => { if (previous === key) previous = ''; });
  });
}

export function useNativeChrome() {
  useEffect(() => {
    if (!isWindows) return;
    let active = true;
    const releases: (() => void)[] = [];
    const attach = (promise: Promise<() => void>) => {
      void promise.then((release) => { if (active) releases.push(release); else release(); }, () => {});
    };
    attach(listen<boolean>('aero-max-hover', ({ payload }) => {
      document.documentElement.dataset.nativeMaxHover = String(payload);
    }));
    attach(getCurrentWindow().onScaleChanged(() => { previous = ''; scheduleChromeRegions(); }));
    const onResize = () => scheduleChromeRegions();
    const keyboard = (event: KeyboardEvent) => {
      if (event.altKey && event.code === 'Space') { event.preventDefault(); void showSystemMenu(false).catch(() => {}); }
    };
    window.addEventListener('resize', onResize);
    document.addEventListener('keydown', keyboard, true);
    previous = '';
    scheduleChromeRegions();
    return () => {
      active = false;
      window.removeEventListener('resize', onResize);
      document.removeEventListener('keydown', keyboard, true);
      for (const release of releases) release();
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      previous = '';
      delete document.documentElement.dataset.nativeMaxHover;
    };
  }, []);
}
