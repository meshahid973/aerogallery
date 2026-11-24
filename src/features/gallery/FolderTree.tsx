import { useState } from 'react';
import { AeroButton } from '../../components/aero/controls';
import { AeroIcon } from '../../components/aero/Icon';
import { childFolder, folderKey, rootFolder } from '../../lib/desktop';
import type { Folder, Root } from '../../lib/desktop';
import type { FolderCache } from './useGallery';
import styles from './Gallery.module.css';

const CHILD_BATCH = 100;

interface TreeProps {
  roots: Root[];
  active: Folder | null;
  cache: FolderCache;
  expanded: ReadonlySet<string>;
  busy: boolean;
  onSelect: (folder: Folder) => void;
  onToggle: (folder: Folder) => void;
}

type BranchProps = Omit<TreeProps, 'roots'> & { folder: Folder; name: string };

function FolderBranch({ folder, name, ...props }: BranchProps) {
  const [limit, setLimit] = useState(CHILD_BATCH);
  const key = folderKey(folder);
  const children = props.cache.get(key);
  const open = props.expanded.has(key) && children !== undefined;
  const current = props.active !== null && folderKey(props.active) === key;
  const canExpand = children === undefined || children.length > 0;
  return (
    <li>
      <div className={styles.treeRow} data-current={current}>
        {canExpand ? (
          <button type="button" className={styles.expander} aria-expanded={open} aria-label={`${open ? 'Collapse' : 'Expand'} ${name}`} disabled={props.busy} onClick={() => props.onToggle(folder)}>
            <span className={styles.chevron} data-open={open} />
          </button>
        ) : <span className={styles.expanderSpacer} />}
        <button type="button" className={styles.treeItem} aria-current={current ? 'location' : undefined} title={name} disabled={props.busy} onClick={() => props.onSelect(folder)}>
          <AeroIcon name="folder" size={18} />
          <span>{name}</span>
        </button>
      </div>
      {open && children && children.length > 0 && (
        <ul className={styles.branch}>
          {children.slice(0, limit).map((child) => (
            <FolderBranch key={child} {...props} folder={childFolder(folder, child)} name={child} />
          ))}
          {children.length > limit && <li><AeroButton variant="toolbar" onClick={() => setLimit((value) => value + CHILD_BATCH)}>Show more</AeroButton></li>}
        </ul>
      )}
    </li>
  );
}

export function FolderTree({ roots, ...props }: TreeProps) {
  return (
    <nav className={styles.sidebar} aria-label="Picture folders">
      <h2 className={styles.sidebarHeading}><AeroIcon name="picture" size={18} /> Folders</h2>
      <ul className={styles.tree}>
        {roots.map((root) => <FolderBranch key={root.id} {...props} folder={rootFolder(root)} name={root.name} />)}
      </ul>
    </nav>
  );
}
