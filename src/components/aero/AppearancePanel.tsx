import { useEffect, useState } from 'react';
import type { RefObject } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { defaults, readAppearance, saveAppearance, themes, themeNames, tintOf, tintChannels } from '../../lib/appearance';
import type { Appearance } from '../../lib/appearance';
import { defaultApps, isWindows } from '../../lib/desktop';
import { AeroButton, AeroInput, AeroRange } from './controls';
import styles from '../../App.module.css';

let materialQueue: Promise<unknown> = Promise.resolve();
export function AppearancePanel({ shell }: { shell: RefObject<HTMLDivElement | null> }) {
  const [value, setValue] = useState(readAppearance);
  const [error, setError] = useState('');
  const [revision, refresh] = useState(0);
  const [focused, setFocused] = useState(true);
  function change(next: Appearance) {
    setValue(next);
    setError(saveAppearance(next) ? '' : 'Appearance could not be saved.');
  }
  useEffect(() => {
    const node = shell.current;
    if (!node) return;
    node.dataset.theme = value.theme;
    node.dataset.windowFocused = String(focused);
    // Keep title-bar text legible even if a very dark custom tint is selected.
    const readable = value.tint ? tintChannels(value).map((channel) => Math.round(120 + channel * (135 / 255))).join(', ') : tintChannels(value).join(', ');
    node.style.setProperty('--appearance-tint', readable);
    node.style.setProperty('--appearance-opacity', String(value.opacity / 100));
    node.style.setProperty('--reflection-strength', String(value.reflection / 65));
  }, [shell, value, focused]);
  useEffect(() => {
    const queries = [matchMedia('(prefers-reduced-transparency: reduce)'), matchMedia('(forced-colors: active)')];
    const changed = () => refresh((n) => n + 1);
    queries.forEach((query) => query.addEventListener('change', changed));
    let active = true;
    const releases: Array<() => void> = [];
    if (isWindows) {
      void getCurrentWindow().isFocused().then((state) => { if (active) setFocused(state); }).catch(() => {});
      for (const pending of [listen('aero-material-settings', changed), getCurrentWindow().onFocusChanged(({ payload }) => { if (active) setFocused(payload); })]) {
        void pending.then((release) => { if (active) releases.push(release); else release(); }).catch(() => {});
      }
    }
    return () => { active = false; queries.forEach((query) => query.removeEventListener('change', changed)); releases.forEach((release) => release()); };
  }, []);
  useEffect(() => {
    const node = shell.current;
    if (!node) return;
    let active = true;
    const permitted = isWindows && !matchMedia('(prefers-reduced-transparency: reduce)').matches && !matchMedia('(forced-colors: active)').matches && value.opacity < 100;
    if (!permitted) node.dataset.nativeGlass = 'false';
    if (!isWindows) return;
    const timer = window.setTimeout(() => {
      materialQueue = materialQueue.catch(() => {}).then(async () => {
        if (!active) return;
        const enabled = await invoke<boolean>('apply_material', { tint: tintChannels(value), opacity: value.opacity, permitted }).catch(() => false);
        if (active) node.dataset.nativeGlass = String(enabled);
      });
    }, 100);
    return () => { active = false; window.clearTimeout(timer); };
  }, [shell, value.theme, value.tint, value.opacity, revision]);
  return <div id="appearance-panel" popover="auto" className={styles.appearance} aria-label="Appearance">
    <div className={styles.themeChoices} role="group" aria-label="Theme">
      {themes.map((choice) => <button type="button" className={styles.themeChoice} key={choice} aria-pressed={value.theme === choice} onClick={() => change({ ...value, theme: choice, tint: null })}>
        <span className={styles.themeSample} data-theme={choice} aria-hidden="true"><span /><i /></span>{themeNames[choice]}
      </button>)}
    </div>
    <div className={styles.liveGlass} aria-hidden="true"><span className="aero-caption-material">AeroGallery</span><span className="aero-toolbar" data-tone="glass">Pictures</span></div>
    <div className={styles.appearanceFields}>
      <label htmlFor="glass-tint">Tint</label><AeroInput id="glass-tint" type="color" value={tintOf(value)} onChange={(event) => change({ ...value, tint: event.currentTarget.value })} /><span />
      <label htmlFor="glass-opacity">Opacity</label><AeroRange id="glass-opacity" min={35} max={100} value={value.opacity} onChange={(event) => change({ ...value, opacity: Number(event.currentTarget.value) })} /><output htmlFor="glass-opacity">{value.opacity}%</output>
      <label htmlFor="glass-reflection">Reflection</label><AeroRange id="glass-reflection" min={0} max={100} value={value.reflection} onChange={(event) => change({ ...value, reflection: Number(event.currentTarget.value) })} /><output htmlFor="glass-reflection">{value.reflection}%</output>
    </div>
    <AeroButton onClick={() => change({ ...defaults })}>Reset</AeroButton>
    {isWindows && <AeroButton onClick={() => void defaultApps().catch((failure) => setError(String(failure)))}>Windows Default Apps</AeroButton>}
    {error && <p role="status" className={styles.preferenceError}>{error}</p>}
  </div>;
}
