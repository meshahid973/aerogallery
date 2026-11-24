use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::{self, Metadata},
    io::Cursor,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ENTRIES: usize = 25_000;
const MAX_ROOTS: usize = 64;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 160 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;
const CACHE_BYTES: usize = 16 * 1024 * 1024;
const CACHE_ENTRIES: usize = 64;

#[derive(Clone, Serialize)]
pub struct Root {
    pub id: u32,
    pub name: String,
}

struct Source {
    root: Root,
    path: PathBuf,
}

#[derive(Serialize)]
pub struct Photo {
    pub name: String,
    pub bytes: u64,
    pub modified: u64,
}

#[derive(Serialize)]
pub struct Listing {
    pub folders: Vec<String>,
    pub photos: Vec<Photo>,
    pub skipped: usize,
}

#[derive(Clone, Default)]
pub struct Library(Arc<Inner>);

#[derive(Default)]
struct Inner {
    sources: Mutex<Vec<Source>>,
    next_id: AtomicUsize,
    picker: Arc<AtomicUsize>,
    listing: Arc<AtomicUsize>,
    decoding: Arc<AtomicUsize>,
    cache: Mutex<Cache>,
}

pub struct Permit(Arc<AtomicUsize>);

impl Permit {
    fn acquire(counter: &Arc<AtomicUsize>, limit: usize) -> Result<Self, String> {
        counter
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |count| {
                (count < limit).then_some(count + 1)
            })
            .map_err(|_| "Please try again in a moment.".to_string())?;
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
    path: PathBuf,
    modified: SystemTime,
    bytes: u64,
}

#[derive(Default)]
struct Cache {
    items: VecDeque<(CacheKey, Vec<u8>)>,
    bytes: usize,
}

impl Cache {
    fn get(&mut self, key: &CacheKey) -> Option<Vec<u8>> {
        let index = self.items.iter().position(|(candidate, _)| candidate == key)?;
        let entry = self.items.remove(index)?;
        let data = entry.1.clone();
        self.items.push_front(entry);
        Some(data)
    }

    fn insert(&mut self, key: CacheKey, data: Vec<u8>) {
        if data.len() > CACHE_BYTES {
            return;
        }
        if let Some(index) = self.items.iter().position(|(candidate, _)| candidate == &key) {
            if let Some((_, previous)) = self.items.remove(index) {
                self.bytes -= previous.len();
            }
        }
        while self.items.len() >= CACHE_ENTRIES || self.bytes + data.len() > CACHE_BYTES {
            if let Some((_, previous)) = self.items.pop_back() {
                self.bytes -= previous.len();
            } else {
                break;
            }
        }
        self.bytes += data.len();
        self.items.push_front((key, data));
    }
}

fn lock_error<T>(_: std::sync::PoisonError<T>) -> String {
    "The library is unavailable.".to_string()
}

fn is_link(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

fn valid_component(name: &str) -> bool {
    let mut components = Path::new(name).components();
    !name.is_empty()
        && !name.contains(['/', '\\', ':', '\0'])
        && matches!(components.next(), Some(Component::Normal(part)) if part == name)
        && components.next().is_none()
}

fn allowed_photo(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["jpg", "jpeg", "png", "webp"]
                .iter()
                .any(|format| extension.eq_ignore_ascii_case(format))
        })
}

fn resolve(root: &Path, parts: &[String]) -> Result<PathBuf, String> {
    if parts.len() > 128 || parts.iter().any(|part| !valid_component(part)) {
        return Err("Invalid folder or picture.".into());
    }
    let unavailable = || "The selected item is no longer available.".to_string();
    let root_metadata = fs::symlink_metadata(root).map_err(|_| unavailable())?;
    if is_link(&root_metadata) || !root_metadata.is_dir() {
        return Err(unavailable());
    }
    let mut path = root.to_path_buf();
    for part in parts {
        path.push(part);
        if is_link(&fs::symlink_metadata(&path).map_err(|_| unavailable())?) {
            return Err("Linked folders and pictures are not supported.".into());
        }
    }
    let canonical = fs::canonicalize(&path).map_err(|_| unavailable())?;
    if !canonical.starts_with(root) {
        return Err("The selected item is outside the library.".into());
    }
    Ok(canonical)
}

