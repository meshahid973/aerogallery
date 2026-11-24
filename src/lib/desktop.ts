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
export const readThumbnail = (folder: Folder, name: string) =>
  invoke<ArrayBuffer>('thumbnail', { rootId: folder.rootId, parts: [...folder.parts, name] });
export const readMetadata = (folder: Folder, name: string) =>
  invoke<PhotoMetadata | null>('photo_metadata', { rootId: folder.rootId, parts: [...folder.parts, name] });
