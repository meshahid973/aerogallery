export interface Dimensions { width: number; height: number }
export interface View { zoom: number | null; rotation: number; x: number; y: number }
export const initialView: View = { zoom: null, rotation: 0, x: 0, y: 0 };
const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

export function rotatedSize(image: Dimensions, rotation: number): Dimensions {
  return rotation % 180 === 0 ? image : { width: image.height, height: image.width };
}

export function fitScale(image: Dimensions, viewport: Dimensions, rotation: number) {
  const bounds = rotatedSize(image, rotation);
  return Math.min(1, Math.max(0.001, Math.min(viewport.width / Math.max(1, bounds.width), viewport.height / Math.max(1, bounds.height))));
}

export function scaleOf(view: View, image: Dimensions, viewport: Dimensions) {
  return view.zoom ?? fitScale(image, viewport, view.rotation);
}

export function constrain(view: View, image: Dimensions, viewport: Dimensions): View {
  const bounds = rotatedSize(image, view.rotation);
  const scale = scaleOf(view, image, viewport);
  const x = Math.max(0, (bounds.width * scale - viewport.width) / 2);
  const y = Math.max(0, (bounds.height * scale - viewport.height) / 2);
  return { ...view, x: clamp(view.x, -x, x), y: clamp(view.y, -y, y) };
}

export function zoomAt(view: View, image: Dimensions, viewport: Dimensions, target: number, anchorX = 0, anchorY = 0): View {
  const scale = scaleOf(view, image, viewport);
  const zoom = clamp(target, Math.min(0.1, fitScale(image, viewport, view.rotation)), 8);
  const ratio = zoom / scale;
  return constrain({ ...view, zoom, x: anchorX - (anchorX - view.x) * ratio, y: anchorY - (anchorY - view.y) * ratio }, image, viewport);
}
