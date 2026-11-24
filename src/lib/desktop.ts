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

export const listRoots = () => invoke<Root[]>('list_roots');
export const pickFolder = () => invoke<Root | null>('pick_folder');
export const readFolder = (folder: Folder, refresh = false) => invoke<Listing>('read_folder', { ...folder, refresh });
export const folderRevision = (folder: Folder) => invoke<number>('folder_revision', { ...folder });
export const removeRoot = (rootId: number) => invoke<void>('remove_root', { rootId });
export const readThumbnail = (folder: Folder, name: string) =>
  invoke<ArrayBuffer>('thumbnail', { rootId: folder.rootId, parts: [...folder.parts, name] });
export const readMetadata = (folder: Folder, name: string) =>
  invoke<PhotoMetadata | null>('photo_metadata', { rootId: folder.rootId, parts: [...folder.parts, name] });
