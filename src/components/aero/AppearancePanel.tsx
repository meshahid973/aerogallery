import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { defaults, readAppearance, saveAppearance, themes, themeNames, tintChannels, tintHue, tintFromHue } from '../../lib/appearance';
import type { Appearance } from '../../lib/appearance';
import { isWindows } from '../../lib/desktop';
import { AeroButton, AeroRange } from './controls';
import styles from '../../App.module.css';

let materialQueue: Promise<unknown> = Promise.resolve();
export function AppearancePanel({ shell }: { shell: RefObject<HTMLDivElement | null> }) {
  const [value, setValue] = useState(readAppearance);
  const [error, setError] = useState('');
  const [revision, refresh] = useState(0);
  const [focused, setFocused] = useState(true);
  const [red, green, blue] = tintChannels(value);
  const transparent = value.opacity < 100;
  const hue = tintHue(value);
  const latest = useRef(value);
  const dirty = useRef(false);
  const saveTimer = useRef<number | undefined>(undefined);
  function flush() {
    window.clearTimeout(saveTimer.current);
    if (!dirty.current) return;
    dirty.current = false;
    if (!saveAppearance(latest.current)) {
      dirty.current = true;
      setError('Appearance could not be saved.');
    } else setError('');
  }
  function change(next: Appearance) {
    latest.current = next;
    dirty.current = true;
    setValue(next);
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(flush, 160);
  }
  useEffect(() => {
    const save = () => { if (dirty.current) saveAppearance(latest.current); };
    window.addEventListener('pagehide', save);
    return () => { window.clearTimeout(saveTimer.current); save(); window.removeEventListener('pagehide', save); };
  }, []);
  useEffect(() => {
    const node = shell.current;
    if (!node) return;
    node.dataset.theme = value.theme;
    node.dataset.windowFocused = String(focused);
    // Keep title-bar text legible even if a very dark custom tint is selected.
    const channels = [red, green, blue];
    const readable = (value.tint ? channels.map((channel) => Math.round(120 + channel * (135 / 255))) : channels).join(', ');
    node.style.setProperty('--appearance-tint', readable);
    node.style.setProperty('--appearance-opacity', String(value.opacity / 100));
    node.style.setProperty('--reflection-strength', String(value.reflection / 65));
  }, [shell, value, focused, red, green, blue]);
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
    const permitted = isWindows && !matchMedia('(prefers-reduced-transparency: reduce)').matches && !matchMedia('(forced-colors: active)').matches && transparent;
    if (!permitted) node.dataset.nativeGlass = 'false';
    if (!isWindows) return;
    const timer = window.setTimeout(() => {
      materialQueue = materialQueue.catch(() => {}).then(async () => {
        if (!active) return;
        const enabled = await invoke<boolean>('apply_material', { tint: [red, green, blue], opacity: transparent ? 99 : 100, permitted }).catch(() => false);
        if (active) node.dataset.nativeGlass = String(enabled);
      });
    }, 100);
    return () => { active = false; window.clearTimeout(timer); };
  }, [shell, red, green, blue, transparent, revision]);
  return <div id="appearance-panel" popover="auto" className={styles.appearance} aria-label="Appearance">
    <div className={styles.themeChoices} role="group" aria-label="Theme">
      {themes.map((choice) => <button type="button" className={styles.themeChoice} key={choice} aria-pressed={value.theme === choice} onClick={() => change({ ...value, theme: choice, tint: null })}>
        <span className={styles.themeSample} data-theme={choice} aria-hidden="true" />{themeNames[choice]}
      </button>)}
    </div>
    <div className={styles.appearanceFields} onPointerUp={flush} onBlur={flush}>
      <label htmlFor="glass-tint">Tint</label><AeroRange id="glass-tint" className={styles.tintRange} min={0} max={359} value={hue} aria-valuetext={`${hue} degrees`} onChange={(event) => change({ ...value, tint: tintFromHue(Number(event.currentTarget.value)) })} /><span />
      <label htmlFor="glass-opacity">Opacity</label><AeroRange id="glass-opacity" min={35} max={100} value={value.opacity} onChange={(event) => change({ ...value, opacity: Number(event.currentTarget.value) })} /><output htmlFor="glass-opacity">{value.opacity}%</output>
      <label htmlFor="glass-reflection">Reflection</label><AeroRange id="glass-reflection" min={0} max={100} value={value.reflection} onChange={(event) => change({ ...value, reflection: Number(event.currentTarget.value) })} /><output htmlFor="glass-reflection">{value.reflection}%</output>
    </div>
    <AeroButton variant="toolbar" className={styles.resetAppearance} onClick={() => { change({ ...defaults }); flush(); }}>Reset</AeroButton>
    {error && <p role="status" className={styles.preferenceError}>{error}</p>}
  </div>;
}
