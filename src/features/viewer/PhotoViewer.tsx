import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { AeroButton, AeroIconButton, AeroSelect, AeroToolbar } from '../../components/aero/controls';
import { CaptionBar } from '../../components/aero/CaptionBar';
import { AeroIcon } from '../../components/aero/Icon';
import { suspendThumbnailQueue } from '../gallery/thumbnails';
import { setViewerFullscreen } from '../../lib/desktop';
import type { Folder, Photo } from '../../lib/desktop';
import { photoKey, useViewerAsset } from './assets';
import { PhotoStage } from './PhotoStage';
import type { StageControls } from './PhotoStage';
import styles from './Viewer.module.css';

let fullscreenTail: Promise<unknown> = Promise.resolve();
function changeFullscreen(enabled: boolean) {
  const task = fullscreenTail.then(() => setViewerFullscreen(enabled));
  fullscreenTail = task.catch(() => {});
  return task;
}

export default function PhotoViewer({ folder, photos, name, onNavigate, onClose }: {
  folder: Folder;
  photos: Photo[];
  name: string;
  onNavigate: (name: string) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const stage = useRef<StageControls | null>(null);
  const [retry, setRetry] = useState(0);
  const [loaded, setLoaded] = useState('');
  const [failed, setFailed] = useState('');
  const [zoom, setZoom] = useState(100);
  const [playing, setPlaying] = useState(false);
  const [interval, setInterval] = useState(5);
  const [fullscreen, setFullscreen] = useState(false);
  const [windowError, setWindowError] = useState('');
  const [windowBusy, setWindowBusy] = useState(false);
  const ownsFullscreen = useRef(false);
  const windowPending = useRef(false);
  const mounted = useRef(false);
  const index = photos.findIndex((photo) => photo.name === name);
  const photo = photos[index];
  const next = photos[index + 1] ?? (photos.length > 1 ? photos[0] : undefined);
  const previous = photos[index - 1];
  const result = useViewerAsset(folder, photo, next, previous, retry);
  const asset = result.asset;
  const key = photoKey(photo);
  const error = !photo ? 'Picture is no longer in this folder.' : result.error || (asset && failed === asset.url ? 'Cannot display picture.' : '');
  const ready = !!asset && loaded === asset.url && !error;
  const onReady = useCallback(() => { if (asset) setLoaded(asset.url); }, [asset]);
  const onError = useCallback(() => { if (asset) setFailed(asset.url); }, [asset]);

  useLayoutEffect(() => {
    mounted.current = true;
    const resumeThumbnails = suspendThumbnailQueue();
    const element = dialog.current;
    element?.showModal();
    element?.focus();
    return () => {
      mounted.current = false;
      element?.close();
      resumeThumbnails();
      if (ownsFullscreen.current) {
        ownsFullscreen.current = false;
        void changeFullscreen(false).catch(() => {});
      }
    };
  }, []);

  useLayoutEffect(() => {
    setLoaded('');
    setFailed('');
    setZoom(100);
  }, [key, asset?.url]);

  useEffect(() => {
    if (error || photos.length < 2) setPlaying(false);
  }, [error, photos.length]);

  useEffect(() => {
    const pause = () => { if (document.hidden) setPlaying(false); };
    document.addEventListener('visibilitychange', pause);
    return () => document.removeEventListener('visibilitychange', pause);
  }, []);

  useEffect(() => {
    if (!playing || !ready || error || photos.length < 2 || !next) return;
    const timer = window.setTimeout(() => onNavigate(next.name), interval * 1000);
    return () => window.clearTimeout(timer);
  }, [playing, ready, error, key, next?.name, photos.length, interval, onNavigate]);

  function move(delta: number) {
    const target = photos[index + delta];
    if (!target) return;
    setPlaying(false);
    onNavigate(target.name);
  }

  async function toggleFullscreen() {
    if (windowPending.current) return;
    windowPending.current = true;
    const enabled = !fullscreen;
    setWindowBusy(true);
    setWindowError('');
    if (enabled) ownsFullscreen.current = true;
    try {
      await changeFullscreen(enabled);
      if (!mounted.current) return;
      ownsFullscreen.current = enabled;
      setFullscreen(enabled);
    } catch {
      if (mounted.current) {
        if (enabled) ownsFullscreen.current = false;
        setWindowError('Fullscreen could not be changed.');
      }
    } finally {
      windowPending.current = false;
      if (mounted.current) setWindowBusy(false);
    }
  }

  function escape() {
    if (fullscreen || windowBusy) {
      if (!windowBusy) void toggleFullscreen();
    } else onClose();
  }

  function togglePlay() {
    if (photos.length > 1 && !error) setPlaying((value) => !value);
  }

  function keyboard(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === 'Escape') { event.preventDefault(); escape(); return; }
    const element = event.target as HTMLElement;
    if (element.closest('select,input,textarea,[contenteditable=true]')) return;
    if (event.key === ' ' && element.closest('button')) return;
    const commands: Record<string, () => void> = {
      ArrowLeft: () => event.shiftKey ? stage.current?.pan(80, 0) : move(-1), ArrowRight: () => event.shiftKey ? stage.current?.pan(-80, 0) : move(1),
      ArrowUp: () => stage.current?.pan(0, 80), ArrowDown: () => stage.current?.pan(0, -80),
      '+': () => stage.current?.zoom(1.25), '=': () => stage.current?.zoom(1.25), '-': () => stage.current?.zoom(0.8),
      '0': () => stage.current?.fit(), '1': () => stage.current?.actual(),
      r: () => stage.current?.rotate(event.shiftKey ? -90 : 90),
      f: () => void toggleFullscreen(), F11: () => void toggleFullscreen(), ' ': togglePlay,
    };
    const command = commands[event.key] ?? commands[event.key.toLowerCase()];
    if (command) { event.preventDefault(); if (!event.repeat || ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', '+', '=', '-'].includes(event.key)) command(); }
  }

  return (
    <dialog ref={dialog} className={styles.viewer} tabIndex={-1} aria-label="Photo viewer" data-fullscreen={fullscreen} onKeyDown={keyboard}
      onCancel={(event) => { event.preventDefault(); escape(); }}>
      <CaptionBar fullscreen={fullscreen} />
      <AeroToolbar className={styles.heading} data-tone="glass">
        <AeroButton variant="toolbar" onClick={onClose}><AeroIcon name="back" size={18} /> Gallery</AeroButton>
        <span className={styles.name} title={name}>{name}</span>
        <span className={styles.counter}>{index >= 0 ? index + 1 : '—'} / {photos.length}</span>
        <AeroIconButton variant="toolbar" icon="fullscreen" label={fullscreen ? 'Exit fullscreen (F)' : 'Fullscreen (F)'} aria-pressed={fullscreen} disabled={windowBusy} onClick={() => void toggleFullscreen()} />
      </AeroToolbar>
      <div className={styles.surface} aria-busy={!ready && !error}>
        {asset && !error && <PhotoStage key={asset.url} url={asset.url} name={name} onReady={onReady} onError={onError} controls={stage} onZoom={setZoom} />}
        {!ready && !error && <div className={styles.message} role="status"><span className="aero-spinner" aria-hidden="true" /> Opening picture…</div>}
        {error && <div className={styles.message} role="alert"><span>{error}</span>{photo && <AeroButton onClick={() => { setLoaded(''); setFailed(''); setRetry((value) => value + 1); }}>Retry</AeroButton>}</div>}
        {windowError && <div className={styles.notice} role="alert">{windowError}</div>}
      </div>
      <AeroToolbar className={styles.controls} aria-label="Photo controls">
        <div className={styles.group}>
          <AeroButton className={styles.glyph} aria-label="Zoom out (-)" title="Zoom out (-)" disabled={!ready} onClick={() => stage.current?.zoom(0.8)}>−</AeroButton>
          <AeroButton className={styles.glyph} aria-label="Zoom in (+)" title="Zoom in (+)" disabled={!ready} onClick={() => stage.current?.zoom(1.25)}>+</AeroButton>
          <AeroButton disabled={!ready} title="Fit to window (0)" onClick={() => stage.current?.fit()}>Fit</AeroButton>
          <AeroButton disabled={!ready} title="Actual size (1)" onClick={() => stage.current?.actual()}>1:1</AeroButton>
          <span className={styles.zoom} aria-live="off">{ready ? `${zoom}%` : '—'}</span>
        </div>
        <div className={styles.playback}>
          <AeroIconButton icon="back" label="Previous picture (Left)" disabled={index <= 0} onClick={() => move(-1)} />
          <AeroButton className={styles.play} aria-label={playing ? 'Pause slideshow (Space)' : 'Play slideshow (Space)'} title={playing ? 'Pause slideshow (Space)' : 'Play slideshow (Space)'} aria-pressed={playing} disabled={photos.length < 2 || !!error} onClick={togglePlay}>
            <AeroIcon name={playing ? 'pause' : 'play'} size={24} />
          </AeroButton>
          <AeroIconButton icon="forward" label="Next picture (Right)" disabled={index < 0 || index >= photos.length - 1} onClick={() => move(1)} />
        </div>
        <div className={styles.group}>
          <AeroButton className={styles.glyph} title="Rotate left (Shift+R)" aria-label="Rotate left" disabled={!ready} onClick={() => stage.current?.rotate(-90)}><AeroIcon name="rotateLeft" size={18} /></AeroButton>
          <AeroButton className={styles.glyph} title="Rotate right (R)" aria-label="Rotate right" disabled={!ready} onClick={() => stage.current?.rotate(90)}><AeroIcon name="rotateRight" size={18} /></AeroButton>
          <AeroSelect aria-label="Slideshow interval" title="Slideshow interval" value={interval} onChange={(event) => setInterval(Number(event.currentTarget.value))}>
            <option value={3}>3 s</option><option value={5}>5 s</option><option value={10}>10 s</option>
          </AeroSelect>
        </div>
      </AeroToolbar>
    </dialog>
  );
}