impl Library {
    pub fn picker_permit(&self) -> Result<Permit, String> {
        Permit::acquire(&self.0.picker, 1)
    }

    pub fn listing_permit(&self) -> Result<Permit, String> {
        Permit::acquire(&self.0.listing, 1)
    }

    pub fn decode_permit(&self) -> Result<Permit, String> {
        Permit::acquire(&self.0.decoding, 2)
    }

    pub fn roots(&self) -> Result<Vec<Root>, String> {
        Ok(self.0.sources.lock().map_err(lock_error)?.iter().map(|source| source.root.clone()).collect())
    }

    pub fn add(&self, selected: PathBuf) -> Result<Root, String> {
        let path = fs::canonicalize(&selected).map_err(|_| "Cannot open this folder.")?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| "Cannot open this folder.")?;
        if !metadata.is_dir() || is_link(&metadata) {
            return Err("Choose an accessible folder.".into());
        }
        let mut sources = self.0.sources.lock().map_err(lock_error)?;
        if let Some(source) = sources.iter().find(|source| source.path == path) {
            return Ok(source.root.clone());
        }
        if sources.len() >= MAX_ROOTS {
            return Err("Remove a folder before adding another.".into());
        }
        let id = u32::try_from(self.0.next_id.fetch_add(1, Ordering::Relaxed))
            .map_err(|_| "Too many folders.")?;
        let name = selected.file_name().map_or_else(
            || selected.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let root = Root { id, name };
        sources.push(Source { root: root.clone(), path });
        Ok(root)
    }

    pub fn remove(&self, id: u32) -> Result<(), String> {
        self.0.sources.lock().map_err(lock_error)?.retain(|source| source.root.id != id);
        *self.0.cache.lock().map_err(lock_error)? = Cache::default();
        Ok(())
    }

    fn root_path(&self, id: u32) -> Result<PathBuf, String> {
        self.0.sources.lock().map_err(lock_error)?.iter()
            .find(|source| source.root.id == id)
            .map(|source| source.path.clone())
            .ok_or_else(|| "This folder is no longer in the library.".into())
    }

