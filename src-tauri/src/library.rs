use crate::{disk_cache::DiskCache, imaging::{self, PhotoMetadata}, index::{Index, Source}, paths::{self, fingerprint, is_link}};
pub use crate::index::{Listing, Root};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{atomic::{AtomicUsize, Ordering}, mpsc::{self, Receiver, SyncSender, TrySendError}, Arc, Mutex, Weak},
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
    disk: DiskCache,
    data: PathBuf,
    pool: rayon::ThreadPool,
    picker: Arc<AtomicUsize>,
    listing: Arc<AtomicUsize>,
    decoding: Arc<AtomicUsize>,
    scan_lock: Mutex<()>,
    cache: Mutex<MemoryCache>,
    wake: SyncSender<Work>,
    overflow: Arc<std::sync::atomic::AtomicBool>,
}

enum Work { Refresh, Event(Event) }

pub struct Permit(Arc<AtomicUsize>);

impl Permit {
    fn acquire(counter: &Arc<AtomicUsize>, limit: usize) -> Result<Self,String> {
        counter.fetch_update(Ordering::AcqRel,Ordering::Relaxed,|count| (count < limit).then_some(count+1)).map_err(|_| "Please try again in a moment.".to_string())?;
        Ok(Self(counter.clone()))
    }
}

impl Drop for Permit {
    fn drop(&mut self) { self.0.fetch_sub(1,Ordering::Release); }
}

#[derive(PartialEq,Eq)]
struct CacheKey { root: u32, path: String, stamp: String }

#[derive(Default)]
struct MemoryCache {
    items: VecDeque<(CacheKey,Vec<u8>)>,
    bytes: usize,
}

impl MemoryCache {
    fn get(&mut self,key:&CacheKey) -> Option<Vec<u8>> {
        let index = self.items.iter().position(|(candidate,_)| candidate==key)?;
        let entry = self.items.remove(index)?;
        let data = entry.1.clone();
        self.items.push_front(entry);
        Some(data)
    }

    fn put(&mut self,key:CacheKey,data:Vec<u8>) {
        if data.len() > RAM_BYTES { return; }
        self.retain(|candidate| candidate.root != key.root || candidate.path != key.path);
        while self.items.len() >= RAM_ENTRIES || self.bytes + data.len() > RAM_BYTES {
            if let Some((_,previous)) = self.items.pop_back() { self.bytes -= previous.len(); } else { break; }
        }
        self.bytes += data.len();
        self.items.push_front((key,data));
    }

    fn retain(&mut self, keep: impl Fn(&CacheKey)->bool) {
        self.items.retain(|(key,_)| keep(key));
        self.bytes = self.items.iter().map(|(_,data)|data.len()).sum();
    }
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> String { "The library is unavailable.".into() }

impl Library {
    pub fn open(data: &Path) -> Result<Self,String> {
        let (library, receiver) = Self::initialize(data)?;
        let weak = Arc::downgrade(&library.0);
        thread::Builder::new().name("gallery-index".into()).spawn(move || worker(weak,receiver)).map_err(|_| "Cannot start library indexing.")?;
        Ok(library)
    }

    fn initialize(data: &Path) -> Result<(Self,Receiver<Work>),String> {
        fs::create_dir_all(data).map_err(|_| "Cannot create library storage.")?;
        let data = fs::canonicalize(data).map_err(|_| "Cannot access library storage.")?;
        let index = Index::open(&data.join("library.sqlite3"))?;
        let disk = DiskCache::open(&data.join("thumbnails.sqlite3")).map_err(|error| { eprintln!("Thumbnail database: {error}"); "Cannot open thumbnail cache." })?;
        let pool = rayon::ThreadPoolBuilder::new().num_threads(2).thread_name(|id|format!("gallery-metadata-{id}")).build().map_err(|_| "Cannot start metadata workers.")?;
        let (wake,receiver) = mpsc::sync_channel(256);
        Ok((Self(Arc::new(Inner { index,disk,data,pool,picker:Arc::new(AtomicUsize::new(0)),listing:Arc::new(AtomicUsize::new(0)),decoding:Arc::new(AtomicUsize::new(0)),scan_lock:Mutex::new(()),cache:Mutex::new(MemoryCache::default()),wake,overflow:Arc::new(std::sync::atomic::AtomicBool::new(false)) })),receiver))
    }

    pub fn picker_permit(&self) -> Result<Permit,String> { Permit::acquire(&self.0.picker,1) }
    pub fn listing_permit(&self) -> Result<Permit,String> { Permit::acquire(&self.0.listing,1) }
    pub fn decode_permit(&self) -> Result<Permit,String> { Permit::acquire(&self.0.decoding,2) }

    pub fn roots(&self) -> Result<Vec<Root>,String> { Ok(self.0.index.sources()?.into_iter().map(|source|source.root).collect()) }

    fn wake(&self) {
        if matches!(self.0.wake.try_send(Work::Refresh),Err(TrySendError::Full(_))) { self.0.overflow.store(true,Ordering::Release); }
    }

