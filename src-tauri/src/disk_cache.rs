use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};

const MAX_BYTES: i64 = 512 * 1024 * 1024;
const MAX_ENTRIES: i64 = 20_000;

pub struct DiskCache {
    connection: Mutex<Connection>,
    max_bytes: i64,
    max_entries: i64,
}

impl DiskCache {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("
            PRAGMA auto_vacuum=INCREMENTAL;
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS thumbnails (
                id INTEGER PRIMARY KEY,
                root INTEGER NOT NULL,
                path TEXT NOT NULL,
                stamp TEXT NOT NULL,
                data BLOB NOT NULL,
                bytes INTEGER NOT NULL,
                used INTEGER NOT NULL,
                UNIQUE(root,path)
            );
            CREATE INDEX IF NOT EXISTS cache_lru ON thumbnails(used,id);
            CREATE TABLE IF NOT EXISTS cache_stats (id INTEGER PRIMARY KEY CHECK(id=1), bytes INTEGER NOT NULL, entries INTEGER NOT NULL);
            INSERT OR IGNORE INTO cache_stats VALUES(1,0,0);
            CREATE TRIGGER IF NOT EXISTS cache_insert AFTER INSERT ON thumbnails BEGIN
                UPDATE cache_stats SET bytes=bytes+new.bytes, entries=entries+1 WHERE id=1;
            END;
            CREATE TRIGGER IF NOT EXISTS cache_delete AFTER DELETE ON thumbnails BEGIN
                UPDATE cache_stats SET bytes=bytes-old.bytes, entries=entries-1 WHERE id=1;
            END;
            UPDATE cache_stats SET bytes=(SELECT coalesce(sum(bytes),0) FROM thumbnails), entries=(SELECT count(*) FROM thumbnails) WHERE id=1;
        ")?;
        let result = Self {
            connection: Mutex::new(connection),
            max_bytes: MAX_BYTES,
            max_entries: MAX_ENTRIES,
        };
        result.trim()?;
        Ok(result)
    }

    pub fn get(&self, root: u32, path: &str, stamp: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let result = connection
            .query_row(
                "SELECT data FROM thumbnails WHERE root=?1 AND path=?2 AND stamp=?3",
                params![root, path, stamp],
                |row| row.get(0),
            )
            .optional()?;
        if result.is_some() {
            connection.execute("UPDATE thumbnails SET used=unixepoch() WHERE root=?1 AND path=?2 AND stamp=?3 AND used<unixepoch()", params![root,path,stamp])?;
        }
        Ok(result)
    }

    pub fn put(&self, root: u32, path: &str, stamp: &str, data: &[u8]) -> rusqlite::Result<()> {
        if data.len() as i64 > self.max_bytes {
            return Ok(());
        }
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM thumbnails WHERE root=?1 AND path=?2",
            params![root, path],
        )?;
        transaction.execute("INSERT INTO thumbnails(root,path,stamp,data,bytes,used) VALUES(?1,?2,?3,?4,?5,unixepoch())", params![root,path,stamp,data,data.len() as i64])?;
        Self::evict(&transaction, self.max_bytes, self.max_entries)?;
        transaction.commit()?;
        Ok(())
    }

    fn evict(connection: &Connection, max_bytes: i64, max_entries: i64) -> rusqlite::Result<()> {
        loop {
            let (bytes, entries): (i64, i64) = connection.query_row(
                "SELECT bytes,entries FROM cache_stats WHERE id=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if bytes <= max_bytes && entries <= max_entries {
                break;
            }
            connection.execute("DELETE FROM thumbnails WHERE id=(SELECT id FROM thumbnails ORDER BY used,id LIMIT 1)", [])?;
        }
        Ok(())
    }

    pub fn trim(&self) -> rusqlite::Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        Self::evict(&connection, self.max_bytes, self.max_entries)?;
        connection
            .execute_batch("PRAGMA incremental_vacuum(1024); PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }

    pub fn remove_root(&self, root: u32) -> rusqlite::Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute("DELETE FROM thumbnails WHERE root=?1", [root])?;
        Ok(())
    }

    pub fn invalidate_many(&self, root: u32, paths: &[String]) -> rusqlite::Result<()> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        {
            let mut exact =
                transaction.prepare_cached("DELETE FROM thumbnails WHERE root=?1 AND path=?2")?;
            let mut subtree = transaction.prepare_cached(
                "DELETE FROM thumbnails WHERE root=?1 AND path>=?2||'/' AND path<?2||'0'",
            )?;
            for path in paths {
                exact.execute(params![root, path])?;
                subtree.execute(params![root, path])?;
            }
        }
        transaction.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_survives_reopen_and_evicts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        let mut cache = DiskCache::open(&path).unwrap();
        cache.max_bytes = 8;
        cache.max_entries = 2;
        cache.put(1, "a", "v1", &[1; 4]).unwrap();
        cache.put(1, "b", "v1", &[2; 4]).unwrap();
        cache.put(1, "c", "v1", &[3; 4]).unwrap();
        assert!(cache.get(1, "a", "v1").unwrap().is_none());
        drop(cache);
        let cache = DiskCache::open(&path).unwrap();
        assert_eq!(cache.get(1, "c", "v1").unwrap(), Some(vec![3; 4]));
        assert!(cache.get(1, "c", "v2").unwrap().is_none());
        cache.remove_root(1).unwrap();
        assert!(cache.get(1, "c", "v1").unwrap().is_none());
    }
}