    pub fn list(&self, id: u32, parts: &[String]) -> Result<Listing, String> {
        let path = resolve(&self.root_path(id)?, parts)?;
        let entries = fs::read_dir(path).map_err(|_| "Cannot read this folder.")?;
        let mut listing = Listing { folders: Vec::new(), photos: Vec::new(), skipped: 0 };
        for (index, entry) in entries.enumerate() {
            if index >= MAX_ENTRIES {
                return Err("This folder contains more than 25,000 entries. Open a smaller folder.".into());
            }
            let Ok(entry) = entry else {
                listing.skipped += 1;
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                listing.skipped += 1;
                continue;
            };
            if !kind.is_dir() && !(kind.is_file() && allowed_photo(&entry.path())) {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
                listing.skipped += 1;
                continue;
            };
            if is_link(&metadata) {
                continue;
            }
            let Ok(name) = entry.file_name().into_string() else {
                listing.skipped += 1;
                continue;
            };
            if !valid_component(&name) {
                listing.skipped += 1;
                continue;
            }
            if metadata.is_dir() {
                listing.folders.push(name);
            } else if metadata.is_file() {
                let modified = metadata.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |duration| duration.as_secs());
                listing.photos.push(Photo { name, bytes: metadata.len(), modified });
            }
        }
        listing.folders.sort_by_cached_key(|name| name.to_lowercase());
        listing.photos.sort_by_cached_key(|photo| photo.name.to_lowercase());
        Ok(listing)
    }

    pub fn thumbnail(&self, id: u32, parts: &[String]) -> Result<Vec<u8>, String> {
        let path = resolve(&self.root_path(id)?, parts)?;
        let unavailable = || "Picture unavailable.".to_string();
        let metadata = fs::symlink_metadata(&path).map_err(|_| unavailable())?;
        if !metadata.is_file() || is_link(&metadata) || !allowed_photo(&path) || metadata.len() > MAX_FILE_BYTES {
            return Err(unavailable());
        }
        let key = metadata.modified().ok().map(|modified| CacheKey { path: path.clone(), modified, bytes: metadata.len() });
        if let Some(key) = &key {
            if let Some(data) = self.0.cache.lock().map_err(lock_error)?.get(key) {
                return Ok(data);
            }
        }
        let mut reader = ImageReader::open(&path).map_err(|_| unavailable())?.with_guessed_format().map_err(|_| unavailable())?;
        if !matches!(reader.format(), Some(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)) {
            return Err(unavailable());
        }
        let mut limits = Limits::default();
        limits.max_image_width = Some(16_384);
        limits.max_image_height = Some(16_384);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().map_err(|_| unavailable())?;
        let (width, height) = decoder.dimensions();
        if u64::from(width) * u64::from(height) > MAX_PIXELS || decoder.total_bytes() > MAX_DECODE_BYTES {
            return Err("Picture is too large to preview.".into());
        }
        let orientation = decoder.orientation().map_err(|_| unavailable())?;
        let mut image = DynamicImage::from_decoder(decoder).map_err(|_| unavailable())?;
        image.apply_orientation(orientation);
        let preview = if image.width() > 320 || image.height() > 320 { image.thumbnail(320, 320) } else { image };
        let mut output = Cursor::new(Vec::new());
        preview.write_to(&mut output, ImageFormat::Png).map_err(|_| unavailable())?;
        let bytes = output.into_inner();
        if let Some(key) = key {
            if self.root_path(id).is_ok() {
                self.0.cache.lock().map_err(lock_error)?.insert(key, bytes.clone());
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_escape_components() {
        for name in ["", ".", "..", "../outside", "a/b", "a\\b", "C:drive", "photo.jpg:stream", "\0"] {
            assert!(!valid_component(name), "{name:?}");
        }
        assert!(valid_component("Summer coast.jpg"));
        assert!(valid_component("写真"));
    }

    #[test]
    fn cache_is_bounded_and_refreshes_entries() {
        let mut cache = Cache::default();
        for index in 0..100 {
            cache.insert(CacheKey { path: PathBuf::from(index.to_string()), modified: UNIX_EPOCH, bytes: 1 }, vec![1; 300_000]);
        }
        assert!(cache.items.len() <= CACHE_ENTRIES);
        assert!(cache.bytes <= CACHE_BYTES);
        let key = CacheKey { path: PathBuf::from("99"), modified: UNIX_EPOCH, bytes: 1 };
        assert_eq!(cache.get(&key).map(|bytes| bytes.len()), Some(300_000));
        cache.insert(key, vec![2]);
        assert_eq!(cache.bytes, cache.items.iter().map(|(_, bytes)| bytes.len()).sum::<usize>());
    }

    #[test]
    fn reads_selected_folder_only_and_creates_thumbnail() {
        let dir = std::env::temp_dir().join(format!("aerogallery-test-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(dir.join("nested")).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
        }
        let _cleanup = Cleanup(dir.clone());
        DynamicImage::new_rgb8(640, 480).save(dir.join("photo.png")).unwrap();
        fs::write(dir.join("notes.txt"), b"not a photo").unwrap();
        let library = Library::default();
        let root = library.add(dir).unwrap();
        let listing = library.list(root.id, &[]).unwrap();
        assert_eq!(listing.folders, vec!["nested"]);
        assert_eq!(listing.photos.len(), 1);
        assert!(library.list(root.id, &["..".into()]).is_err());
        let thumbnail = library.thumbnail(root.id, &["photo.png".into()]).unwrap();
        assert_eq!(image::load_from_memory(&thumbnail).unwrap().width(), 320);
        library.remove(root.id).unwrap();
        assert!(library.list(root.id, &[]).is_err());
    }
}