    pub fn add(&self, selected: PathBuf) -> Result<Root,String> {
        let path = fs::canonicalize(&selected).map_err(|_| "Cannot open this folder.")?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| "Cannot open this folder.")?;
        if !metadata.is_dir() || is_link(&metadata) || path.starts_with(&self.0.data) {
            return Err("Choose an accessible picture folder.".into());
        }
        let name = selected.file_name().map_or_else(||selected.to_string_lossy().into_owned(), |name|name.to_string_lossy().into_owned());
        let root = self.0.index.add(&path,name)?;
        self.wake();
        Ok(root)
    }

    pub fn remove(&self,id:u32) -> Result<(),String> {
        self.0.index.remove(id)?;
        self.0.cache.lock().map_err(lock_error)?.retain(|key|key.root != id);
        if let Err(error) = self.0.disk.remove_root(id) { eprintln!("Cache removal: {error}"); }
        self.wake();
        Ok(())
    }

    fn resolved(&self,id:u32,parts:&[String]) -> Result<PathBuf,String> {
        let path = paths::resolve(&self.0.index.root_path(id)?,parts)?;
        if path.starts_with(&self.0.data) { return Err("Application storage is not a picture folder.".into()); }
        Ok(path)
    }

    fn scan_one(&self,id:u32,parts:&[String]) -> Result<Vec<String>,String> {
        let _guard = self.0.scan_lock.lock().map_err(lock_error)?;
        let directory = self.resolved(id,parts)?;
        let relative = paths::relative(parts)?;
        let (folders,invalidated) = self.0.index.scan(id,&relative,&directory,&self.0.data,&self.0.pool)?;
        if !invalidated.is_empty() {
            let matches = |key:&CacheKey| key.root==id && invalidated.iter().any(|path| key.path == *path || key.path.strip_prefix(path).is_some_and(|suffix|suffix.starts_with('/')));
            self.0.cache.lock().map_err(lock_error)?.retain(|key|!matches(key));
            for path in invalidated {
                if let Err(error) = self.0.disk.invalidate(id,&path) { eprintln!("Cache invalidation: {error}"); }
            }
        }
        Ok(folders)
    }

    pub fn list(&self,id:u32,parts:&[String],refresh:bool) -> Result<Listing,String> {
        self.resolved(id,parts)?;
        let relative = paths::relative(parts)?;
        if !refresh {
            if let Some(listing) = self.0.index.list(id,&relative)? { return Ok(listing); }
        }
        self.scan_one(id,parts)?;
        self.0.index.list(id,&relative)?.ok_or_else(||"Cannot read this folder.".into())
    }

    pub fn revision(&self,id:u32,parts:&[String]) -> Result<i64,String> {
        self.resolved(id,parts)?;
        Ok(self.0.index.revision(id,&paths::relative(parts)?)?.unwrap_or(0))
    }

    pub fn metadata(&self,id:u32,parts:&[String]) -> Result<Option<PhotoMetadata>,String> {
        self.resolved(id,parts)?;
        self.0.index.metadata(id,&paths::relative(parts)?)
    }

    pub fn thumbnail(&self,id:u32,parts:&[String]) -> Result<Vec<u8>,String> {
        let path = self.resolved(id,parts)?;
        let metadata = fs::symlink_metadata(&path).map_err(|_|"Picture unavailable.")?;
        if !metadata.is_file() || is_link(&metadata) || !paths::allowed_photo(&path) || metadata.len() > imaging::MAX_FILE_BYTES { return Err("Picture unavailable.".into()); }
        let (stamp,_) = fingerprint(&metadata);
        let key = CacheKey { root:id,path:paths::relative(parts)?,stamp:format!("{}:{stamp}",imaging::THUMB_VERSION) };
        if let Some(data) = self.0.cache.lock().map_err(lock_error)?.get(&key) { return Ok(data); }
        match self.0.disk.get(id,&key.path,&key.stamp) {
            Ok(Some(data)) => {
                self.0.cache.lock().map_err(lock_error)?.put(key,data.clone());
                return Ok(data);
            }
            Ok(None) => {}
            Err(error) => eprintln!("Cache read: {error}"),
        }
        let data = imaging::thumbnail(&path)?;
        let current = self.resolved(id,parts)?;
        if current != path || !fs::symlink_metadata(&current).is_ok_and(|meta| !is_link(&meta) && fingerprint(&meta).0==stamp) { return Err("Picture changed while decoding.".into()); }
        if let Err(error) = self.0.disk.put(id,&key.path,&key.stamp,&data) { eprintln!("Cache write: {error}"); }
        self.0.cache.lock().map_err(lock_error)?.put(key,data.clone());
        Ok(data)
    }

    fn scan_tree(&self,source:&Source) {
        let mut pending = vec![Vec::<String>::new()];
        while let Some(parts) = pending.pop() {
            if let Ok(folders) = self.scan_one(source.root.id,&parts) {
                if parts.len() < 128 {
                    for folder in folders.into_iter().rev() {
                        let mut child = parts.clone();
                        child.push(folder);
                        pending.push(child);
                    }
                }
            }
            if self.0.index.root_path(source.root.id).is_err() { break; }
            thread::yield_now();
        }
    }
}

