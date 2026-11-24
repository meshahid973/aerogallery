use crate::{
    imaging::{self, PhotoMetadata},
    paths::{allowed_photo, fingerprint, is_link, valid_component},
};
use rayon::prelude::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

const MAX_ROOTS: usize = 64;
const MAX_ENTRIES: usize = 250_000;

#[derive(Clone, Debug, Serialize)]
pub struct Root {
    pub id: u32,
    pub name: String,
}

#[derive(Serialize)]
pub struct Photo {
    pub name: String,
    pub bytes: u64,
    pub modified: u64,
    pub stamp: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Serialize)]
pub struct Listing {
    pub folders: Vec<String>,
    pub photos: Vec<Photo>,
    pub skipped: usize,
    pub revision: i64,
}

#[derive(Clone)]
pub struct Source {
    pub root: Root,
    pub path: PathBuf,
}

struct Entry {
    name: String,
    path: String,
    is_dir: bool,
    bytes: u64,
    modified: u64,
    stamp: String,
}

pub struct Index {
    connection: Mutex<Connection>,
}

fn db_error(error: rusqlite::Error) -> String {
    eprintln!("Library database: {error}");
    "The library database is unavailable.".to_string()
}

impl Index {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(db_error)?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_error)?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(db_error)?;
        if version > 1 {
            return Err("The library was created by a newer version.".into());
        }
        connection.execute_batch("
            PRAGMA foreign_keys=ON;
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=FULL;
            BEGIN;
            CREATE TABLE IF NOT EXISTS roots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS revision_sequence (id INTEGER PRIMARY KEY CHECK(id=1), value INTEGER NOT NULL);
            INSERT OR IGNORE INTO revision_sequence VALUES(1,0);
            CREATE TABLE IF NOT EXISTS directories (
                root INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                revision INTEGER NOT NULL,
                skipped INTEGER NOT NULL,
                PRIMARY KEY(root,path)
            ) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS entries (
                root INTEGER NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                parent TEXT NOT NULL,
                name TEXT NOT NULL,
                is_dir INTEGER NOT NULL,
                bytes INTEGER NOT NULL,
                modified INTEGER NOT NULL,
                stamp TEXT NOT NULL,
                width INTEGER,
                height INTEGER,
                metadata TEXT,
                PRIMARY KEY(root,path)
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS entries_parent ON entries(root,parent,is_dir,name);
            CREATE INDEX IF NOT EXISTS entries_pending_metadata ON entries(root,parent,path) WHERE is_dir=0 AND metadata IS NULL;
            PRAGMA user_version=1;
            COMMIT;
        ").map_err(db_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| "The library database is unavailable.".into())
    }

    pub fn sources(&self) -> Result<Vec<Source>, String> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare("SELECT id,name,path FROM roots ORDER BY id")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(Source {
                    root: Root {
                        id: row.get(0)?,
                        name: row.get(1)?,
                    },
                    path: PathBuf::from(row.get::<_, String>(2)?),
                })
            })
            .map_err(db_error)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)
    }

    pub fn root_path(&self, id: u32) -> Result<PathBuf, String> {
        self.lock()?
            .query_row("SELECT path FROM roots WHERE id=?1", [id], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(db_error)?
            .map(PathBuf::from)
            .ok_or_else(|| "This folder is no longer in the library.".into())
    }

    pub fn add(&self, path: &Path, name: String) -> Result<Root, String> {
        let path = path.to_str().ok_or("Unsupported folder name.")?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(db_error)?;
        if let Some(root) = transaction
            .query_row("SELECT id,name FROM roots WHERE path=?1", [path], |row| {
                Ok(Root {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .optional()
            .map_err(db_error)?
        {
            return Ok(root);
        }
        let count: i64 = transaction
            .query_row("SELECT count(*) FROM roots", [], |row| row.get(0))
            .map_err(db_error)?;
        if count >= MAX_ROOTS as i64 {
            return Err("Remove a folder before adding another.".into());
        }
        transaction
            .execute(
                "INSERT INTO roots(path,name) VALUES(?1,?2)",
                params![path, name],
            )
            .map_err(db_error)?;
        let id = u32::try_from(transaction.last_insert_rowid()).map_err(|_| "Too many folders.")?;
        transaction.commit().map_err(db_error)?;
        Ok(Root { id, name })
    }

    pub fn remove(&self, id: u32) -> Result<(), String> {
        self.lock()?
            .execute("DELETE FROM roots WHERE id=?1", [id])
            .map_err(db_error)?;
        Ok(())
    }

    pub fn revision(&self, id: u32, parent: &str) -> Result<Option<i64>, String> {
        self.lock()?
            .query_row(
                "SELECT revision FROM directories WHERE root=?1 AND path=?2",
                params![id, parent],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)
    }

    pub fn list(&self, id: u32, parent: &str) -> Result<Option<Listing>, String> {
        let connection = self.lock()?;
        let Some((revision, skipped)) = connection
            .query_row(
                "SELECT revision,skipped FROM directories WHERE root=?1 AND path=?2",
                params![id, parent],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(db_error)?
        else {
            return Ok(None);
        };
        let mut listing = Listing {
            folders: Vec::new(),
            photos: Vec::new(),
            skipped: skipped as usize,
            revision,
        };
        let mut statement = connection.prepare("SELECT name,is_dir,bytes,modified,stamp,width,height FROM entries WHERE root=?1 AND parent=?2 ORDER BY name COLLATE NOCASE").map_err(db_error)?;
        let mut rows = statement.query(params![id, parent]).map_err(db_error)?;
        while let Some(row) = rows.next().map_err(db_error)? {
            if row.get::<_, bool>(1).map_err(db_error)? {
                listing.folders.push(row.get(0).map_err(db_error)?);
            } else {
                listing.photos.push(Photo {
                    name: row.get(0).map_err(db_error)?,
                    bytes: row.get::<_, i64>(2).map_err(db_error)? as u64,
                    modified: row.get::<_, i64>(3).map_err(db_error)? as u64,
                    stamp: row.get(4).map_err(db_error)?,
                    width: row.get(5).map_err(db_error)?,
                    height: row.get(6).map_err(db_error)?,
                });
            }
        }
        Ok(Some(listing))
    }

    pub fn metadata(&self, id: u32, path: &str) -> Result<Option<PhotoMetadata>, String> {
        let result: Option<String> = self
            .lock()?
            .query_row(
                "SELECT metadata FROM entries WHERE root=?1 AND path=?2 AND is_dir=0",
                params![id, path],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .flatten();
        result
            .map(|json| {
                serde_json::from_str(&json)
                    .map_err(|_| "Picture metadata is unavailable.".to_string())
            })
            .transpose()
    }

    fn existing(&self, id: u32, parent: &str) -> Result<HashMap<String, (bool, String)>, String> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare("SELECT name,is_dir,stamp FROM entries WHERE root=?1 AND parent=?2")
            .map_err(db_error)?;
        let rows = statement
            .query_map(params![id, parent], |row| {
                Ok((row.get(0)?, (row.get(1)?, row.get(2)?)))
            })
            .map_err(db_error)?;
        rows.collect::<rusqlite::Result<HashMap<_, _>>>()
            .map_err(db_error)
    }

    pub fn scan(
        &self,
        id: u32,
        parent: &str,
        directory: &Path,
        excluded: &Path,
    ) -> Result<(Vec<String>, Vec<String>), String> {
        let existing = self.existing(id, parent)?;
        let mut seen = HashSet::new();
        let mut folders = Vec::new();
        let mut changes = Vec::new();
        let mut skipped = 0usize;
        let mut incomplete = false;
        let entries = fs::read_dir(directory).map_err(|_| "Cannot read this folder.")?;
        for (index, entry) in entries.enumerate() {
            if index >= MAX_ENTRIES {
                return Err("This folder contains more than 250,000 entries.".into());
            }
            let Ok(entry) = entry else {
                skipped += 1;
                incomplete = true;
                continue;
            };
            let Ok(name) = entry.file_name().into_string() else {
                skipped += 1;
                incomplete = true;
                continue;
            };
            if !valid_component(&name) {
                skipped += 1;
                continue;
            }
            let path = entry.path();
            if path.starts_with(excluded) {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                skipped += 1;
                incomplete = true;
                continue;
            };
            if !kind.is_dir() && !(kind.is_file() && allowed_photo(&path)) {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                skipped += 1;
                incomplete = true;
                continue;
            };
            if is_link(&metadata) || (!metadata.is_dir() && !metadata.is_file()) {
                continue;
            }
            if metadata.len() > i64::MAX as u64 {
                skipped += 1;
                continue;
            }
            let is_dir = metadata.is_dir();
            if !is_dir && !allowed_photo(&path) {
                continue;
            }
            let (stamp, modified) = if is_dir {
                ("directory".into(), 0)
            } else {
                fingerprint(&metadata)
            };
            seen.insert(name.clone());
            if is_dir {
                folders.push(name.clone());
            }
            if existing
                .get(&name)
                .is_some_and(|(kind, old)| *kind == is_dir && old == &stamp)
            {
                continue;
            }
            let relative = if parent.is_empty() {
                name.clone()
            } else {
                format!("{parent}/{name}")
            };
            changes.push(Entry {
                name,
                path: relative,
                is_dir,
                bytes: if is_dir { 0 } else { metadata.len() },
                modified,
                stamp,
            });
        }
        let missing: Vec<String> = if incomplete {
            Vec::new()
        } else {
            existing
                .keys()
                .filter(|name| !seen.contains(*name))
                .cloned()
                .collect()
        };
        if fs::canonicalize(directory).ok().as_deref() != Some(directory) {
            return Err("Folder changed while scanning.".into());
        }
        let mut invalidated = Vec::new();
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(db_error)?;
        let root_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM roots WHERE id=?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if !root_exists {
            return Err("This folder is no longer in the library.".into());
        }
        if !parent.is_empty() {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM entries WHERE root=?1 AND path=?2 AND is_dir=1)",
                    params![id, parent],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if !exists {
                return Err("Folder changed while scanning.".into());
            }
        }
        let prior: Option<(i64, i64)> = transaction
            .query_row(
                "SELECT revision,skipped FROM directories WHERE root=?1 AND path=?2",
                params![id, parent],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        let changed = !changes.is_empty()
            || !missing.is_empty()
            || prior.is_none_or(|(_, old)| old != skipped as i64);
        for name in missing {
            let path = if parent.is_empty() {
                name
            } else {
                format!("{parent}/{name}")
            };
            Self::delete_subtree(&transaction, id, &path)?;
            invalidated.push(path);
        }
        {
            let mut upsert = transaction.prepare_cached("INSERT INTO entries(root,path,parent,name,is_dir,bytes,modified,stamp,width,height,metadata) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(root,path) DO UPDATE SET parent=excluded.parent,name=excluded.name,is_dir=excluded.is_dir,bytes=excluded.bytes,modified=excluded.modified,stamp=excluded.stamp,width=excluded.width,height=excluded.height,metadata=excluded.metadata").map_err(db_error)?;
            for entry in changes {
                if existing
                    .get(&entry.name)
                    .is_some_and(|(kind, _)| *kind != entry.is_dir)
                {
                    Self::delete_subtree(&transaction, id, &entry.path)?;
                }
                upsert
                    .execute(params![
                        id,
                        entry.path,
                        parent,
                        entry.name,
                        entry.is_dir,
                        entry.bytes as i64,
                        entry.modified as i64,
                        entry.stamp,
                        Option::<u32>::None,
                        Option::<u32>::None,
                        Option::<String>::None
                    ])
                    .map_err(db_error)?;
                if existing.contains_key(&entry.name) {
                    invalidated.push(entry.path);
                }
            }
        }
        if changed {
            transaction
                .execute("UPDATE revision_sequence SET value=value+1 WHERE id=1", [])
                .map_err(db_error)?;
            transaction.execute("INSERT INTO directories(root,path,revision,skipped) VALUES(?1,?2,(SELECT value FROM revision_sequence WHERE id=1),?3) ON CONFLICT(root,path) DO UPDATE SET revision=excluded.revision,skipped=excluded.skipped", params![id,parent,skipped as i64]).map_err(db_error)?;
        }
        transaction.commit().map_err(db_error)?;
        folders.sort_unstable();
        Ok((folders, invalidated))
    }

    pub fn enrich(&self, id: u32, parent: &str, directory: &Path, pool: &rayon::ThreadPool) -> Result<(), String> {
        loop {
            let batch: Vec<(String, String, String)> = {
                let connection = self.lock()?;
                let mut statement = connection.prepare_cached("SELECT path,name,stamp FROM entries WHERE root=?1 AND parent=?2 AND is_dir=0 AND metadata IS NULL ORDER BY path LIMIT 64").map_err(db_error)?;
                let rows = statement.query_map(params![id,parent], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(db_error)?;
                rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_error)?
            };
            if batch.is_empty() { return Ok(()); }
            let results: Vec<_> = pool.install(|| batch.into_par_iter().map(|(path,name,stamp)| {
                let file=directory.join(name);
                let current=|| fs::symlink_metadata(&file).is_ok_and(|meta| !is_link(&meta) && fingerprint(&meta).0==stamp);
                let metadata=if current() { imaging::metadata(&file) } else { PhotoMetadata::default() };
                (path,stamp,metadata,current())
            }).collect());
            let mut connection=self.lock()?;
            let transaction=connection.transaction().map_err(db_error)?;
            let mut updated=0;
            {
                let mut statement=transaction.prepare_cached("UPDATE entries SET width=?4,height=?5,metadata=?6 WHERE root=?1 AND path=?2 AND stamp=?3 AND metadata IS NULL").map_err(db_error)?;
                for (path,stamp,metadata,current) in results {
                    if !current { continue; }
                    let json=serde_json::to_string(&metadata).map_err(|_|"Cannot store picture metadata.")?;
                    updated+=statement.execute(params![id,path,stamp,metadata.width,metadata.height,json]).map_err(db_error)?;
                }
            }
            if updated>0 {
                transaction.execute("UPDATE revision_sequence SET value=value+1 WHERE id=1", []).map_err(db_error)?;
                transaction.execute("UPDATE directories SET revision=(SELECT value FROM revision_sequence WHERE id=1) WHERE root=?1 AND path=?2",params![id,parent]).map_err(db_error)?;
            }
            transaction.commit().map_err(db_error)?;
            if updated==0 { return Ok(()); }
            std::thread::yield_now();
        }
    }

    fn delete_subtree(connection: &Connection, id: u32, path: &str) -> Result<(), String> {
        for table in ["entries", "directories"] {
            connection
                .execute(
                    &format!("DELETE FROM {table} WHERE root=?1 AND path=?2"),
                    params![id, path],
                )
                .map_err(db_error)?;
            connection
                .execute(
                    &format!(
                        "DELETE FROM {table} WHERE root=?1 AND path>=?2||'/' AND path<?2||'0'"
                    ),
                    params![id, path],
                )
                .map_err(db_error)?;
        }
        Ok(())
    }
}
