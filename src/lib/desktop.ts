import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export const isDesktop = isTauri();

export async function getAppVersion(): Promise<string> {
  return invoke<string>('app_version');
}

export async function selectFolder(): Promise<string | null> {
  const result = await open({ directory: true, multiple: false, title: 'Select a picture folder' });
  if (result === null) return null;
  if (typeof result !== 'string') throw new Error('Expected one folder');
  return result;
}