fn worker(weak:Weak<Inner>,receiver:Receiver<Work>) {
    let Some(inner) = weak.upgrade() else { return; };
    let wake = inner.wake.clone();
    let overflow = inner.overflow.clone();
    let excluded = inner.data.clone();
    drop(inner);
    let mut watcher = notify::recommended_watcher(move |result:notify::Result<Event>| {
        match result {
            Ok(event) if !matches!(event.kind,EventKind::Access(_)) && (event.paths.is_empty() || event.paths.iter().any(|path|!path.starts_with(&excluded))) => {
                if matches!(wake.try_send(Work::Event(event)),Err(TrySendError::Full(_))) { overflow.store(true,Ordering::Release); }
            }
            Err(_) => { overflow.store(true,Ordering::Release); let _ = wake.try_send(Work::Refresh); }
            _ => {}
        }
    }).ok();
    let mut watched = HashSet::new();
    let mut last_full = Instant::now();
    let mut full = true;
    loop {
        let Some(inner) = weak.upgrade() else { break; };
        let library = Library(inner);
        let sources = library.0.index.sources().unwrap_or_default();
        let desired:HashSet<PathBuf> = sources.iter().map(|source|source.path.clone()).collect();
        if let Some(watcher) = watcher.as_mut() {
            for path in watched.difference(&desired) { let _ = watcher.unwatch(path); }
            watched.retain(|path|desired.contains(path));
            for path in desired.difference(&watched).cloned().collect::<Vec<_>>() {
                if watcher.watch(&path,RecursiveMode::Recursive).is_ok() { watched.insert(path); }
            }
        }
        let mut dirty = HashSet::new();
        if !full {
            drop(library);
            let message = receiver.recv_timeout(Duration::from_secs(2));
            let Some(inner) = weak.upgrade() else { break; };
            let library = Library(inner);
            match message {
                Ok(message) => {
                    let mut events = vec![message];
                    thread::sleep(Duration::from_millis(300));
                    events.extend(receiver.try_iter().take(256));
                    for message in events {
                        match message {
                            Work::Refresh => full=true,
                            Work::Event(event) => {
                                if event.paths.is_empty() { full=true; }
                                for source in &sources {
                                    if event.paths.iter().any(|path|path.starts_with(&source.path) || source.path.starts_with(path)) { dirty.insert(source.root.id); }
                                }
                            }
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            full |= library.0.overflow.swap(false,Ordering::AcqRel) || last_full.elapsed() >= RECONCILE;
            if full || !dirty.is_empty() {
                let current = library.0.index.sources().unwrap_or_default();
                for source in &current {
                    if full || dirty.contains(&source.root.id) { library.scan_tree(source); }
                }
                if full { last_full=Instant::now(); }
                if let Err(error) = library.0.disk.trim() { eprintln!("Cache maintenance: {error}"); }
            }
            full=false;
            continue;
        }
        for source in &sources { library.scan_tree(source); }
        last_full=Instant::now();
        full=false;
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
        DynamicImage::new_rgb8(640,480).save(source.join("photo.bmp")).unwrap();
        let (library,_receiver) = Library::initialize(&data).unwrap();
        let root = library.add(source.clone()).unwrap();
        let first = library.list(root.id,&[],true).unwrap();
        assert_eq!(first.photos.len(),1);
        assert_eq!(first.folders,vec!["nested"]);
        assert_eq!(library.list(root.id,&[],true).unwrap().revision,first.revision);
        assert_eq!(library.metadata(root.id,&["photo.bmp".into()]).unwrap().unwrap().width,Some(640));
        let thumbnail=library.thumbnail(root.id,&["photo.bmp".into()]).unwrap();
        assert_eq!(image::load_from_memory(&thumbnail).unwrap().width(),320);
        drop(library);
        let (library,_receiver) = Library::initialize(&data).unwrap();
        assert_eq!(library.roots().unwrap()[0].id,root.id);
        assert_eq!(library.list(root.id,&[],false).unwrap().revision,first.revision);
        assert_eq!(library.thumbnail(root.id,&["photo.bmp".into()]).unwrap(),thumbnail);
        DynamicImage::new_rgb8(800,600).save(source.join("photo.bmp")).unwrap();
        let modified=library.list(root.id,&[],true).unwrap();
        assert!(modified.revision>first.revision);
        assert_ne!(modified.photos[0].stamp,first.photos[0].stamp);
        fs::remove_file(source.join("photo.bmp")).unwrap();
        assert!(library.list(root.id,&[],true).unwrap().photos.is_empty());
        assert!(library.list(root.id,&["..".into()],false).is_err());
        library.remove(root.id).unwrap();
        assert!(library.list(root.id,&[],false).is_err());
        assert!(source.is_dir());
    }
}
