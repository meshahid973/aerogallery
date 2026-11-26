pub use crate::index::{Listing, Root};
use crate::{
    disk_cache::DiskCache,
    imaging::{self, PhotoMetadata},
    index::{Index, Source},
    paths::{self, fingerprint, is_link},
};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use rayon::prelude::*;
use std::hash::{Hash, Hasher};
use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, Mutex, Weak,
    },
    thread,
    time::{Duration, Instant},
};

const RAM_BYTES: usize = 16 * 1024 * 1024;
const RAM_ENTRIES: usize = 64;
const RECONCILE: Duration = Duration::from_secs(120);

#[derive(Clone)]
pub struct Library(Arc<Inner>);

struct Inner {
    index: Index,
    disk: Option<DiskCache>,
    data: PathBuf,
    pool: rayon::ThreadPool,
    scan_pool: rayon::ThreadPool,
    picker: Arc<AtomicUsize>,
    listing: Arc<AtomicUsize>,
    decoding: Arc<AtomicUsize>,
    viewing: Arc<AtomicUsize>,
    scan_locks: Vec<Mutex<()>>,
    cache_write: Mutex<()>,
    cache: Mutex<MemoryCache>,
    wake: SyncSender<Work>,
    overflow: Arc<std::sync::atomic::AtomicBool>,
}

enum Work {
    Refresh,
    Folder(u32, Vec<String>),
    Event(Event),
}

pub struct Permit(Arc<AtomicUsize>);

