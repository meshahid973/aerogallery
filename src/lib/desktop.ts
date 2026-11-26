import { invoke, isTauri } from '@tauri-apps/api/core';

export const isDesktop = isTauri();

export interface Root {
  id: number;
  name: string;
}

export interface Folder {
  rootId: number;
  parts: string[];
}

export interface Photo {
  name: string;
  bytes: number;
  modified: number;
}

export interface Listing {
  folders: string[];
  photos: Photo[];
  skipped: number;
}

export const folderKey = (folder: Folder) => JSON.stringify([folder.rootId, ...folder.parts]);
export const rootFolder = (root: Root): Folder => ({ rootId: root.id, parts: [] });
export const childFolder = (folder: Folder, name: string): Folder => ({ ...folder, parts: [...folder.parts, name] });

export const listRoots = () => invoke<Root[]>('list_roots');
export const pickFolder = () => invoke<Root | null>('pick_folder');
export const readFolder = (folder: Folder) => invoke<Listing>('read_folder', { ...folder });
export const removeRoot = (rootId: number) => invoke<void>('remove_root', { rootId });
export const readThumbnail = (folder: Folder, name: string) =>
  invoke<ArrayBuffer>('thumbnail', { rootId: folder.rootId, parts: [...folder.parts, name] });
