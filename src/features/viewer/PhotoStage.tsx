import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent } from 'react';
import { constrain, initialView, scaleOf, zoomAt } from './geometry';
import type { Dimensions, View } from './geometry';
import styles from './Viewer.module.css';

export interface StageControls {
  fit: () => void;
  actual: () => void;
  zoom: (factor: number) => void;
  rotate: (delta: number) => void;
  pan: (x: number, y: number) => void;
}

export function PhotoStage({ url, name, onReady, onError, controls, onZoom }: {
  url: string;
  name: string;
  onReady: () => void;
  onError: () => void;
  controls: { current: StageControls | null };
  onZoom: (zoom: number) => void;
}) {
  const stage = useRef<HTMLDivElement>(null);
  const [image, setImage] = useState<Dimensions>({ width: 0, height: 0 });
  const [viewport, setViewport] = useState<Dimensions>({ width: 0, height: 0 });
  const [view, setView] = useState<View>(initialView);
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const frame = useRef<number | null>(null);
  const pending = useRef<{ dx: number; dy: number; factor: number; x: number; y: number }>({ dx: 0, dy: 0, factor: 1, x: 0, y: 0 });
  const scale = scaleOf(view, image, viewport);
  const ready = image.width > 0 && viewport.width > 0;
  const shown = constrain(view, image, viewport);

  useLayoutEffect(() => {
    const element = stage.current;
    if (!element) return;
    const measure = () => {
      const next = { width: Math.max(1, element.clientWidth), height: Math.max(1, element.clientHeight) };
      setViewport((old) => old.width === next.width && old.height === next.height ? old : next);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const change = useCallback((update: (old: View) => View) => {
    if (image.width === 0 || viewport.width === 0) return;
    setView((old) => constrain(update(constrain(old, image, viewport)), image, viewport));
  }, [image, viewport]);

  useLayoutEffect(() => {
    controls.current = {
      fit: () => change((old) => ({ ...old, zoom: null, x: 0, y: 0 })),
      actual: () => change((old) => zoomAt(old, image, viewport, 1)),
      zoom: (factor) => change((old) => zoomAt(old, image, viewport, scaleOf(old, image, viewport) * factor)),
      rotate: (delta) => change((old) => ({ ...old, rotation: (old.rotation + delta + 360) % 360, x: 0, y: 0 })),
      pan: (x, y) => change((old) => ({ ...old, x: old.x + x, y: old.y + y })),
    };
    return () => { controls.current = null; };
  }, [change, controls, image, viewport]);

  useEffect(() => {
    if (ready) onZoom(Math.round(scale * 100));
  }, [ready, scale, onZoom]);

  useEffect(() => {
    const element = stage.current;
    if (!element) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      const bounds = element.getBoundingClientRect();
      const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 300 : 1);
      const factor = Math.exp(-Math.max(-600, Math.min(600, delta)) * 0.002);
      change((old) => zoomAt(old, image, viewport, scaleOf(old, image, viewport) * factor, event.clientX - bounds.left - bounds.width / 2, event.clientY - bounds.top - bounds.height / 2));
    };
    element.addEventListener('wheel', wheel, { passive: false });
    return () => element.removeEventListener('wheel', wheel);
  }, [change, image, viewport]);

  useEffect(() => () => { if (frame.current !== null) cancelAnimationFrame(frame.current); }, []);

  function down(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || pointers.current.size >= 2) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    pointers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
  }

  function move(event: ReactPointerEvent<HTMLDivElement>) {
    const old = pointers.current.get(event.pointerId);
    if (!old) return;
    const other = [...pointers.current].find(([id]) => id !== event.pointerId)?.[1];
    const next = { x: event.clientX, y: event.clientY };
    pointers.current.set(event.pointerId, next);
    const update = pending.current;
    if (other) {
      const before = Math.hypot(old.x - other.x, old.y - other.y);
      const after = Math.hypot(next.x - other.x, next.y - other.y);
      const bounds = event.currentTarget.getBoundingClientRect();
      if (before > 0) update.factor *= after / before;
      update.x = (next.x + other.x) / 2 - bounds.left - bounds.width / 2;
      update.y = (next.y + other.y) / 2 - bounds.top - bounds.height / 2;
      update.dx += (next.x - old.x) / 2;
      update.dy += (next.y - old.y) / 2;
    } else {
      update.dx += next.x - old.x;
      update.dy += next.y - old.y;
    }
    if (frame.current !== null) return;
    frame.current = requestAnimationFrame(() => {
      frame.current = null;
      const { dx, dy, factor, x, y } = pending.current;
      pending.current = { dx: 0, dy: 0, factor: 1, x: 0, y: 0 };
      change((oldView) => {
        const zoomed = factor === 1 ? oldView : zoomAt(oldView, image, viewport, scaleOf(oldView, image, viewport) * factor, x, y);
        return { ...zoomed, x: zoomed.x + dx, y: zoomed.y + dy };
      });
    });
  }

  return (
    <div ref={stage} className={styles.stage} data-ready={ready} onPointerDown={down} onPointerMove={move}
      onPointerUp={(event) => pointers.current.delete(event.pointerId)} onPointerCancel={(event) => pointers.current.delete(event.pointerId)}
      onLostPointerCapture={(event) => pointers.current.delete(event.pointerId)}
      onDoubleClick={() => change((old) => old.zoom === null ? zoomAt(old, image, viewport, 1) : { ...old, zoom: null, x: 0, y: 0 })}>
      <img className={styles.picture} src={url} alt={name} draggable={false}
        onLoad={(event) => { const { naturalWidth, naturalHeight } = event.currentTarget; setImage({ width: naturalWidth, height: naturalHeight }); onReady(); }}
        onError={onError}
        style={{ width: image.width, height: image.height, visibility: ready ? 'visible' : 'hidden', transform: `translate(-50%, -50%) translate(${shown.x}px, ${shown.y}px) rotate(${shown.rotation}deg) scale(${scale})` }} />
    </div>
  );
}
