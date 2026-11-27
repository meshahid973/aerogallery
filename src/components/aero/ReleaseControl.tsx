import { useCallback, useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { AeroButton } from './controls';
import { defaultApps } from '../../lib/desktop';
import { AeroIcon } from './Icon';
import { checkRelease, isNewerRelease } from '../../lib/releases';
import type { ReleaseCheck } from '../../lib/releases';
import styles from '../../App.module.css';

export function ReleaseControl() {
  const [version, setVersion] = useState('');
  const [result, setResult] = useState<ReleaseCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState('');
  const [openError, setOpenError] = useState('');
  const mounted = useRef(false);
  const pending = useRef(false);
  const openingPending = useRef(false);
  const installed = useRef('');

  const check = useCallback(async (force: boolean) => {
    if (pending.current) return;
    pending.current = true;
    setChecking(true);
    setError('');
    try {
      const current = installed.current || await getVersion();
      if (!mounted.current) return;
      installed.current = current;
      setVersion(current);
      const latest = await checkRelease(force);
      if (latest.tag !== null) isNewerRelease(latest.tag, current);
      if (mounted.current) setResult(latest);
    } catch (failure) {
      if (mounted.current) setError(failure instanceof Error && failure.message.startsWith('GitHub is limiting') ? failure.message : 'Unable to check for updates.');
    } finally {
      pending.current = false;
      if (mounted.current) setChecking(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    const timer = window.setTimeout(() => { void check(false); }, 2000);
    return () => { mounted.current = false; window.clearTimeout(timer); };
  }, [check]);

  const available = !!result?.tag && !!version && isNewerRelease(result.tag, version);
  const message = checking ? 'Checking for updates…' : error || (result
    ? available ? `Version ${result.tag} is available.` : result.tag ? 'You are up to date.' : 'No published releases yet.'
    : 'Check for the latest version.');

  async function openRelease() {
    if (openingPending.current) return;
    openingPending.current = true;
    setOpening(true);
    setOpenError('');
    try {
      await invoke('open_release', { tag: available ? result?.tag : null });
    } catch {
      if (mounted.current) setOpenError('Could not open GitHub Releases.');
    } finally {
      openingPending.current = false;
      if (mounted.current) setOpening(false);
    }
  }

  return <>
    <AeroButton variant={available ? 'accent' : 'toolbar'} className={available ? '' : 'aero-icon-button'} popoverTarget="release-panel" aria-label={available ? 'Update available' : 'Application options and updates'} title={available ? 'A new version is available' : 'Application options and updates'}>
      <AeroIcon name="refresh" size={16} /> {available && 'Update available'}
    </AeroButton>
    <div id="release-panel" popover="auto" className={`${styles.appearance} ${styles.releasePanel}`} aria-label="Software updates">
      <div className={styles.releaseInfo}>
        <AeroIcon name="picture" size={32} />
        <div><strong>AeroGallery{version && ` ${version}`}</strong><p role="status">{message}</p></div>
      </div>
      <div className={styles.releaseActions}>
        <AeroButton disabled={checking} onClick={() => void check(true)}>Check now</AeroButton>
        <AeroButton variant={available ? 'accent' : 'standard'} disabled={opening} onClick={() => void openRelease()}>{available ? 'View release' : 'GitHub Releases'}</AeroButton>
      </div>
      <AeroButton variant="toolbar" className={styles.defaultApps} onClick={() => { setOpenError(''); void defaultApps().catch(() => setOpenError('Could not open Windows Default Apps.')); }}>Windows Default Apps</AeroButton>
      {openError && <p className={styles.preferenceError} role="alert">{openError}</p>}
    </div>
  </>;
}