impl Permit {
    fn acquire(counter: &Arc<AtomicUsize>, limit: usize) -> Result<Self, String> {
        let mut count = counter.load(Ordering::Relaxed);
        loop {
            if count >= limit {
                return Err("Please try again in a moment.".into());
            }
            match counter.compare_exchange_weak(
                count,
                count + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => count = actual,
            }
        }
        Ok(Self(counter.clone()))
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

#[derive(PartialEq, Eq)]
struct CacheKey {
    root: u32,
    path: String,
    stamp: String,
}

#[derive(Default)]
struct MemoryCache {
    items: VecDeque<(CacheKey, Vec<u8>)>,
    bytes: usize,
}

impl MemoryCache {
    fn get(&mut self, key: &CacheKey) -> Option<Vec<u8>> {
        let index = self
            .items
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        let entry = self.items.remove(index)?;
        let data = entry.1.clone();
        self.items.push_front(entry);
        Some(data)
    }

    fn put(&mut self, key: CacheKey, data: Vec<u8>) {
        if data.len() > RAM_BYTES {
            return;
        }
        self.retain(|candidate| candidate.root != key.root || candidate.path != key.path);
        while self.items.len() >= RAM_ENTRIES || self.bytes + data.len() > RAM_BYTES {
            if let Some((_, previous)) = self.items.pop_back() {
                self.bytes -= previous.len();
            } else {
                break;
            }
        }
        self.bytes += data.len();
        self.items.push_front((key, data));
    }

    fn retain(&mut self, keep: impl Fn(&CacheKey) -> bool) {
        self.items.retain(|(key, _)| keep(key));
        self.bytes = self.items.iter().map(|(_, data)| data.len()).sum();
    }
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> String {
    "The library is unavailable.".into()
}

impl Library {
    pub fn open(data: &Path) -> Result<Self, String> {
        let (library, receiver) = Self::initialize(data)?;
        let weak = Arc::downgrade(&library.0);
        thread::Builder::new()
            .name("gallery-index".into())
            .spawn(move || worker(weak, receiver))
            .map_err(|_| "Cannot start library indexing.")?;
        Ok(library)
    }

    fn initialize(data: &Path) -> Result<(Self, Receiver<Work>), String> {
        fs::create_dir_all(data).map_err(|_| "Cannot create library storage.")?;
        let data = fs::canonicalize(data).map_err(|_| "Cannot access library storage.")?;
        let index = Index::open(&data.join("library.sqlite3"))?;
        let disk = DiskCache::open(&data.join("thumbnails.sqlite3"))
            .map_err(|error| eprintln!("Thumbnail cache unavailable: {error}"))
            .ok();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .thread_name(|id| format!("gallery-metadata-{id}"))
            .build()
            .map_err(|_| "Cannot start metadata workers.")?;
        let scan_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .thread_name(|id| format!("gallery-scan-{id}"))
            .build()
            .map_err(|_| "Cannot start folder workers.")?;
        let (wake, receiver) = mpsc::sync_channel(256);
        Ok((
            Self(Arc::new(Inner {
                index,
                disk,
                data,
                pool,
                scan_pool,
                picker: Arc::new(AtomicUsize::new(0)),
                listing: Arc::new(AtomicUsize::new(0)),
                decoding: Arc::new(AtomicUsize::new(0)),
                viewing: Arc::new(AtomicUsize::new(0)),
                scan_locks: (0..64).map(|_| Mutex::new(())).collect(),
                cache_write: Mutex::new(()),
                cache: Mutex::new(MemoryCache::default()),
                wake,
                overflow: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            })),
            receiver,
        ))
    }

    pub fn picker_permit(&self) -> Result<Permit, String> {
        Permit::acquire(&self.0.picker, 1)
    }
    pub fn listing_permit(&self) -> Result<Permit, String> {
        Permit::acquire(&self.0.listing, 3)
    }
    pub fn decode_permit(&self) -> Result<Permit, String> {
        Permit::acquire(&self.0.decoding, 2)
    }

    pub fn roots(&self) -> Result<Vec<Root>, String> {
        Ok(self
            .0
            .index
            .sources()?
            .into_iter()
            .map(|source| source.root)
            .collect())
    }

    fn schedule(&self, work: Work) {
        if matches!(self.0.wake.try_send(work), Err(TrySendError::Full(_))) {
            self.0.overflow.store(true, Ordering::Release);
        }
    }

    pub fn add(&self, selected: PathBuf) -> Result<Root, String> {
        let path = fs::canonicalize(&selected).map_err(|_| "Cannot open this folder.")?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| "Cannot open this folder.")?;
        if !metadata.is_dir() || is_link(&metadata) || path.starts_with(&self.0.data) {
            return Err("Choose an accessible picture folder.".into());
        }
        let name = selected.file_name().map_or_else(
            || selected.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let root = self.0.index.add(&path, name)?;
        self.schedule(Work::Folder(root.id, Vec::new()));
        Ok(root)
    }

    pub fn remove(&self, id: u32) -> Result<(), String> {
        let _gate = self.0.cache_write.lock().map_err(lock_error)?;
        self.0.index.remove(id)?;
        self.0
            .cache
            .lock()
            .map_err(lock_error)?
            .retain(|key| key.root != id);
        if let Some(disk) = &self.0.disk {
            if let Err(error) = disk.remove_root(id) {
                eprintln!("Cache removal: {error}");
            }
        }
        Ok(())
    }

    fn resolved(&self, id: u32, parts: &[String]) -> Result<PathBuf, String> {
        let path = paths::resolve(&self.0.index.root_path(id)?, parts)?;
        if path.starts_with(&self.0.data) {
            return Err("Application storage is not a picture folder.".into());
        }
        Ok(path)
    }

    fn scan_one(&self, id: u32, parts: &[String]) -> Result<Vec<String>, String> {
        let relative = paths::relative(parts)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        relative.hash(&mut hasher);
        let lock = &self.0.scan_locks[(hasher.finish() % self.0.scan_locks.len() as u64) as usize];
        let _guard = lock.lock().map_err(lock_error)?;
        let directory = self.resolved(id, parts)?;
        let (folders, invalidated) = self.0.index.scan(id, &relative, &directory, &self.0.data)?;
        drop(_guard);
        if !invalidated.is_empty() {
            let matches = |key: &CacheKey| {
                key.root == id
                    && invalidated.iter().any(|path| {
                        key.path == *path
                            || key
                                .path
                                .strip_prefix(path)
                                .is_some_and(|suffix| suffix.starts_with('/'))
                    })
            };
            self.0
                .cache
                .lock()
                .map_err(lock_error)?
                .retain(|key| !matches(key));
            if let Some(disk) = &self.0.disk {
                if let Err(error) = disk.invalidate_many(id, &invalidated) {
                    eprintln!("Cache invalidation: {error}");
                }
            }
        }
        Ok(folders)
    }

    pub fn list(&self, id: u32, parts: &[String], refresh: bool) -> Result<Listing, String> {
        self.resolved(id, parts)?;
        let relative = paths::relative(parts)?;
        if !refresh {
            if let Some(listing) = self.0.index.list(id, &relative)? {
                return Ok(listing);
            }
        }
        self.scan_one(id, parts)?;
        self.schedule(Work::Folder(id, parts.to_vec()));
        self.0
            .index
            .list(id, &relative)?
            .ok_or_else(|| "Cannot read this folder.".into())
    }

    pub fn revision(&self, id: u32, parts: &[String]) -> Result<i64, String> {
        self.resolved(id, parts)?;
        Ok(self
            .0
            .index
            .revision(id, &paths::relative(parts)?)?
            .unwrap_or(0))
    }

    pub fn metadata(&self, id: u32, parts: &[String]) -> Result<Option<PhotoMetadata>, String> {
        let path = self.resolved(id, parts)?;
        if let Some(metadata) = self.0.index.metadata(id, &paths::relative(parts)?)? {
            return Ok(Some(metadata));
        }
        if !paths::allowed_photo(&path) {
            return Ok(None);
        }
        Ok(Some(self.0.pool.install(|| imaging::metadata(&path))))
    }

    pub fn view_permit(&self) -> Result<Permit, String> {
        Permit::acquire(&self.0.viewing, 1)
    }

    pub fn full_image(
        &self,
        id: u32,
        parts: &[String],
        expected_stamp: &str,
    ) -> Result<Vec<u8>, String> {
        let path = self.resolved(id, parts)?;
        let stamp = || {
            fs::symlink_metadata(&path)
                .ok()
                .filter(|meta| meta.is_file() && !is_link(meta))
                .map(|meta| fingerprint(&meta).0)
        };
        if stamp().as_deref() != Some(expected_stamp) {
            return Err("Picture changed. Refresh the folder.".into());
        }
        let bytes = imaging::full_image(&path)?;
        if self.resolved(id, parts)? != path || stamp().as_deref() != Some(expected_stamp) {
            return Err("Picture changed while opening.".into());
        }
        Ok(bytes)
    }

    pub fn thumbnail(&self, id: u32, parts: &[String]) -> Result<Vec<u8>, String> {
        let path = self.resolved(id, parts)?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| "Picture unavailable.")?;
        if !metadata.is_file()
            || is_link(&metadata)
            || !paths::allowed_photo(&path)
            || metadata.len() > imaging::MAX_FILE_BYTES
        {
            return Err("Picture unavailable.".into());
        }
        let (stamp, _) = fingerprint(&metadata);
        let key = CacheKey {
            root: id,
            path: paths::relative(parts)?,
            stamp: format!("{}:{stamp}", imaging::THUMB_VERSION),
        };
        if let Some(data) = self.0.cache.lock().map_err(lock_error)?.get(&key) {
            return Ok(data);
        }
        if let Some(disk) = &self.0.disk {
            match disk.get(id, &key.path, &key.stamp) {
                Ok(Some(data)) => {
                    self.0
                        .cache
                        .lock()
                        .map_err(lock_error)?
                        .put(key, data.clone());
                    return Ok(data);
                }
                Ok(None) => {}
                Err(error) => eprintln!("Cache read: {error}"),
            }
        }
        let data = imaging::thumbnail(&path)?;
        let _gate = self.0.cache_write.lock().map_err(lock_error)?;
        let current = self.resolved(id, parts)?;
        if current != path
            || !fs::symlink_metadata(&current)
                .is_ok_and(|meta| !is_link(&meta) && fingerprint(&meta).0 == stamp)
        {
            return Err("Picture changed while decoding.".into());
        }
        if let Some(disk) = &self.0.disk {
            if let Err(error) = disk.put(id, &key.path, &key.stamp, &data) {
                eprintln!("Cache write: {error}");
            }
        }
        self.0
            .cache
            .lock()
            .map_err(lock_error)?
            .put(key, data.clone());
        Ok(data)
    }

    fn enrich_folder(&self, id: u32, parts: &[String]) {
        if let (Ok(directory), Ok(relative)) = (self.resolved(id, parts), paths::relative(parts)) {
            if let Err(error) = self.0.index.enrich(id, &relative, &directory, &self.0.pool) {
                eprintln!("Metadata indexing: {error}");
            }
        }
    }

    fn scan_tree(&self, source: &Source, start: Vec<String>) {
        let mut pending = vec![start];
        while !pending.is_empty() {
            let batch: Vec<_> = (0..2).filter_map(|_| pending.pop()).collect();
            let scanned: Vec<_> = self.0.scan_pool.install(|| {
                batch
                    .into_par_iter()
                    .map(|parts| {
                        let folders = match self.scan_one(source.root.id, &parts) {
                            Ok(folders) => {
                                self.enrich_folder(source.root.id, &parts);
                                folders
                            }
                            Err(_) => Vec::new(),
                        };
                        (parts, folders)
                    })
                    .collect()
            });
            for (parts, folders) in scanned {
                if parts.len() >= 128 {
                    continue;
                }
                for folder in folders.into_iter().rev() {
                    let mut child = parts.clone();
                    child.push(folder);
                    pending.push(child);
                }
            }
            if self.0.index.root_path(source.root.id).is_err() {
                break;
            }
            thread::yield_now();
        }
    }

    fn reconcile(&self, source: &Source, mut parts: Vec<String>) {
        loop {
            if let Ok(folders) = self.scan_one(source.root.id, &parts) {
                self.enrich_folder(source.root.id, &parts);
                for name in folders {
                    let mut child = parts.clone();
                    child.push(name);
                    if child.len() <= 128
                        && paths::relative(&child).ok().is_some_and(|path| {
                            self.0
                                .index
                                .revision(source.root.id, &path)
                                .ok()
                                .flatten()
                                .is_none()
                        })
                    {
                        self.scan_tree(source, child);
                    }
                }
                return;
            }
            if parts.pop().is_none() {
                return;
            }
        }
    }
}

fn event_relative(root: &Path, path: &Path) -> Option<Vec<String>> {
    let direct = path.strip_prefix(root).ok().map(Path::to_path_buf);
    let resolved = direct.or_else(|| {
        let canonical = fs::canonicalize(path).ok().or_else(|| {
            let parent = fs::canonicalize(path.parent()?).ok()?;
            Some(parent.join(path.file_name()?))
        })?;
        canonical.strip_prefix(root).ok().map(Path::to_path_buf)
    })?;
    let parts = resolved
        .components()
        .map(|part| part.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    paths::relative(&parts).ok()?;
    Some(parts)
}

fn worker(weak: Weak<Inner>, receiver: Receiver<Work>) {
    let Some(inner) = weak.upgrade() else {
        return;
    };
    let wake = inner.wake.clone();
    let overflow = inner.overflow.clone();
    let excluded = inner.data.clone();
    drop(inner);
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<Event>| match result {
            Ok(event)
                if !matches!(event.kind, EventKind::Access(_))
                    && (event.paths.is_empty()
                        || event.paths.iter().any(|path| !path.starts_with(&excluded))) =>
            {
                if matches!(
                    wake.try_send(Work::Event(event)),
                    Err(TrySendError::Full(_))
                ) {
                    overflow.store(true, Ordering::Release);
                }
            }
            Err(_) => {
                overflow.store(true, Ordering::Release);
                let _ = wake.try_send(Work::Refresh);
            }
            _ => {}
        })
        .ok();
    let mut watched = HashSet::<PathBuf>::new();
    let mut last_full = Instant::now();
    let mut full = true;
    let mut events = Vec::new();
    loop {
        let Some(inner) = weak.upgrade() else {
            break;
        };
        let library = Library(inner);
        let sources = library.0.index.sources().unwrap_or_default();
        let desired: HashSet<PathBuf> = sources.iter().map(|source| source.path.clone()).collect();
        if let Some(watcher) = watcher.as_mut() {
            for path in watched
                .iter()
                .filter(|path| !desired.contains(*path) || !path.exists())
            {
                let _ = watcher.unwatch(path);
            }
            watched.retain(|path| desired.contains(path) && path.exists());
            for path in desired.difference(&watched).cloned().collect::<Vec<_>>() {
                if watcher.watch(&path, RecursiveMode::Recursive).is_ok() {
                    watched.insert(path);
                }
            }
        }
        full |=
            library.0.overflow.swap(false, Ordering::AcqRel) || last_full.elapsed() >= RECONCILE;
        let had_events = !events.is_empty();
        let mut dirty = HashSet::<(u32, Vec<String>)>::new();
        let mut recursive = HashSet::<(u32, Vec<String>)>::new();
        for message in events.drain(..) {
            match message {
                Work::Refresh => full = true,
                Work::Folder(id, parts) => {
                    dirty.insert((id, parts));
                }
                Work::Event(event) => {
                    if event.paths.is_empty() {
                        full = true;
                    }
                    for path in event.paths {
                        if path.starts_with(&library.0.data) {
                            continue;
                        }
                        let mut matched = false;
                        for source in &sources {
                            if let Some(mut parts) = event_relative(&source.path, &path) {
                                matched = true;
                                if parts.is_empty() {
                                    recursive.insert((source.root.id, parts));
                                    continue;
                                }
                                if path.is_dir() {
                                    recursive.insert((source.root.id, parts.clone()));
                                }
                                parts.pop();
                                dirty.insert((source.root.id, parts));
                            }
                        }
                        if !matched {
                            full = true;
                        }
                    }
                }
            }
        }
        if full {
            for source in &sources {
                library.scan_tree(source, Vec::new());
            }
            last_full = Instant::now();
        } else {
            for (id, parts) in dirty {
                if let Some(source) = sources.iter().find(|source| source.root.id == id) {
                    library.reconcile(source, parts);
                }
            }
            for (id, parts) in recursive {
                if let Some(source) = sources.iter().find(|source| source.root.id == id) {
                    library.scan_tree(source, parts);
                }
            }
        }
        if full || had_events {
            if let Some(disk) = &library.0.disk {
                if let Err(error) = disk.trim() {
                    eprintln!("Cache maintenance: {error}");
                }
            }
        }
        full = false;
        drop(library);
        match receiver.recv_timeout(Duration::from_secs(2)) {
            Ok(message) => {
                events.push(message);
                thread::sleep(Duration::from_millis(300));
                events.extend(receiver.try_iter().take(256));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    #[test]
    fn persistence_incremental_updates_and_scoped_reads() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("pictures");
        let data = temp.path().join("data");
        fs::create_dir_all(source.join("nested")).unwrap();
        DynamicImage::new_rgb8(640, 480)
            .save(source.join("photo.bmp"))
            .unwrap();
        let (library, _receiver) = Library::initialize(&data).unwrap();
        let root = library.add(source.clone()).unwrap();
        let first = library.list(root.id, &[], true).unwrap();
        assert_eq!(first.photos.len(), 1);
        assert_eq!(first.folders, vec!["nested"]);
        assert_eq!(
            library.list(root.id, &[], true).unwrap().revision,
            first.revision
        );
        assert_eq!(
            library
                .metadata(root.id, &["photo.bmp".into()])
                .unwrap()
                .unwrap()
                .width,
            Some(640)
        );
        let thumbnail = library.thumbnail(root.id, &["photo.bmp".into()]).unwrap();
        assert_eq!(image::load_from_memory(&thumbnail).unwrap().width(), 320);
        drop(library);
        let (library, _receiver) = Library::initialize(&data).unwrap();
        assert_eq!(library.roots().unwrap()[0].id, root.id);
        assert_eq!(
            library.list(root.id, &[], false).unwrap().revision,
            first.revision
        );
        assert_eq!(
            library.thumbnail(root.id, &["photo.bmp".into()]).unwrap(),
            thumbnail
        );
        DynamicImage::new_rgb8(800, 600)
            .save(source.join("photo.bmp"))
            .unwrap();
        let modified = library.list(root.id, &[], true).unwrap();
        assert!(modified.revision > first.revision);
        assert_ne!(modified.photos[0].stamp, first.photos[0].stamp);
        fs::remove_file(source.join("photo.bmp")).unwrap();
        assert!(library.list(root.id, &[], true).unwrap().photos.is_empty());
        assert!(library.list(root.id, &["..".into()], false).is_err());
        library.remove(root.id).unwrap();
        assert!(library.list(root.id, &[], false).is_err());
        assert!(source.is_dir());
    }

    #[test]
    fn recursively_indexes_and_removes_subtrees() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("pictures");
        fs::create_dir_all(source.join("a")).unwrap();
        fs::create_dir_all(source.join("b")).unwrap();
        DynamicImage::new_rgb8(64, 48)
            .save(source.join("a/photo.png"))
            .unwrap();
        DynamicImage::new_rgb8(48, 64)
            .save(source.join("b/photo.png"))
            .unwrap();
        let (library, _receiver) = Library::initialize(&temp.path().join("data")).unwrap();
        let root = library.add(source.clone()).unwrap();
        let item = library.0.index.sources().unwrap().pop().unwrap();
        library.scan_tree(&item, Vec::new());
        assert_eq!(
            library
                .0
                .index
                .list(root.id, "a")
                .unwrap()
                .unwrap()
                .photos
                .len(),
            1
        );
        assert_eq!(
            library
                .0
                .index
                .list(root.id, "b")
                .unwrap()
                .unwrap()
                .photos
                .len(),
            1
        );
        let old = library.revision(root.id, &["a".into()]).unwrap();
        fs::remove_dir_all(source.join("a")).unwrap();
        library.reconcile(&item, Vec::new());
        assert!(library.0.index.list(root.id, "a").unwrap().is_none());
        fs::create_dir(source.join("a")).unwrap();
        library.reconcile(&item, Vec::new());
        assert!(library.revision(root.id, &["a".into()]).unwrap() > old);
    }

    #[test]
    fn filesystem_notifications_update_cached_listing() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("pictures");
        fs::create_dir_all(&source).unwrap();
        let library = Library::open(&temp.path().join("data")).unwrap();
        let root = library.add(source.clone()).unwrap();
        let wait = |condition: &dyn Fn() -> bool| {
            let start = Instant::now();
            while !condition() {
                assert!(
                    start.elapsed() < Duration::from_secs(15),
                    "filesystem reconciliation timed out"
                );
                thread::sleep(Duration::from_millis(100));
            }
        };
        wait(&|| library.0.index.revision(root.id, "").unwrap().is_some());
        DynamicImage::new_rgb8(64, 48)
            .save(source.join("new.png"))
            .unwrap();
        wait(&|| {
            library
                .0
                .index
                .list(root.id, "")
                .unwrap()
                .is_some_and(|listing| listing.photos.len() == 1)
        });
        fs::remove_file(source.join("new.png")).unwrap();
        wait(&|| {
            library
                .0
                .index
                .list(root.id, "")
                .unwrap()
                .is_some_and(|listing| listing.photos.is_empty())
        });
    }
    #[test]
    fn listing_is_published_before_metadata_enrichment() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("pictures");
        fs::create_dir_all(&source).unwrap();
        DynamicImage::new_rgb8(80, 60)
            .save(source.join("photo.png"))
            .unwrap();
        let (library, _receiver) = Library::initialize(&temp.path().join("data")).unwrap();
        let root = library.add(source).unwrap();
        let initial = library.list(root.id, &[], true).unwrap();
        assert_eq!(initial.photos.len(), 1);
        assert_eq!(initial.photos[0].width, None);
        library.enrich_folder(root.id, &[]);
        let enriched = library.list(root.id, &[], false).unwrap();
        assert_eq!(enriched.photos[0].width, Some(80));
        assert!(enriched.revision > initial.revision);
        assert_eq!(initial.photos[0].stamp, enriched.photos[0].stamp);
    }
    #[test]
    fn full_image_requires_current_scoped_stamp() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("pictures");
        fs::create_dir_all(&source).unwrap();
        DynamicImage::new_rgb8(640, 480)
            .save(source.join("photo.png"))
            .unwrap();
        let (library, _receiver) = Library::initialize(&temp.path().join("data")).unwrap();
        let root = library.add(source).unwrap();
        let listing = library.list(root.id, &[], true).unwrap();
        let photo = &listing.photos[0];
        let parts = vec![photo.name.clone()];
        assert!(library.full_image(root.id, &parts, &photo.stamp).is_ok());
        assert!(library.full_image(root.id, &parts, "stale").is_err());
        assert!(library
            .full_image(root.id, &["..".into()], &photo.stamp)
            .is_err());
        library.remove(root.id).unwrap();
        assert!(library.full_image(root.id, &parts, &photo.stamp).is_err());
    }
}
