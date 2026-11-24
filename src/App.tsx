import { useEffect, useRef, useState } from 'react';
import { AeroButton, AeroPanel, AeroSelect, AeroToolbar } from './components/aero/controls';
import { AeroIcon } from './components/aero/Icon';
import { glassDensities, readDensity, saveDensity } from './lib/appearance';
import type { GlassDensity } from './lib/appearance';
import { getAppVersion, isDesktop, selectFolder } from './lib/desktop';
import styles from './App.module.css';

export default function App() {
  const [density, setDensity] = useState<GlassDensity>(readDensity);
  const [folder, setFolder] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pickerPending = useRef(false);
  const [error, setError] = useState('');
  const [preferenceNotice, setPreferenceNotice] = useState('');
  const [version, setVersion] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState(false);

  useEffect(() => {
    if (!isDesktop) return;
    let active = true;
    getAppVersion().then(
      (value) => { if (active) setVersion(value); },
      () => { if (active) setConnectionError(true); },
    );
    return () => { active = false; };
  }, []);

  async function chooseFolder() {
    if (pickerPending.current || !isDesktop) return;
    pickerPending.current = true;
    setBusy(true);
    setError('');
    try {
      const selected = await selectFolder();
      if (selected !== null) setFolder(selected);
    } catch {
      setError('The folder picker could not be opened. Please try again.');
    } finally {
      pickerPending.current = false;
      setBusy(false);
    }
  }

  function changeDensity(value: GlassDensity) {
    setDensity(value);
    setPreferenceNotice(saveDensity(value) ? '' : 'Appearance is applied for this session only.');
  }

  const connection = !isDesktop ? 'Browser preview' : connectionError ? 'Desktop connection failed' : version ? `AeroGallery ${version}` : 'Connecting…';

  return (
    <div className={styles.environment} data-glass={density}>
      <div className={`aero-glass ${styles.shell}`}>
        <header className={styles.heading}>
          <AeroIcon name="picture" size={32} />
          <h1>Photo Library</h1>
        </header>
        <AeroToolbar className={styles.toolbar}>
          <AeroButton variant="toolbar" disabled={busy || !isDesktop} onClick={chooseFolder}>
            <AeroIcon name="openFolder" /> {busy ? 'Opening…' : 'Add folder'}
          </AeroButton>
          <div className={styles.toolbarSpacer} />
          <label htmlFor="glass-density">Glass</label>
          <AeroSelect id="glass-density" value={density} onChange={(event) => {
            const value = glassDensities.find((item) => item === event.currentTarget.value);
            if (value) changeDensity(value);
          }}>
            <option value="clear">Clear</option>
            <option value="balanced">Balanced</option>
            <option value="solid">Solid</option>
          </AeroSelect>
        </AeroToolbar>
        <div className={styles.workspace}>
          <aside className={styles.sidebar} aria-label="Picture folders">
            <h2>Folders</h2>
            <div className={styles.libraryItem}><AeroIcon name="picture" size={18} /> Library</div>
            {folder && <div className={styles.folderItem} title={folder}>
              <AeroIcon name="folder" size={20} />
              <span>{folder.split(/[\\/]/).filter(Boolean).at(-1) ?? folder}</span>
            </div>}
          </aside>
          <main className={styles.content}>
            <AeroPanel className={styles.empty}>
              <AeroIcon name={folder ? 'folder' : 'picture'} size={64} />
              <h2>{folder ? 'Folder selected' : 'Your pictures'}</h2>
              <p>{folder ? 'Folder indexing is not available in this foundation build.' : isDesktop ? 'Choose a picture folder to get started.' : 'Open AeroGallery on your desktop to choose a local folder.'}</p>
              {folder && <p className={styles.path}>{folder}</p>}
              {isDesktop && <AeroButton variant="accent" disabled={busy} onClick={chooseFolder}>{folder ? 'Choose another folder' : 'Browse folders'}</AeroButton>}
              {error && <p role="alert" className={styles.error}>{error}</p>}
              {connectionError && <p role="alert" className={styles.error}>The native application bridge did not respond.</p>}
            </AeroPanel>
          </main>
        </div>
        <footer className={styles.status} role="status">
          <span>{preferenceNotice || (folder ? '1 folder selected · not indexed' : 'No folders selected')}</span>
          <span>{connection}</span>
        </footer>
      </div>
    </div>
  );
}
