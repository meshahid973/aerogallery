import { invoke, isTauri } from '@tauri-apps/api/core';

export const isDesktop = isTauri();

export interface Root { id: number; name: string }
export interface Folder { rootId: number; parts: string[] }
export interface Photo {
  name: string;
  bytes: number;
  modified: number;
  stamp: string;
  width: number | null;
  height: number | null;
}
export interface PhotoMetadata {
  width: number | null;
  height: number | null;
  orientation: number | null;
  camera_make: string | null;
  camera_model: string | null;
  captured_at: string | null;
  iso: string | null;
  exposure: string | null;
  aperture: string | null;
  focal_length: string | null;
}
export interface Listing {
  folders: string[];
  photos: Photo[];
  skipped: number;
  revision: number;
}

export const folderKey = (folder: Folder) => JSON.stringify([folder.rootId, ...folder.parts]);
export const rootFolder = (root: Root): Folder => ({ rootId: root.id, parts: [] });
export const childFolder = (folder: Folder, name: string): Folder => ({ ...folder, parts: [...folder.parts, name] });

function bounded<T>(request: Promise<T>, milliseconds: number, message: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => reject(message), milliseconds);
    void request.then(resolve, reject).finally(() => window.clearTimeout(timer));
  });
}

export const listRoots = () => bounded(invoke<Root[]>('list_roots'), 15000, 'Cannot connect to the library.');
export const pickFolder = () => invoke<Root | null>('pick_folder');
export const readFolder = (folder: Folder, refresh = false) => bounded(invoke<Listing>('read_folder', { ...folder, refresh }), 15000, 'Folder is taking longer than expected. Try Refresh.');
export const folderRevision = (folder: Folder) => bounded(invoke<number>('folder_revision', { ...folder }), 5000, 'Cannot check folder changes.');
export const removeRoot = (rootId: number) => invoke<void>('remove_root', { rootId });
function binaryBuffer(value: unknown): ArrayBuffer {
  if (value instanceof ArrayBuffer) return value;
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength).slice().buffer;
  if (Array.isArray(value) && value.length <= 64 * 1024 * 1024) return Uint8Array.from(value).buffer;
  throw new Error('The desktop returned an invalid image response.');
}

export const readThumbnail = (folder: Folder, name: string) =>
  invoke<unknown>('thumbnail', { rootId: folder.rootId, parts: [...folder.parts, name] }).then(binaryBuffer);
export const readMetadata = (folder: Folder, name: string) =>
  invoke<PhotoMetadata | null>('photo_metadata', { rootId: folder.rootId, parts: [...folder.parts, name] });

export const readFullImage = (folder: Folder, photo: Photo) =>
  (folder.rootId === 0 ? invoke<unknown>('external_image', { session: folder.parts[0], name: photo.name, stamp: photo.stamp }) :
    invoke<unknown>('full_image', { rootId: folder.rootId, parts: [...folder.parts, photo.name], stamp: photo.stamp })).then(binaryBuffer);

export interface ExternalSelection { session: string; roots: Root[]; photos: Photo[]; rejected: number }
export const takeShellItems = () => invoke<ExternalSelection | null>('take_shell_items');
export const releaseExternal = (session: string) => invoke<void>('release_external', { session });
export const defaultApps = () => invoke<void>('default_apps');
export const isWindows = isDesktop && /Windows/i.test(navigator.userAgent);
export const setWallpaper = (folder: Folder, photo: Photo, position: string) =>
  invoke<void>('set_wallpaper', { rootId: folder.rootId, parts: [...folder.parts, photo.name], stamp: photo.stamp, position });
export const setViewerFullscreen = (enabled: boolean) => invoke<void>('viewer_fullscreen', { enabled });
